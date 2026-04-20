use super::*;

pub(crate) fn validate_module_semantics_contract(
    slug: &str,
    spec: &ModuleSpec,
    metadata: &ModulePackageMetadata,
) -> Result<()> {
    let ownership = metadata.ownership.trim();
    let trust_level = metadata.trust_level.trim();

    if spec.source == "path" && ownership != "first_party" {
        anyhow::bail!(
            "Module '{slug}' uses source='path' and must declare ownership='first_party', got '{}'",
            ownership
        );
    }

    if spec.required && trust_level != "core" {
        anyhow::bail!(
            "Module '{slug}' is required in modules.toml and must declare trust_level='core', got '{}'",
            trust_level
        );
    }

    if !spec.required && trust_level == "core" {
        anyhow::bail!(
            "Module '{slug}' is optional in modules.toml and must not declare trust_level='core'"
        );
    }

    Ok(())
}

pub(crate) fn validate_module_dependency_contract(
    slug: &str,
    spec: &ModuleSpec,
    manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    let manifest_dependencies = normalize_dependency_set(spec.depends_on.as_deref().unwrap_or(&[]));
    let package_dependencies = normalize_dependency_set(
        &manifest
            .dependencies
            .keys()
            .map(|dependency| dependency.to_string())
            .collect::<Vec<_>>(),
    );

    if manifest_dependencies != package_dependencies {
        anyhow::bail!(
            "Module '{slug}' dependency mismatch between modules.toml and rustok-module.toml: modules.toml={:?}, rustok-module.toml={:?}",
            manifest_dependencies,
            package_dependencies
        );
    }

    if let Some(runtime_dependencies) = extract_runtime_module_dependencies(module_root)? {
        if manifest_dependencies != runtime_dependencies {
            anyhow::bail!(
                "Module '{slug}' dependency mismatch between modules.toml and RusToKModule::dependencies(): modules.toml={:?}, runtime={:?}",
                manifest_dependencies,
                runtime_dependencies
            );
        }
    }

    Ok(())
}

pub(crate) fn validate_module_kind_contract(
    slug: &str,
    spec: &ModuleSpec,
    module_root: &Path,
) -> Result<()> {
    let lib_path = module_root.join("src").join("lib.rs");
    if !lib_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&lib_path)
        .with_context(|| format!("Failed to read {}", lib_path.display()))?;
    if !content.contains("impl RusToKModule for") {
        return Ok(());
    }

    let runtime_kind = extract_runtime_module_kind(&content);
    if spec.required {
        if runtime_kind != Some("Core") {
            anyhow::bail!(
                "Module '{slug}' is required in modules.toml and must declare fn kind(&self) -> ModuleKind {{ ModuleKind::Core }} in {}",
                lib_path.display()
            );
        }
    } else if runtime_kind == Some("Core") {
        anyhow::bail!(
            "Module '{slug}' is optional in modules.toml and must not declare ModuleKind::Core in {}",
            lib_path.display()
        );
    }

    Ok(())
}

pub(crate) fn normalize_dependency_set(dependencies: &[String]) -> HashSet<String> {
    dependencies
        .iter()
        .map(|dependency| dependency.trim())
        .filter(|dependency| !dependency.is_empty())
        .map(|dependency| dependency.to_string())
        .collect()
}

pub(crate) fn validate_module_ui_surface_contract(
    slug: &str,
    module_root: &Path,
    surface: &str,
    crate_name: Option<&str>,
) -> Result<()> {
    let manifest_path = module_root.join(surface).join("Cargo.toml");
    let has_subcrate = manifest_path.exists();
    let crate_name = crate_name.map(str::trim).filter(|value| !value.is_empty());

    if has_subcrate && crate_name.is_none() {
        anyhow::bail!(
            "Module '{slug}' contains {}, but rustok-module.toml is missing [provides.{surface}_ui].leptos_crate",
            manifest_path.display()
        );
    }

    if !has_subcrate && crate_name.is_some() {
        anyhow::bail!(
            "Module '{slug}' declares [provides.{surface}_ui].leptos_crate, but {} is missing",
            manifest_path.display()
        );
    }

    Ok(())
}

pub(crate) fn validate_module_ui_package(
    slug: &str,
    module_root: &Path,
    surface: &str,
    crate_name: Option<&str>,
    expected_version: &str,
    workspace_manifest: &TomlValue,
) -> Result<Option<ModuleUiPackagePreview>> {
    let Some(crate_name) = crate_name else {
        return Ok(None);
    };

    let manifest_path = module_root.join(surface).join("Cargo.toml");
    if !manifest_path.exists() {
        anyhow::bail!(
            "Module '{slug}' declares provides.{surface}_ui.leptos_crate='{crate_name}', but {} is missing",
            manifest_path.display()
        );
    }

    let package = load_resolved_cargo_package(&manifest_path, workspace_manifest)?;
    if package.name != crate_name {
        anyhow::bail!(
            "Module '{slug}' declares provides.{surface}_ui.leptos_crate='{crate_name}', but {} declares '{}'",
            manifest_path.display(),
            package.name
        );
    }
    if package.version != expected_version {
        anyhow::bail!(
            "Module '{slug}' {surface} package version mismatch: expected '{expected_version}', got '{}'",
            package.version
        );
    }

    Ok(Some(ModuleUiPackagePreview {
        crate_name: package.name,
        manifest_path: manifest_path.display().to_string(),
    }))
}
