use crate::manifest_io::workspace_root;
use crate::manifest_validation::{module_validate_command, to_pascal_case};
use crate::module_commands::module_lint_command;
use anyhow::{Context, Result};
use std::fs;
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

pub(crate) fn write_file_content(path: &Path, content: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("  [CREATE] {}", path.display());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    fs::write(path, content).with_context(|| format!("Failed to write file {}", path.display()))?;
    println!("  Created {}", path.display());
    Ok(())
}

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

    if module_dir.exists() && !options.dry_run {
        anyhow::bail!(
            "Module crate directory already exists at {}",
            module_dir.display()
        );
    }

    if !options.dry_run {
        let modules_toml_path = root.join("modules.toml");
        if let Ok(content) = fs::read_to_string(&modules_toml_path) {
            let needle = format!("{slug} =");
            let needle_sp = format!("{slug} ");
            if content.lines().any(|l| {
                let trimmed = l.trim();
                trimmed.starts_with(&needle) || trimmed.starts_with(&needle_sp)
            }) {
                anyhow::bail!("Module '{slug}' is already registered in modules.toml");
            }
        }
    }

    println!("Scaffolding module '{}' ({})...", slug, crate_name);
    if options.dry_run {
        println!("  [DRY-RUN] Target directory: {}", module_dir.display());
    }

    // 1. Generate a native runtime identity only (no FBA readiness claim)
    backend::generate_backend(options, &module_dir, &pascal_name, &snake_name)?;

    // 2. Create requested Leptos FFA development packages; no host mounting.
    ui::generate_ui(options, &module_dir, &pascal_name)?;

    // 3. Update manifests and global registration
    if !options.dry_run {
        registration::register_module(&root, options, &pascal_name, &snake_name)?;
        println!("  Registration complete across workspace manifests and docs.");

        // 4. Run the same architecture lint users can invoke directly,
        // then check publish metadata. A skip is reported as unverified.
        match validate_scaffold(slug, options.skip_validate)? {
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
    } else {
        println!("\n  [DRY-RUN] Completed preview without disk modifications.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{scaffold, ScaffoldOptions, ScaffoldUi};
    use super::options::ScaffoldProfile;

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
