use crate::manifest_io::workspace_root;
use crate::manifest_validation::{module_validate_command, to_pascal_case};
use crate::module_commands::module_lint_command;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::path::Path;

mod backend;
mod options;
pub(crate) mod registration;
mod ui;

#[allow(unused_imports)]
pub use options::{
    default_title_from_slug, is_valid_slug, parse_options, print_scaffold_usage, ScaffoldOptions,
    ScaffoldUi,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannedFileKind {
    Create,
    Update,
}

#[derive(Debug, Clone)]
struct PlannedFile {
    path: PathBuf,
    content: String,
    kind: PlannedFileKind,
}

struct FileSnapshot {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

struct ScaffoldTransaction {
    snapshots: Vec<FileSnapshot>,
    module_dir: PathBuf,
}

impl ScaffoldTransaction {
    fn apply(plan: &[PlannedFile], module_dir: &Path) -> Result<Self> {
        let mut snapshots = Vec::with_capacity(plan.len());
        for file in plan {
            let original = match fs::read(&file.path) {
                Ok(content) => Some(content),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("Failed to snapshot scaffold target {}", file.path.display())
                    });
                }
            };
            snapshots.push(FileSnapshot {
                path: file.path.clone(),
                original,
            });
        }

        let transaction = Self {
            snapshots,
            module_dir: module_dir.to_path_buf(),
        };

        for file in plan {
            let write_result = (|| -> Result<()> {
                if let Some(parent) = file.path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("Failed to create {}", parent.display()))?;
                }
                fs::write(&file.path, file.content.as_bytes())
                    .with_context(|| format!("Failed to write {}", file.path.display()))?;
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

    fn rollback(self) -> Result<()> {
        let mut failures = Vec::new();
        for snapshot in self.snapshots.into_iter().rev() {
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

        match fs::remove_dir_all(&self.module_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!(
                "Failed to remove generated module directory {}: {error}",
                self.module_dir.display()
            )),
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
    let needle = format!("{} =", options.slug);
    let needle_sp = format!("{} ", options.slug);
    if modules_content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&needle) || trimmed.starts_with(&needle_sp)
    }) {
        anyhow::bail!("Module '{}' is already registered in modules.toml", options.slug);
    }

    let mut plan = Vec::new();
    for (relative, content) in backend::render_backend(options, pascal_name)
        .into_iter()
        .chain(ui::render_ui_templates(options, pascal_name))
    {
        plan.push(PlannedFile {
            path: module_dir.join(relative),
            content,
            kind: PlannedFileKind::Create,
        });
    }

    for change in registration::plan_module_registration(root, options, pascal_name, snake_name)? {
        plan.push(PlannedFile {
            path: change.path,
            content: change.content,
            kind: PlannedFileKind::Update,
        });
    }

    let mut seen = HashSet::new();
    for file in &plan {
        if !file.path.starts_with(root) {
            anyhow::bail!("Scaffold target escapes workspace: {}", file.path.display());
        }
        if !seen.insert(file.path.clone()) {
            anyhow::bail!("Scaffold plan contains duplicate target {}", file.path.display());
        }
        ensure_no_symlink_ancestors(root, &file.path)?;
        match file.kind {
            PlannedFileKind::Create => match fs::symlink_metadata(&file.path) {
                Ok(_) => anyhow::bail!("Scaffold create target already exists: {}", file.path.display()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("Failed to inspect {}", file.path.display())
                    });
                }
            },
            PlannedFileKind::Update => {
                let metadata = fs::symlink_metadata(&file.path)
                    .with_context(|| format!("Registration target is missing: {}", file.path.display()))?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    anyhow::bail!("Registration target must be a regular file: {}", file.path.display());
                }
            }
        }
    }

    Ok(plan)
}

fn print_scaffold_plan(root: &Path, plan: &[PlannedFile]) {
    for file in plan {
        let display_path = file.path.strip_prefix(root).unwrap_or(&file.path);
        let action = match file.kind {
            PlannedFileKind::Create => "CREATE",
            PlannedFileKind::Update => "UPDATE",
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
    let plan = build_scaffold_plan(
        &root,
        &module_dir,
        options,
        &pascal_name,
        &snake_name,
    )?;

    if options.dry_run {
        println!("  [DRY-RUN] Target directory: {}", module_dir.display());
        print_scaffold_plan(&root, &plan);
        println!("\n  [DRY-RUN] Preview includes source files and every registration manifest; no files were changed.");
        return Ok(());
    }

    let transaction = ScaffoldTransaction::apply(&plan, &module_dir)?;
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
            println!("\n  REGISTERED BUT UNVERIFIED: validation was explicitly skipped for '{slug}'.");
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
        build_scaffold_plan, scaffold, PlannedFile, PlannedFileKind, ScaffoldOptions,
        ScaffoldTransaction, ScaffoldUi,
    };
    use super::options::ScaffoldProfile;
    use std::fs;
    use std::path::PathBuf;
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

    #[test]
    fn scaffold_validation_runs_architecture_before_publish_metadata() {
        let mut calls = Vec::new();
        let state = super::validate_scaffold_with(
            "example",
            false,
            |args| {
                assert_eq!(args, &[String::from("example")]);
                calls.push("lint");
                Ok(())
            },
            |args| {
                assert_eq!(args, &["example"]);
                calls.push("validate");
                Ok(())
            },
        ).unwrap();
        assert_eq!(state, super::ScaffoldValidation::Verified);
        assert_eq!(calls, vec!["lint", "validate"]);
    }

    #[test]
    fn publish_validation_failure_does_not_claim_verified() {
        let mut calls = Vec::new();
        let state = super::validate_scaffold_with(
            "example",
            false,
            |_| {
                calls.push("lint");
                Ok(())
            },
            |_| {
                calls.push("validate");
                anyhow::bail!("publish failure")
            },
        );
        let error = state.unwrap_err().to_string();
        assert_eq!(calls, vec!["lint", "validate"]);
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
        ).unwrap();
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
        fs::write(&manifest, "before\\n").unwrap();

        let plan = vec![
            PlannedFile {
                path: module_dir.join("src/lib.rs"),
                content: "pub struct Example;\\n".into(),
                kind: PlannedFileKind::Create,
            },
            PlannedFile {
                path: manifest.clone(),
                content: "after\\n".into(),
                kind: PlannedFileKind::Update,
            },
        ];

        let transaction = ScaffoldTransaction::apply(&plan, &module_dir).unwrap();
        assert_eq!(fs::read_to_string(&manifest).unwrap(), "after\\n");
        assert!(module_dir.join("src/lib.rs").is_file());

        transaction.rollback().unwrap();
        assert_eq!(fs::read_to_string(&manifest).unwrap(), "before\\n");
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_preflight_rejects_an_existing_module_directory() {
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(&module_dir).unwrap();

        let error = build_scaffold_plan(&root, &module_dir, &options(), "Example", "example")
            .unwrap_err();
        assert!(error.to_string().contains("already exists"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_preflight_rejects_an_existing_registry_slug() {
        let root = temp_root();
        let module_dir = root.join("crates/modules/rustok-example");
        fs::create_dir_all(module_dir.parent().unwrap()).unwrap();
        fs::write(root.join("modules.toml"), "[modules]\\nexample = { crate = \\"rustok-example\\" }\\n")
            .unwrap();

        let error = build_scaffold_plan(&root, &module_dir, &options(), "Example", "example")
            .unwrap_err();
        assert!(error.to_string().contains("already registered"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn database_requests_fail_even_in_preview_mode() {
        let mut request = options();
        request.with_db = true;
        assert!(scaffold(&request).unwrap_err().to_string().contains("--with-db"));
    }

    #[test]
    fn every_requested_ui_mode_has_a_writable_render_plan() {
        let mut request = options();
        for mode in [ScaffoldUi::Admin, ScaffoldUi::Storefront, ScaffoldUi::Dual] {
            request.ui = mode;
            let planned = super::ui::render_ui_templates(
                &request, "Example"
            );
            assert!(!planned.is_empty());
            assert!(planned.iter().any(|(path, _)| path.ends_with("src/ui/leptos.rs")));
            assert!(scaffold(&request).is_ok(), "dry-run must preview every UI mode");
        }
    }
}
