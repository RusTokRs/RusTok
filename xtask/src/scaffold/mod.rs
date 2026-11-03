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
mod preview;
pub(crate) mod registration;
mod registration_toml;
mod transaction;
mod ui;

#[cfg(test)]
use transaction::ScaffoldTransaction;
use transaction::ScaffoldWorkspace;

#[allow(unused_imports)]
pub use options::{
    default_title_from_slug, is_valid_slug, parse_options, print_scaffold_usage, ScaffoldOptions,
    ScaffoldProfile, ScaffoldUi,
};

pub(crate) fn scaffold_command(args: &[String]) -> Result<()> {
    if args.first().is_some_and(|argument| argument == "--recover") {
        anyhow::ensure!(
            args.len() == 1,
            "scaffold --recover accepts no module or additional options"
        );
        ScaffoldWorkspace::recover_command(&workspace_root())?;
        println!("Scaffold recovery complete.");
        return Ok(());
    }
    let options = match options::parse_options(args)? {
        Some(opts) => opts,
        None => return Ok(()),
    };

    scaffold(&options, std::io::stdout())
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

fn ensure_no_symlink_ancestors(root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("Scaffold target escapes workspace: {}", path.display()))?;
    anyhow::ensure!(
        relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_))),
        "Scaffold target contains a non-normal path component: {}",
        path.display()
    );
    let mut current = root.to_path_buf();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            current.push(component);
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link(&metadata) => {
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

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
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

pub fn scaffold(options: &ScaffoldOptions, preview_output: impl Write) -> Result<()> {
    scaffold_in_workspace(
        &workspace_root(),
        options,
        preview_output,
        validate_scaffold,
    )
}

fn scaffold_in_workspace(
    root: &Path,
    options: &ScaffoldOptions,
    mut preview_output: impl Write,
    validate: impl FnOnce(&str, bool) -> Result<ScaffoldValidation>,
) -> Result<()> {
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

    let preview_guard = if options.dry_run {
        ScaffoldWorkspace::preview(root)?
    } else {
        None
    };
    let workspace = if options.dry_run {
        None
    } else {
        let workspace = ScaffoldWorkspace::acquire(root)?;
        workspace.recover()?;
        Some(workspace)
    };
    let slug = &options.slug;
    let pascal_name = to_pascal_case(slug);
    let snake_name = slug.clone();
    let crate_name = options.crate_name();
    let module_dir = root.join("crates").join("modules").join(&crate_name);

    println!("Scaffolding module '{}' ({})...", slug, crate_name);
    let plan = build_scaffold_plan(root, &module_dir, options, &pascal_name, &snake_name)?;

    if options.dry_run {
        drop(preview_guard);
        println!("  [DRY-RUN] Target directory: {}", module_dir.display());
        preview::write_preview(&mut preview_output, root, &plan)?;
        preview_output.flush()?;
        println!("\n  [DRY-RUN] Preview includes source files and every registration manifest; no files were changed.");
        return Ok(());
    }

    let workspace = workspace.context("Missing live scaffold workspace guard")?;
    workspace.retain_runner()?;
    let transaction = workspace.apply(&plan, &module_dir)?;
    print_scaffold_plan(root, &plan);
    println!("  Registration plan applied across workspace manifests and docs.");

    let validation = match validate(slug, options.skip_validate) {
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

    transaction.commit()?;

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

    pub(super) fn options() -> ScaffoldOptions {
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

    pub(super) fn copy_scaffold_inputs(root: &Path) {
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
    fn dry_run_previews_the_same_content_twice_without_touching_workspace_bytes() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let options = options();
        let module_dir = root.join("crates/modules").join(options.crate_name());
        let index = root.join("docs/modules/_index.md");
        let crlf = fs::read_to_string(&index).unwrap().replace("\r\n", "\n");
        fs::write(&index, crlf.trim_end_matches('\n').replace('\n', "\r\n")).unwrap();
        let inputs: Vec<_> = super::registration::registration_target_paths(&root, &options)
            .into_iter()
            .map(|path| {
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        let plan = build_scaffold_plan(&root, &module_dir, &options, "Example", "example").unwrap();
        let index_change = plan.iter().find(|file| file.path == index).unwrap();
        assert!(!index_change.content.ends_with('\n'));
        assert!(index_change.content.contains("\r\n"));
        assert!(index_change
            .content
            .split_inclusive('\n')
            .all(|line| !line.ends_with('\n') || line.ends_with("\r\n")));
        let render = || {
            let mut output = Vec::new();
            super::scaffold_in_workspace(&root, &options, &mut output, |_, _| {
                panic!("dry-run must not run post-apply validation")
            })
            .unwrap();
            let output = String::from_utf8(output).unwrap();
            assert_eq!(
                output
                    .lines()
                    .filter(|line| line.starts_with("+++ b/"))
                    .count(),
                plan.len()
            );
            assert!(output.contains("+example = { crate = \"rustok-example\""));
            output
        };
        assert_eq!(render(), render());
        for (path, bytes) in inputs {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert!(!root.join(".rustok").exists());
        assert!(!module_dir.exists());
        fs::remove_dir_all(root).unwrap();
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
        assert!(error.to_string().contains("Rollback preserved"));
        assert_eq!(
            fs::read_to_string(manifest).unwrap(),
            "external manifest edit"
        );
        assert_eq!(fs::read_to_string(owner_file).unwrap(), "external work");
        assert!(module_dir.join("src/lib.rs").exists());
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
        fs::write(&manifest, "schema = 2\n[modules]\nsome_example = { crate = \"rustok-some-example\" }\n# example = a comment, not a module\n").unwrap();
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
    fn toml_registration_preserves_bytes_across_supported_layouts() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let workspace = "['workspace'] # workspace\r\n'members' = ['crates/modules/*'] # keep [this]\r\n\n[ workspace . 'dependencies' ] # EOF";
        let modules = "schema = 2\n'modules'.blog.crate = 'rustok-blog'\nmarker = 'example = a lookalike'\n'modules'.blog.path = 'crates/modules/rustok-blog' # keep\n";
        let server = "[package.metadata.'cargo-udeps'.ignore]\r\n'normal' = [\r\n\t'rustok-rbac' # keep [this]\r\n]\r\n\r\n[ 'features' ] # reorder\r\nmod-workflow = [\r\n    'dep:workflow' # keep\r\n]\r\n# mod-example = a lookalike\r\n[dependencies.workflow]\r\nworkspace = true\r\noptional = true";
        let distribution = "features = { 'mod-blog' = ['dep:blog'] }\nmarker = 'rustok-example = lookalike'\ndependencies = { blog.workspace = true, other = '1', blog.optional = true } # EOF";
        let fixtures = [
            ("Cargo.toml", workspace, workspace
                .replace("'crates/modules/*'", "'crates/modules/*', \"crates/modules/rustok-example/admin\", \"crates/modules/rustok-example/storefront\"")
                + "\nrustok-example = { path = \"crates/modules/rustok-example\" }"),
            ("modules.toml", modules, modules.to_owned()
                + "modules.example = { crate = \"rustok-example\", source = \"path\", path = \"crates/modules/rustok-example\" }\n"),
            (super::registration_toml::SERVER, server, server
                .replace("'rustok-rbac' # keep [this]\r\n]", "'rustok-rbac', # keep [this]\r\n\t\"rustok-example\"\r\n]")
                .replace("# mod-example", "mod-example = [\"dep:rustok-example\", \"rustok-distribution/mod-example\"]\r\n# mod-example")
                + "\r\n[dependencies]\r\nrustok-example = { workspace = true, optional = true }"),
            (super::registration_toml::DISTRIBUTION, distribution, distribution
                .replace("['dep:blog']", "['dep:blog'], mod-example = [\"dep:rustok-example\"]")
                .replace("blog.optional = true", "blog.optional = true, rustok-example = { workspace = true, optional = true }")),
        ];
        for (relative, original, _) in &fixtures {
            fs::write(root.join(relative), original).unwrap();
        }
        let mut request = options();
        request.ui = ScaffoldUi::Dual;
        let plan = build_scaffold_plan(
            &root,
            &root.join("crates/modules/rustok-example"),
            &request,
            "Example",
            "example",
        )
        .unwrap();
        for (relative, original, expected) in fixtures {
            let target = root.join(relative);
            let staged = plan.iter().find(|file| file.path == target).unwrap();
            assert_eq!(staged.content, expected, "{relative}");
            assert_eq!(fs::read_to_string(target).unwrap(), original);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn toml_conflicts_fail_before_apply_even_when_validation_is_skipped() {
        for (relative, input, reason) in [
            ("modules.toml", "schema = 2\nmodules = { other = { crate = 'rustok-example' } }", "already claims crate"),
            ("modules.toml", "schema = 2\n[modules.other]\ncrate = 'rustok-other'\npath = './crates/modules/../modules/rustok-example'", "already claims crate"),
            ("Cargo.toml", "workspace = { members = ['crates/modules/*'], exclude = ['crates/modules/rustok-example/admin/Cargo.toml'], dependencies = {} }", "excludes requested module path"),
            ("Cargo.toml", "workspace = { members = ['crates/modules/rustok-example', './crates/modules/rustok-example'], dependencies = {} }", "Duplicate Cargo workspace member"),
            ("Cargo.toml", "[workspace]\nmembers = []\n[workspace.dependencies]\n'rustok-example' = '1'", "already occupied"),
            (super::registration_toml::SERVER, "features = { 'mod-example' = ['dep:rustok-example'] }\ndependencies = {}", "already occupied"),
            (super::registration_toml::DISTRIBUTION, "features = {}\ndependencies.rustok-example.workspace = true", "already occupied"),
            (super::registration_toml::DISTRIBUTION, "[dependencies\n", "TOML parse error"),
        ] {
            let root = temp_root();
            copy_scaffold_inputs(&root);
            let target = root.join(relative);
            fs::write(&target, input).unwrap();
            let mut request = options();
            request.ui = ScaffoldUi::Admin;
            request.skip_validate = true;
            let originals = super::registration::registration_target_paths(&root, &request)
                .into_iter().map(|path| (fs::read(&path).unwrap(), path)).collect::<Vec<_>>();
            let module = root.join("crates/modules/rustok-example");
            let error = build_scaffold_plan(&root, &module, &request, "Example", "example").unwrap_err();
            assert!(format!("{error:#}").contains(reason), "{relative}: {error:#}");
            for (before, path) in originals {
                assert_eq!(fs::read(path).unwrap(), before);
            }
            assert!(!module.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn skipping_host_wiring_keeps_workspace_and_manifest_registration() {
        let root = temp_root();
        copy_scaffold_inputs(&root);
        let mut request = options();
        request.skip_server_wire = true;
        let mut targets = super::registration::registration_target_paths(&root, &options());
        for host in targets.split_off(5) {
            fs::remove_file(host).unwrap();
        }
        fs::write(
            root.join("Cargo.toml"),
            "workspace = { members = ['xtask'], dependencies = {} }",
        )
        .unwrap();
        fs::write(
            root.join("modules.toml"),
            "schema = 2\n[modules.existing]\ncrate = 'rustok-existing'",
        )
        .unwrap();
        let module = root.join("crates/modules/rustok-example");
        let plan = build_scaffold_plan(&root, &module, &request, "Example", "example").unwrap();
        let updates: Vec<_> = plan
            .iter()
            .filter(|file| file.original.is_some())
            .map(|file| &file.path)
            .collect();
        assert_eq!(updates, targets.iter().collect::<Vec<_>>());
        assert!(!module.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_membership_honors_cargo_exclusion_precedence() {
        let path = "crates/modules/rustok-example";
        let root = std::env::temp_dir();
        let included = |members: &[&str], excludes: &[&str]| {
            super::registration_toml::workspace_has_member(&root, members, excludes, path)
        };
        assert!(included(&["crates/modules/*"], &["crates/modules/rustok-*"]).unwrap());
        assert!(included(&[path], &["crates/modules"]).unwrap());
        assert!(included(&["crates/modules/*"], &["crates/modules"]).is_err());
        assert!(!included(&["crates/modules/rustok-[A-Z]*"], &[]).unwrap());
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
        copy_scaffold_inputs(&root);
        fs::write(
            root.join("modules.toml"),
            "schema = 2\n[modules]\nexample = { crate = \"rustok-example\" }\n",
        )
        .unwrap();

        let error =
            build_scaffold_plan(&root, &module_dir, &options(), "Example", "example").unwrap_err();
        assert!(format!("{error:#}").contains("already registered"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn database_requests_fail_even_in_preview_mode() {
        let mut request = options();
        request.with_db = true;
        assert!(scaffold(&request, std::io::sink())
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
                scaffold(&request, std::io::sink()).is_ok(),
                "dry-run must preview every UI mode"
            );
        }
    }
}
