use super::options::Options;
use crate::scaffold::registration::TempWorkspace;
use crate::scaffold::registration_toml::{self, inline_table};
use crate::scaffold::{ensure_no_symlink_ancestors, PlannedFile};
use anyhow::{ensure, Context, Result};
use rustok_manifest::ModulesManifest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::Value;

const REGISTRY_MANIFEST: &str = "crates/utils/rustok-cli-registry/Cargo.toml";
const REGISTRY_SOURCE: &str = "crates/utils/rustok-cli-registry/src/generated.rs";
const GENERATOR: &str = "scripts/generate/generate-cli-registry.mjs";

pub(in crate::scaffold) fn is_registration_target(owner: &Path, target: &Path) -> bool {
    ["Cargo.toml", REGISTRY_MANIFEST, REGISTRY_SOURCE]
        .iter()
        .any(|path| target == Path::new(path))
        || target == owner.join("rustok-module.toml")
}

pub(super) fn owner_path(
    options: &Options,
    manifest: &ModulesManifest,
) -> Result<(PathBuf, String)> {
    let owner = manifest
        .modules
        .get(&options.slug)
        .with_context(|| format!("Unknown registered module '{}'", options.slug))?;
    let path = Path::new("crates/modules").join(&owner.crate_name);
    let canonical_name = owner.crate_name.strip_prefix("rustok-");
    ensure!(
        owner.source == "path"
            && owner.runtime != "extension"
            && canonical_name.is_some_and(|name| {
                !name.contains('_') && crate::scaffold::is_valid_slug(&name.replace('-', "_"))
            })
            && owner.path.as_deref().map(Path::new) == Some(path.as_path()),
        "Expected a registered native owner under crates/modules/<package>"
    );
    Ok((path, owner.crate_name.clone()))
}

fn stage(root: &Path, temporary: &Path, relative: &Path) -> Result<Option<String>> {
    let path = root.join(relative);
    ensure_no_symlink_ancestors(root, &path)?;
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("Cannot stage {}", path.display()))
        }
    };
    let staged = temporary.join(relative);
    fs::create_dir_all(staged.parent().context("Missing staged path parent")?)?;
    fs::write(&staged, &source)?;
    Ok(Some(source))
}

fn generate_registry(staged: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("node")
        .arg(staged.join(GENERATOR))
        .args(args)
        .current_dir(staged)
        .output()
        .context("Cannot execute the CLI registry generator")?;
    ensure!(
        output.status.success(),
        "CLI registry generation failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

pub(super) fn check_registry(root: &Path) -> Result<()> {
    generate_registry(root, &["--check"])
}

pub(super) fn plan(
    root: &Path,
    manifest: &ModulesManifest,
    owner: &Path,
    owner_name: &str,
    slug: &str,
) -> Result<Vec<PlannedFile>> {
    let temporary = TempWorkspace::new()?;
    let staged = &temporary.root;
    let owner_manifest = owner.join("rustok-module.toml");
    let targets = [
        PathBuf::from("Cargo.toml"),
        owner_manifest.clone(),
        PathBuf::from(REGISTRY_MANIFEST),
        PathBuf::from(REGISTRY_SOURCE),
    ];
    let mut originals = Vec::with_capacity(targets.len());
    for target in &targets {
        let source = stage(root, staged, target)?
            .with_context(|| format!("Missing CLI registration file {}", target.display()))?;
        originals.push(source);
    }
    for relative in ["modules.toml", GENERATOR] {
        stage(root, staged, Path::new(relative))?
            .with_context(|| format!("Missing CLI registry input {relative}"))?;
    }
    stage(root, staged, Path::new("cli-registry.toml"))?;
    // Materialize the registry generator's inputs in the shadow workspace.
    // Each candidate manifest has the original relative path.
    for entry in manifest.modules.values() {
        let Some(relative) = entry.path.as_deref().map(Path::new) else {
            continue;
        };
        if relative != owner {
            stage(root, staged, &relative.join("rustok-module.toml"))?;
        }
    }
    check_registry(staged)?;

    let source = &originals[1];
    let package = rustok_manifest::parse_module_package_manifest(source)?;
    ensure!(
        package.module.slug == slug,
        "CLI owner manifest slug does not match '{slug}'"
    );
    ensure!(
        package.provides.cli.is_none(),
        "Module '{slug}' already declares CLI commands"
    );
    let cli_name = format!("{owner_name}-cli");
    let factory = format!("{}::command_provider", cli_name.replace('-', "_"));
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let next = format!(
        "{}{newline}{newline}[provides.cli]{newline}namespace = \"{slug}\"{newline}factory = \"{factory}\"{newline}",
        source.trim_end_matches(['\r', '\n']),
    );
    let package = rustok_manifest::parse_module_package_manifest(&next)?;
    rustok_manifest::validate_module_package_manifest(&package)?;
    fs::write(staged.join(owner_manifest), next)?;

    let cli_path = owner.join("cli").to_string_lossy().replace('\\', "/");
    registration_toml::edit(staged, "Cargo.toml", |editor| {
        let members = editor.strings(&["workspace", "members"])?;
        let excludes = if editor.get(&["workspace", "exclude"]).is_some() {
            editor.strings(&["workspace", "exclude"])?
        } else {
            Vec::new()
        };
        if !registration_toml::workspace_has_member(root, &members, &excludes, &cli_path)? {
            editor.append(&["workspace", "members"], std::slice::from_ref(&cli_path))?;
        }
        editor.insert(
            &["workspace", "dependencies"],
            &cli_name,
            &inline_table([("path", Value::from(cli_path.as_str()))]),
        )
    })?;
    registration_toml::edit(staged, REGISTRY_MANIFEST, |editor| {
        editor.insert(
            &["dependencies"],
            &cli_name,
            &inline_table([("workspace", Value::from(true))]),
        )
    })?;

    generate_registry(staged, &[])?;
    let generated = fs::read_to_string(staged.join(REGISTRY_SOURCE))?;
    let registry_entry = format!("{factory}(runtime),");
    ensure!(
        generated.lines().any(|line| line.trim() == registry_entry),
        "The CLI registry generator did not select the provider for '{slug}'"
    );
    targets
        .into_iter()
        .zip(originals)
        .map(|(relative, original)| {
            let content = fs::read_to_string(staged.join(&relative))?;
            Ok(PlannedFile {
                path: root.join(relative),
                content,
                original: Some(original.into_bytes()),
            })
        })
        .collect()
}
