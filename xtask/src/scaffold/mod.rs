use crate::manifest_io::workspace_root;
use crate::manifest_validation::{module_validate_command, to_pascal_case};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

mod backend;
mod options;
mod registration;
mod ui;

#[allow(unused_imports)]
pub use options::{print_scaffold_usage, ScaffoldOptions, ScaffoldUi};

pub(crate) fn write_file_content(path: &Path, content: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("  [CREATE] {}", path.display());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    fs::write(path, content)
        .with_context(|| format!("Failed to write file {}", path.display()))?;
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

pub fn scaffold(options: &ScaffoldOptions) -> Result<()> {
    let root = workspace_root();
    let slug = &options.slug;
    let pascal_name = to_pascal_case(slug);
    let snake_name = slug.replace('-', "_");
    let crate_name = format!("rustok-{slug}");
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

    // 1. Generate backend files (FBA)
    backend::generate_backend(options, &module_dir, &pascal_name, &snake_name)?;

    // 2. Generate UI packages (FFA: core, transport, ui, i18n)
    ui::generate_ui(options, &module_dir, &pascal_name)?;

    // 3. Update manifests and global registration
    if !options.dry_run {
        registration::register_module(&root, options, &pascal_name, &snake_name)?;
        println!("  Registration complete across workspace manifests and docs.");

        // 4. Run post-scaffold validation if not skipped
        if !options.skip_validate {
            println!("\nVerifying module publish-readiness contracts for '{slug}'...");
            match module_validate_command(std::slice::from_ref(slug)) {
                Ok(_) => {
                    println!("\n  SUCCESS: Module '{slug}' created and verified!");
                    println!("  Next steps:");
                    println!("    cargo check -p {crate_name}");
                    if options.ui.has_admin() {
                        println!("    cargo check -p {crate_name}-admin");
                    }
                    if options.ui.has_storefront() {
                        println!("    cargo check -p {crate_name}-storefront");
                    }
                }
                Err(err) => {
                    println!("\n  WARNING: Module files created, but verification reported:");
                    println!("    {err:#}");
                }
            }
        }
    } else {
        println!("\n  [DRY-RUN] Completed preview without disk modifications.");
    }

    Ok(())
}
