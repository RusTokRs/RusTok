use crate::manifest_io::workspace_root;
use crate::manifest_validation::{module_validate_command, to_pascal_case};
use crate::module_commands::module_lint_command;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

mod backend;
mod options;
pub(crate) mod registration;
mod ui;

#[allow(unused_imports)]
pub use options::{
    default_title_from_slug, is_valid_slug, parse_options, print_scaffold_usage, ScaffoldOptions,
    ScaffoldProfile, ScaffoldUi,
};

pub(crate) fn scaffold_command(args: &[String]) -> Result<()> {
    let options = match options::parse_options(args)? {
        Some(opts) => opts,
        None => return Ok(()),
    };

    scaffold(&options)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScaffoldValidation {
    Verified,
    Skipped,
}

fn validate_scaffold_with(
    slug: &str,
    skip_validate: bool,
    lint: impl FnOnce(&[String]) -> Result<()>,
    validate: impl FnOnce(&[String]) -> Result<()>,
) -> Result<ScaffoldValidation> {
    if skip_validate {
        return Ok(ScaffoldValidation::Skipped);
    }
    let args = vec![slug.to_owned()];
    lint(&args).with_context(|| {
        format!("Module '{slug}' files were written but architecture lint failed; scaffold is NOT verified")
    })?;
    validate(&args).with_context(|| {
        format!("Module '{slug}' files were written but publish-readiness validation failed; scaffold is NOT verified")
    })?;
    Ok(ScaffoldValidation::Verified)
}

fn validate_scaffold(slug: &str, skip_validate: bool) -> Result<ScaffoldValidation> {
    validate_scaffold_with(
        slug,
        skip_validate,
        module_lint_command,
        module_validate_command,
    )
}

#[derive(Debug, Clone)]
struct PlannedFile {
    path: PathBuf,
    content: String,
    original: Option<Vec<u8>>,
}

struct FileSnapshot {
    path: PathBuf,
    original: Option<Vec<u8>>,
    written: Vec<u8>,
}

struct ScaffoldTransaction {
    root: PathBuf,
    snapshots: Vec<FileSnapshot>,
    created_dirs: Vec<PathBuf>,
}

impl ScaffoldTransaction {
    fn apply(root: &Path, plan: &[PlannedFile], module_dir: &Path) -> Result<Self> {
        Self::apply_with_writer(root, plan, module_dir, |output, content| {
            output.write(content)
        })
    }

    fn apply_with_writer(
        root: &Path,
        plan: &[PlannedFile],
        module_dir: &Path,
        mut write: impl FnMut(&mut fs::File, &[u8]) -> std::io::Result<usize>,
    ) -> Result<Self> {
        // Refuse a stale plan before claiming a directory or changing any file.
        ensure_no_symlink_ancestors(root, module_dir)?;
        for file in plan {
            ensure_no_symlink_ancestors(root, &file.path)?;
            if file.original.is_none() && !file.path.starts_with(module_dir) {
                anyhow::bail!(
                    "Create target escapes module directory: {}",
                    file.path.display()
                );
            }
            Self::check_original(file)?;
        }

        fs::create_dir(module_dir).with_context(|| {
            format!(
                "Failed to exclusively create module directory {}",
                module_dir.display()
            )
        })?;
        let mut transaction = Self {
            root: root.to_path_buf(),
            snapshots: Vec::with_capacity(plan.len()),
            created_dirs: vec![module_dir.to_path_buf()],
        };

        for file in plan {
            let write_result = (|| -> Result<()> {
                ensure_no_symlink_ancestors(root, &file.path)?;
                if let Some(parent) = file.path.parent() {
                    transaction.create_parent_dirs(parent)?;
                }
                Self::check_original(file)?;
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(file.original.is_none())
                    .truncate(file.original.is_some())
                    .open(&file.path)
                    .with_context(|| format!("Failed to open {}", file.path.display()))?;
                let mut snapshot = FileSnapshot {
                    path: file.path.clone(),
                    original: file.original.clone(),
                    written: Vec::with_capacity(file.content.len()),
                };
                let mut remaining = file.content.as_bytes();
                let result = (|| -> std::io::Result<()> {
                    while !remaining.is_empty() {
                        match write(&mut output, remaining) {
                            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                            Ok(count) => {
                                snapshot.written.extend_from_slice(&remaining[..count]);
                                remaining = &remaining[count..];
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                                continue
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Ok(())
                })();
                // Record the actual prefix even on failure so rollback can restore
                // our truncated/partial write without mistaking it for an external edit.
                transaction.snapshots.push(snapshot);
                result.with_context(|| format!("Failed to write {}", file.path.display()))?;
                Ok(())
            })();

            if let Err(error) = write_result {
                return match transaction.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(error).context(format!(
                        "Scaffold write failed and rollback also failed: {rollback_error}"
                    )),
                };
            }
        }

        Ok(transaction)
    }

    fn check_original(file: &PlannedFile) -> Result<()> {
        let actual = match fs::symlink_metadata(&file.path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                Some(fs::read(&file.path)?)
            }
            Ok(_) => anyhow::bail!(
                "Scaffold target must be a regular file: {}",
                file.path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("Failed to inspect scaffold target"),
        };
        if actual != file.original {
            anyhow::bail!(
                "Scaffold target changed since planning: {}",
                file.path.display()
            );
        }
        Ok(())
    }

    fn create_parent_dirs(&mut self, parent: &Path) -> Result<()> {
        let mut missing = Vec::new();
        let mut current = parent;
        while !current.try_exists()? {
            missing.push(current.to_path_buf());
            current = current
                .parent()
                .context("Scaffold directory has no parent")?;
        }
        for directory in missing.into_iter().rev() {
            fs::create_dir(&directory)
                .with_context(|| format!("Failed to create {}", directory.display()))?;
            self.created_dirs.push(directory);
        }
        Ok(())
    }

    fn rollback(self) -> Result<()> {
        let mut failures = Vec::new();
        for snapshot in self.snapshots.into_iter().rev() {
            if let Err(error) = ensure_no_symlink_ancestors(&self.root, &snapshot.path) {
                failures.push(error.to_string());
                continue;
            }
            // Preserve edits made after our write, including untracked owner work.
            let current = match fs::symlink_metadata(&snapshot.path) {
                Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                    fs::read(&snapshot.path)
                }
                Ok(_) => {
                    failures.push(format!(
                        "Rollback target is no longer a regular file: {}",
                        snapshot.path.display()
                    ));
                    continue;
                }
                Err(error) => Err(error),
            };
            match current {
                Ok(content) if content == snapshot.written => {}
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && snapshot.original.is_none() =>
                {
                    continue
                }
                _ => {
                    failures.push(format!(
                        "Rollback preserved changed target: {}",
                        snapshot.path.display()
                    ));
                    continue;
                }
            }
            let result = match snapshot.original {
                Some(content) => fs::write(&snapshot.path, content)
                    .with_context(|| format!("Failed to restore {}", snapshot.path.display())),
                None => match fs::remove_file(&snapshot.path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error)
                        .with_context(|| format!("Failed to remove {}", snapshot.path.display())),
                },
            };
            if let Err(error) = result {
                failures.push(error.to_string());
            }
        }

        // Only remove directories we created, and only while they are empty.
        for directory in self.created_dirs.into_iter().rev() {
            if let Err(error) = ensure_no_symlink_ancestors(&self.root, &directory) {
                failures.push(error.to_string());
                continue;
            }
            match fs::remove_dir(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => failures.push(format!(
                    "Rollback preserved directory {}: {error}",
                    directory.display()
                )),
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("Scaffold rollback incomplete: {}", failures.join("; "));
        }
    }

    fn commit(self) {}
}

fn ensure_no_symlink_ancestors(root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("Scaffold target escapes workspace: {}", path.display()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            anyhow::bail!(
                "Scaffold target contains a non-normal path component: {}",
                path.display()
            );
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                anyhow::bail!("Scaffold target traverses a symlink: {}", current.display());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to inspect {}", current.display()));
            }
        }
    }
    Ok(())
}

fn build_scaffold_plan(
    root: &Path,
    module_dir: &Path,
    options: &ScaffoldOptions,
    pascal_name: &str,
    snake_name: &str,
) -> Result<Vec<PlannedFile>> {
    match fs::symlink_metadata(module_dir) {
        Ok(_) => anyhow::bail!(
            "Module crate directory already exists at {}",
            module_dir.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to inspect {}", module_dir.display()));
        }
    }

    let modules_toml_path = root.join("modules.toml");
    let modules_content = fs::read_to_string(&modules_toml_path)
        .with_context(|| format!("Failed to read {}", modules_toml_path.display()))?;
    let composition: toml::Value = toml::from_str(&modules_content)?;
    let modules = composition
        .get("modules")
        .and_then(toml::Value::as_table)
        .context("modules.toml is missing its [modules] table")?;
    if modules.contains_key(&options.slug) {
        anyhow::bail!(
            "Module '{}' is already registered in modules.toml",
            options.slug
        );
    }

    let mut plan = Vec::new();
    for (relative, content) in backend::render_backend(options, pascal_name)
        .into_iter()
        .chain(ui::render_ui_templates(options, pascal_name))
    {
        plan.push(PlannedFile {
            path: module_dir.join(relative),
            content,
            original: None,
        });
    }

    for change in registration::plan_module_registration(root, options, pascal_name, snake_name)? {
        plan.push(PlannedFile {
            path: change.path,
            content: change.content,
            original: Some(change.original.into_bytes()),
        });
    }

    let mut seen = HashSet::new();
    for file in &plan {
        if !file.path.starts_with(root) {
            anyhow::bail!("Scaffold target escapes workspace: {}", file.path.display());
        }
        if !seen.insert(file.path.clone()) {
            anyhow::bail!(
                "Scaffold plan contains duplicate target {}",
                file.path.display()
            );
        }
        ensure_no_symlink_ancestors(root, &file.path)?;
        match &file.original {
            None => match fs::symlink_metadata(&file.path) {
                Ok(_) => anyhow::bail!(
                    "Scaffold create target already exists: {}",
                    file.path.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Failed to inspect {}", file.path.display()));
                }
            },
            Some(_) => {
                let metadata = fs::symlink_metadata(&file.path).with_context(|| {
                    format!("Registration target is missing: {}", file.path.display())
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    anyhow::bail!(
                        "Registration target must be a regular file: {}",
                        file.path.display()
                    );
                }
            }
        }
    }

    Ok(plan)
}

fn print_scaffold_plan(root: &Path, plan: &[PlannedFile]) {
    for file in plan {
        let display_path = file.path.strip_prefix(root).unwrap_or(&file.path);
        let action = if file.original.is_some() {
            "UPDATE"
        } else {
            "CREATE"
        };
        println!("  [{action}] {}", display_path.display());
    }
}

pub fn scaffold(options: &ScaffoldOptions) -> Result<()> {
    // This renderer has native-only composition semantics. Never allow a
    // support crate or Component Model guest into native module registration.
    if options.profile != options::ScaffoldProfile::NativeModule {
        anyhow::bail!("cargo xtask scaffold only generates native runtime modules");
    }
    // There is no safe generic owner schema or UI business contract. Fail
    // before writing anything instead of registering fake storage or routes.
    if options.with_db {
        anyhow::bail!("--with-db requires an actual owner schema and migration contract; native scaffolding currently supports --no-db only");
    }
    // Generating an editable Leptos FFA package is not publishing a working
    // UI. Requested crates join Cargo workspace only; host mounting and
    // manifest [provides.*_ui] metadata are deferred to module owners.
    if options.ui != ScaffoldUi::None {
        println!("  [DRAFT FFA] Creating Leptos package skeletons: future_ui / not_started; no host route or CRUD");
    }

    let root = workspace_root();
    let slug = &options.slug;
    let pascal_name = to_pascal_case(slug);
    let snake_name = slug.clone();
    let crate_name = options.crate_name();
    let module_dir = root.join("crates").join("modules").join(&crate_name);

    println!("Scaffolding module '{}' ({})...", slug, crate_name);
    let plan = build_scaffold_plan(&root, &module_dir, options, &pascal_name, &snake_name)?;

    if options.dry_run {
        println!("  [DRY-RUN] Target directory: {}", module_dir.display());
        print_scaffold_plan(&root, &plan);
        println!("\n  [DRY-RUN] Preview includes source files and every registration manifest; no files were changed.");
        return Ok(());
    }

    let transaction = ScaffoldTransaction::apply(&root, &plan, &module_dir)?;
    print_scaffold_plan(&root, &plan);
    println!("  Registration plan applied across workspace manifests and docs.");

    let validation = match validate_scaffold(slug, options.skip_validate) {
        Ok(state) => state,
        Err(error) => {
            return match transaction.rollback() {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(error).context(format!(
                    "Scaffold validation failed and rollback also failed: {rollback_error}"
                )),
            };
        }
    };

    match validation {
        ScaffoldValidation::Verified => {
            println!("\n  SCAFFOLDED: Module '{slug}' registered; architecture lint and publish metadata checks passed. Business/UI readiness remains not_started.");
            println!("  Next steps:");
            println!("    cargo check -p {crate_name}");
            if options.ui.has_admin() {
                println!("    cargo check -p {crate_name}-admin");
            }
            if options.ui.has_storefront() {
                println!("    cargo check -p {crate_name}-storefront");
            }
        }
        ScaffoldValidation::Skipped => {
            println!(
                "\n  REGISTERED BUT UNVERIFIED: validation was explicitly skipped for '{slug}'."
            );
            println!("  Run: cargo xtask module lint {slug}");
            println!("  Run: cargo xtask module validate {slug}");
        }
    }

    transaction.commit();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_scaffold_plan, scaffold, PlannedFile, ScaffoldOptions, ScaffoldProfile,
        ScaffoldTransaction, ScaffoldUi,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustok-scaffold-transaction-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn options() -> ScaffoldOptions {
        ScaffoldOptions {
            profile: ScaffoldProfile::NativeModule,
            slug: "example".into(),
            title: "Example".into(),
            description: "An example native runtime identity".into(),
            category: "collaboration".into(),
            with_db: false,
            ui: ScaffoldUi::None,
            dry_run: true,
            skip_server_wire: false,
            skip_validate: false,
        }
    }

    fn copy_scaffold_inputs(root: &Path) {
        let workspace = crate::manifest_io::workspace_root();
        for relative in [
            "Cargo.toml",
            "modules.toml",
            "docs/modules/registry.md",
            "docs/modules/_index.md",
            "docs/modules/implementation-plans-registry.md",
            "docs/modules/UI_PACKAGES_INDEX.md",
            "apps/server/Cargo.toml",
            "crates/modules/rustok-distribution/Cargo.toml",
            "crates/modules/rustok-distribution/src/lib.rs",
        ] {
            let source = workspace.join(relative);
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(&source, &destination)
                .unwrap_or_else(|error| panic!("failed to stage {relative}: {error}"));
        }
    }

    fn plan_path_snapshot(root: &Path, plan: &[PlannedFile]) -> String {
        let paths = plan
            .iter()
            .map(|file| {
                file.path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();
        paths.join("\n") + "\n"
    }

    #[test]
    fn complete_scaffold_plans_match_golden_file_trees_for_every_ui_mode() {
        let root = temp_root();
        copy_scaffold_inputs(&root);

        let scenarios = [
            (
                "scaffold_golden_headless",
                ScaffoldUi::None,
                include_str!("goldens/plan-headless.golden"),
            ),
            (
                "scaffold_golden_admin",
                ScaffoldUi::Admin,
                include_str!("goldens/plan-admin.golden"),
            ),
            (
                "scaffold_golden_storefront",
                ScaffoldUi::Storefront,
                include_str!("goldens/plan-storefront.golden"),
            ),
            (
                "scaffold_golden_dual",
                ScaffoldUi::Dual,
                include_str!("goldens/plan-dual.golden"),
            ),
        ];

        for (slug, ui, expected) in scenarios {
            let mut request = options();
            request.slug = slug.to_string();
            request.title = format!("Golden {slug}");
            request.description =
                "Golden fixture for native module scaffold generation".to_string();
            request.ui = ui;

            let module_dir = root.join("crates/modules").join(request.crate_name());
            let pascal = crate::manifest_validation::to_pascal_case(slug);
            let plan = build_scaffold_plan(&root, &module_dir, &request, &pascal, slug)
                .unwrap_or_else(|error| panic!("failed to build {slug} plan: {error}"));
            let actual = plan_path_snapshot(&root, &plan);

            assert_eq!(
                actual, expected,
                "generated file tree changed for scaffold mode {slug}"
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scaffold_validation_runs_architecture_before_publish_metadata() {
        let calls = std::cell::RefCell::new(Vec::new());
        let state = super::validate_scaffold_with(
            "example",
            false,
            |args| {
                assert_eq!(args, &[String::from("example")]);
                calls.borrow_mut().push("lint");
                Ok(())
            },
            |args| {
                assert_eq!(args, &["example"]);
                calls.borrow_mut().push("validate");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(state, super::ScaffoldValidation::Verified);
        assert_eq!(calls.borrow().as_slice(), &["lint", "validate"][..]);
    }

    #[test]
    fn publish_validation_failure_does_not_claim_verified() {
        let calls = std::cell::RefCell::new(Vec::new());
        let state = super::validate_scaffold_with(
            "example",
            false,
            |_| {
                calls.borrow_mut().push("lint");
                Ok(())
            },
            |_| {
                calls.borrow_mut().push("validate");
                anyhow::bail!("publish failure")
            },
        );
        let error = state.unwrap_err().to_string();
        assert_eq!(calls.borrow().as_slice(), &["lint", "validate"][..]);
        assert!(error.contains("publish-readiness validation failed"));
        assert!(error.contains("NOT verified"));
    }

    #[test]
    fn skipped_scaffold_validation_never_claims_verified() {
        let state = super::validate_scaffold_with(
            "example",
            true,
            |_| panic!("lint must be skipped"),
            |_| panic!("publish validation must be skipped"),
        )
        .unwrap();
        assert_eq!(state, super::ScaffoldValidation::Skipped);
    }

    #[test]
    fn architecture_lint_failure_stops_before_publish_validation() {
        let state = super::validate_scaffold_with(
            "example",
            false,
            |_| anyhow::bail!("architecture failure"),
            |_| panic!("publish validation must not run after lint failure"),
        );
        let error = state.unwrap_err().to_string();
        assert!(error.contains("architecture lint failed"));
        assert!(error.contains("NOT verified"));
    }

    #[test]
    fn scaffold_transaction_restores_manifests_and_removes_created_crate_on_rollback() {
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(root.join("crates/modules")).unwrap();
        let manifest = root.join("modules.toml");
        fs::write(&manifest, "before\n").unwrap();

        let plan = vec![
            PlannedFile {
                path: module_dir.join("src/lib.rs"),
                content: "pub struct Example;\n".into(),
                original: None,
            },
            PlannedFile {
                path: manifest.clone(),
                content: "after\n".into(),
                original: Some(b"before\n".to_vec()),
            },
        ];

        let transaction = ScaffoldTransaction::apply(&root, &plan, &module_dir).unwrap();
        assert_eq!(fs::read_to_string(&manifest).unwrap(), "after\n");
        assert!(module_dir.join("src/lib.rs").is_file());

        transaction.rollback().unwrap();
        assert_eq!(fs::read_to_string(&manifest).unwrap(), "before\n");
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_registration_plan_fails_before_any_workspace_write() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let module_dir = root.join("crates/modules/rustok-example");
        let plan =
            build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").unwrap();
        let manifest = root.join("modules.toml");
        let changed = fs::read_to_string(&manifest).unwrap() + "\n# external edit\n";
        fs::write(&manifest, &changed).unwrap();
        let error = ScaffoldTransaction::apply(&root, &plan, &module_dir)
            .err()
            .unwrap();
        assert!(error.to_string().contains("changed since planning"));
        assert_eq!(fs::read_to_string(manifest).unwrap(), changed);
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_directory_created_after_planning_is_never_owned_or_removed() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let module_dir = root.join("crates/modules/rustok-example");
        let plan =
            build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").unwrap();
        fs::create_dir(&module_dir).unwrap();
        let owner_file = module_dir.join("owner.txt");
        fs::write(&owner_file, "external work").unwrap();
        assert!(ScaffoldTransaction::apply(&root, &plan, &module_dir).is_err());
        assert_eq!(fs::read_to_string(owner_file).unwrap(), "external work");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_preserves_external_edits_and_untracked_files() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let module_dir = root.join("crates/modules/rustok-example");
        let plan =
            build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").unwrap();
        let transaction = ScaffoldTransaction::apply(&root, &plan, &module_dir).unwrap();
        let manifest = root.join("modules.toml");
        fs::write(&manifest, "external manifest edit").unwrap();
        let owner_file = module_dir.join("owner.txt");
        fs::write(&owner_file, "external work").unwrap();
        let error = transaction.rollback().unwrap_err();
        assert!(error
            .to_string()
            .contains("Rollback preserved changed target"));
        assert_eq!(
            fs::read_to_string(manifest).unwrap(),
            "external manifest edit"
        );
        assert_eq!(fs::read_to_string(owner_file).unwrap(), "external work");
        assert!(!module_dir.join("src/lib.rs").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mid_apply_failure_restores_only_written_files_and_owned_directories() {
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(module_dir.parent().unwrap()).unwrap();
        let manifest = root.join("modules.toml");
        fs::write(&manifest, "before").unwrap();
        let plan = vec![
            PlannedFile {
                path: manifest.clone(),
                content: "after".into(),
                original: Some(b"before".to_vec()),
            },
            PlannedFile {
                path: module_dir.join("src"),
                content: "a file blocking the next parent".into(),
                original: None,
            },
            PlannedFile {
                path: module_dir.join("src/lib.rs"),
                content: "unreachable".into(),
                original: None,
            },
        ];
        assert!(ScaffoldTransaction::apply(&root, &plan, &module_dir).is_err());
        assert_eq!(fs::read_to_string(manifest).unwrap(), "before");
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_write_failure_restores_the_original_manifest() {
        use std::io::Write;
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(module_dir.parent().unwrap()).unwrap();
        let manifest = root.join("modules.toml");
        fs::write(&manifest, "original manifest").unwrap();
        let plan = vec![PlannedFile {
            path: manifest.clone(),
            content: "replacement manifest".into(),
            original: Some(b"original manifest".to_vec()),
        }];
        let mut calls = 0;
        let result =
            ScaffoldTransaction::apply_with_writer(&root, &plan, &module_dir, |output, content| {
                calls += 1;
                if calls == 1 {
                    output.write(&content[..3])
                } else {
                    Err(std::io::Error::other("injected write failure"))
                }
            });
        assert!(result.is_err());
        assert_eq!(calls, 2);
        assert_eq!(fs::read_to_string(manifest).unwrap(), "original manifest");
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn suffix_and_comment_matches_do_not_replace_a_real_module_key() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let manifest = root.join("modules.toml");
        fs::write(&manifest, "[modules]\nsome_example = { crate = \"rustok-some-example\" }\n# example = a comment, not a module\n").unwrap();
        let plan = build_scaffold_plan(
            &root,
            &root.join("crates/modules/rustok-example"),
            &options(),
            "Example",
            "example",
        )
        .unwrap();
        let planned = &plan
            .iter()
            .find(|file| file.path == manifest)
            .unwrap()
            .content;
        let parsed: toml::Value = toml::from_str(planned).unwrap();
        assert!(parsed["modules"].get("example").is_some());
        assert!(parsed["modules"].get("some_example").is_some());
        assert!(!fs::read_to_string(manifest)
            .unwrap()
            .contains("crate = \"rustok-example\""));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_composition_anchors_fail_during_preflight_even_when_validation_is_skipped() {
        for relative in [
            "Cargo.toml",
            "apps/server/Cargo.toml",
            "crates/modules/rustok-distribution/src/lib.rs",
        ] {
            let root = temp_root();
            copy_scaffold_inputs(&root);
            fs::write(
                root.join(relative),
                "# missing required registration anchors\n",
            )
            .unwrap();
            let before = fs::read(root.join("modules.toml")).unwrap();
            let mut request = options();
            request.skip_validate = true;
            let module_dir = root.join("crates/modules/rustok-example");
            assert!(
                build_scaffold_plan(&root, &module_dir, &request, "Example", "example").is_err(),
                "accepted missing {relative}"
            );
            assert_eq!(fs::read(root.join("modules.toml")).unwrap(), before);
            assert!(!module_dir.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn default_composition_requires_both_distribution_files() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        fs::remove_file(root.join("crates/modules/rustok-distribution/Cargo.toml")).unwrap();
        let module_dir = root.join("crates/modules/rustok-example");
        assert!(build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").is_err());
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_preflight_rejects_an_existing_module_directory() {
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(&module_dir).unwrap();

        let error =
            build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").unwrap_err();
        assert!(error.to_string().contains("already exists"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_preflight_rejects_an_existing_registry_slug() {
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(module_dir.parent().unwrap()).unwrap();
        fs::write(
            root.join("modules.toml"),
            "[modules]\nexample = { crate = \"rustok-example\" }\n",
        )
        .unwrap();

        let error =
            build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").unwrap_err();
        assert!(error.to_string().contains("already registered"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn database_requests_fail_even_in_preview_mode() {
        let mut request = options();
        request.with_db = true;
        assert!(scaffold(&request)
            .unwrap_err()
            .to_string()
            .contains("--with-db"));
    }

    #[test]
    fn every_requested_ui_mode_has_a_writable_render_plan() {
        let mut request = options();
        for mode in [ScaffoldUi::Admin, ScaffoldUi::Storefront, ScaffoldUi::Dual] {
            request.ui = mode;
            let planned = super::ui::render_ui_templates(&request, "Example");
            assert!(!planned.is_empty());
            assert!(planned
                .iter()
                .any(|(path, _)| path.ends_with("src/ui/leptos.rs")));
            assert!(
                scaffold(&request).is_ok(),
                "dry-run must preview every UI mode"
            );
        }
    }
}
