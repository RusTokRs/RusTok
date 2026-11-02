use super::*;

pub(crate) fn validate_module_semantics_contract(
    slug: &str,
    spec: &ModuleSpec,
    metadata: &ModulePackageMetadata,
) -> Result<()> {
    if spec.runtime.trim() != metadata.runtime.trim() {
        anyhow::bail!(
            "Module '{slug}' runtime mismatch between modules.toml ('{}') and rustok-module.toml ('{}')",
            spec.runtime,
            metadata.runtime
        );
    }

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
    let Some(source) = load_runtime_module_source(module_root)? else {
        return Ok(());
    };

    let runtime_kind = extract_runtime_module_kind(&source.content);
    if spec.required {
        if runtime_kind != Some("Core") {
            anyhow::bail!(
                "Module '{slug}' is required in modules.toml and must declare fn kind(&self) -> ModuleKind {{ ModuleKind::Core }} in {}",
                source.path.display()
            );
        }
    } else if runtime_kind == Some("Core") {
        anyhow::bail!(
            "Module '{slug}' is optional in modules.toml and must not declare ModuleKind::Core in {}",
            source.path.display()
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
        // A future_ui module may carry an unmounted draft package before it
        // publishes a live UI surface or a reusable support crate. Keep the
        // package identity canonical without inventing provides metadata.
        let module_manifest_path = module_root.join("rustok-module.toml");
        let module_source = std::fs::read_to_string(&module_manifest_path).map_err(|error| {
            anyhow::anyhow!(
                "Failed to read {} while validating draft UI package: {error}",
                module_manifest_path.display()
            )
        })?;
        let module_manifest = toml::from_str::<toml::Value>(&module_source).map_err(|error| {
            anyhow::anyhow!(
                "Failed to parse {} while validating draft UI package: {error}",
                module_manifest_path.display()
            )
        })?;
        let classification = module_manifest
            .get("module")
            .and_then(|module| module.get("ui_classification"))
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .trim();

        if classification == "future_ui" {
            let cargo_source = std::fs::read_to_string(&manifest_path).map_err(|error| {
                anyhow::anyhow!(
                    "Failed to read draft UI package {}: {error}",
                    manifest_path.display()
                )
            })?;
            let cargo_manifest = toml::from_str::<toml::Value>(&cargo_source).map_err(|error| {
                anyhow::anyhow!(
                    "Failed to parse draft UI package {}: {error}",
                    manifest_path.display()
                )
            })?;
            let package_name = cargo_manifest
                .get("package")
                .and_then(|package| package.get("name"))
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .trim();
            let expected_name = format!("rustok-{}-{surface}", slug.replace('_', "-"));

            if package_name != expected_name {
                anyhow::bail!(
                    "Module '{slug}' future_ui draft {surface} package must declare package.name='{expected_name}', got '{package_name}'"
                );
            }

            return Ok(());
        }

        anyhow::bail!(
            "Module '{slug}' contains {}, but rustok-module.toml is missing [provides.{surface}_ui].leptos_crate or [provides.{surface}_support].leptos_crate",
            manifest_path.display()
        );
    }

    if !has_subcrate && crate_name.is_some() {
        anyhow::bail!(
            "Module '{slug}' declares [provides.{surface}_ui].leptos_crate or [provides.{surface}_support].leptos_crate, but {} is missing",
            manifest_path.display()
        );
    }

    Ok(())
}

pub(crate) fn validate_module_ui_package(
    slug: &str,
    module_root: &Path,
    surface: &str,
    section: &str,
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
            "Module '{slug}' declares provides.{section}.leptos_crate='{crate_name}', but {} is missing",
            manifest_path.display()
        );
    }

    let package = load_resolved_cargo_package(&manifest_path, workspace_manifest)?;
    if package.name != crate_name {
        anyhow::bail!(
            "Module '{slug}' declares provides.{section}.leptos_crate='{crate_name}', but {} declares '{}'",
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


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_root() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rustok-module-ui-contract-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn future_ui_allows_an_unmounted_draft_package_with_canonical_identity() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("admin")).unwrap();
        std::fs::write(
            root.join("rustok-module.toml"),
            "[module]\nui_classification = \"future_ui\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("admin/Cargo.toml"),
            "[package]\nname = \"rustok-example-admin\"\n",
        )
        .unwrap();

        validate_module_ui_surface_contract("example", &root, "admin", None).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn future_ui_rejects_a_draft_package_with_noncanonical_identity() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("admin")).unwrap();
        std::fs::write(
            root.join("rustok-module.toml"),
            "[module]\nui_classification = \"future_ui\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("admin/Cargo.toml"),
            "[package]\nname = \"wrong-admin-package\"\n",
        )
        .unwrap();

        let error = validate_module_ui_surface_contract("example", &root, "admin", None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("rustok-example-admin"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn non_future_ui_cannot_hide_an_unregistered_ui_subcrate() {
        let root = temp_root();
        std::fs::create_dir_all(root.join("admin")).unwrap();
        std::fs::write(
            root.join("rustok-module.toml"),
            "[module]\nui_classification = \"no_ui\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("admin/Cargo.toml"),
            "[package]\nname = \"rustok-example-admin\"\n",
        )
        .unwrap();

        let error = validate_module_ui_surface_contract("example", &root, "admin", None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing [provides.admin_ui]"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
