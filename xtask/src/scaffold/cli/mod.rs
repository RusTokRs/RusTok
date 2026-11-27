use super::transaction::ScaffoldWorkspace;
use super::{ensure_no_symlink_ancestors, preview, PlannedFile};
use crate::manifest_io::{load_manifest_from, workspace_root};
use crate::manifest_validation::module_validate_command;
use crate::module_commands::lint_module_source;
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;

mod options;
mod registration;
mod render;
#[cfg(test)]
mod tests;

pub(super) use registration::is_registration_target;

pub(crate) const USAGE: &str = "cargo xtask module scaffold-cli <slug> --name <command> --summary <text> --adapter <file.rs> --tests <file.rs> [--dependency <workspace-key>] [--dev-dependency <workspace-key>] [--supports-dry-run] [--dry-run]";

pub(crate) fn command(args: &[String]) -> Result<()> {
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let options = options::Options::parse(args)?;
    let root = workspace_root();
    scaffold(&root, &options, std::io::stdout(), |slug| {
        lint_module_source(&root, slug, "--cli")?;
        registration::check_registry(&root)?;
        module_validate_command(&[slug.to_owned()])
    })
}

fn scaffold(
    root: &Path,
    options: &options::Options,
    mut preview_output: impl Write,
    validate: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
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
    let manifest = load_manifest_from(&root.join("modules.toml"))?;
    let (owner, owner_name) = registration::owner_path(options, &manifest)?;
    let cli_dir = root.join(&owner).join("cli");
    ensure_no_symlink_ancestors(root, &cli_dir)?;
    match fs::symlink_metadata(&cli_dir) {
        Ok(_) => bail!("CLI package already exists at {}", cli_dir.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("Cannot inspect {}", cli_dir.display()))
        }
    }
    let catalog: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    let dependencies = catalog
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .context("Cargo.toml is missing workspace.dependencies")?;
    let mut plan = render::files(options, &owner_name, dependencies)?
        .into_iter()
        .map(|(relative, content)| PlannedFile {
            path: cli_dir.join(relative),
            content,
            original: None,
        })
        .collect::<Vec<_>>();
    plan.extend(registration::plan(
        root,
        &manifest,
        &owner,
        &owner_name,
        &options.slug,
    )?);
    if options.dry_run {
        drop(preview_guard);
        preview::write_preview(&mut preview_output, root, &plan)?;
        preview_output.flush()?;
        return Ok(());
    }

    let workspace = workspace.context("Missing live CLI scaffold workspace guard")?;
    workspace.retain_runner()?;
    let transaction = workspace.apply(&plan, &cli_dir)?;
    if let Err(error) = validate(&options.slug) {
        return match transaction.rollback() {
            Ok(()) => {
                Err(error).context("CLI scaffold validation failed. Changes were rolled back.")
            }
            Err(rollback) => Err(error).context(format!(
                "CLI scaffold validation failed and recovery is required: {rollback}"
            )),
        };
    }
    transaction.commit()?;
    super::print_scaffold_plan(root, &plan);
    println!(
        "Registered CLI adapter for '{} {}'. Source and metadata checks passed.",
        options.slug, options.name
    );
    println!(
        "Run cargo test -p {owner_name}-cli --lib to compile and exercise the supplied contract."
    );
    Ok(())
}
