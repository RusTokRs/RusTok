use super::*;

pub(crate) fn manifest_path() -> PathBuf {
    PathBuf::from("modules.toml")
}

pub(crate) fn load_manifest() -> Result<Manifest> {
    load_manifest_from(&manifest_path())
}

pub(crate) fn load_manifest_from(path: &Path) -> Result<Manifest> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let manifest: Manifest =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;

    if manifest.schema != 2 {
        anyhow::bail!("Unsupported manifest schema: {}", manifest.schema);
    }

    Ok(manifest)
}

pub(crate) fn module_package_manifest_path(
    manifest_path: &Path,
    spec: &ModuleSpec,
) -> Option<PathBuf> {
    if spec.source != "path" {
        return None;
    }

    let module_path = spec.path.as_ref()?;
    Some(
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(module_path)
            .join("rustok-module.toml"),
    )
}

pub(crate) fn load_module_package_manifest(path: &Path) -> Result<ModulePackageManifest> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .expect("xtask should live under the workspace root")
}

fn workspace_manifest_path() -> PathBuf {
    workspace_root().join("Cargo.toml")
}

pub(crate) fn load_workspace_manifest() -> Result<TomlValue> {
    let path = workspace_manifest_path();
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

pub(crate) fn load_toml_value(path: &Path) -> Result<TomlValue> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

pub(crate) fn resolve_workspace_inherited_string(
    value: Option<&TomlValue>,
    workspace_manifest: &TomlValue,
    field: &str,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    if let Some(string) = value.as_str() {
        return Ok(Some(string.trim().to_string()));
    }

    if let Some(table) = value.as_table() {
        if table.get("workspace").and_then(TomlValue::as_bool) == Some(true) {
            let inherited = workspace_manifest
                .get("workspace")
                .and_then(TomlValue::as_table)
                .and_then(|workspace| workspace.get("package"))
                .and_then(TomlValue::as_table)
                .and_then(|package| package.get(field))
                .and_then(TomlValue::as_str)
                .map(|value| value.trim().to_string());
            return Ok(inherited);
        }
    }

    anyhow::bail!("Unsupported package.{field} declaration in Cargo manifest")
}

pub(crate) fn load_resolved_cargo_package(
    path: &Path,
    workspace_manifest: &TomlValue,
) -> Result<ResolvedCargoPackage> {
    let manifest = load_toml_value(path)?;
    let package = manifest
        .get("package")
        .and_then(TomlValue::as_table)
        .with_context(|| format!("{} is missing [package]", path.display()))?;

    let name = package
        .get("name")
        .and_then(TomlValue::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{} is missing package.name", path.display()))?;
    let version =
        resolve_workspace_inherited_string(package.get("version"), workspace_manifest, "version")?
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{} is missing package.version", path.display()))?;
    let license =
        resolve_workspace_inherited_string(package.get("license"), workspace_manifest, "license")?
            .filter(|value| !value.is_empty());

    Ok(ResolvedCargoPackage {
        name,
        version,
        license,
        manifest_path: path.to_path_buf(),
    })
}
