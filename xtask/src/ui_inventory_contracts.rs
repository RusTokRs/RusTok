use super::*;

pub(crate) fn validate_host_ui_inventory_contract(
    manifest_path: &Path,
    manifest: &Manifest,
) -> Result<()> {
    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let admin_manifest_path = workspace_root.join("apps").join("admin").join("Cargo.toml");
    let storefront_manifest_path = workspace_root
        .join("apps")
        .join("storefront")
        .join("Cargo.toml");

    validate_host_ui_surface_inventory(manifest_path, manifest, &admin_manifest_path, "admin")?;
    validate_host_ui_surface_inventory(
        manifest_path,
        manifest,
        &storefront_manifest_path,
        "storefront",
    )?;

    Ok(())
}

fn validate_host_ui_surface_inventory(
    manifest_path: &Path,
    manifest: &Manifest,
    host_manifest_path: &Path,
    surface: &str,
) -> Result<()> {
    let declared_ui_crates =
        collect_declared_ui_crates_for_surface(manifest_path, manifest, surface)?;
    let host_manifest = load_toml_value(host_manifest_path)?;
    let Some(host_dependencies) = host_manifest
        .get("dependencies")
        .and_then(TomlValue::as_table)
    else {
        return Ok(());
    };

    for (crate_name, dependency) in host_dependencies {
        let Some(dependency_table) = dependency.as_table() else {
            continue;
        };
        let Some(relative_path) = dependency_table
            .get("path")
            .and_then(TomlValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };

        let base_dir = host_manifest_path.parent().with_context(|| {
            format!(
                "Failed to resolve parent directory for {}",
                host_manifest_path.display()
            )
        })?;
        let ui_crate_dir = base_dir.join(relative_path);
        let actual_ui_manifest_path = ui_crate_dir.join("Cargo.toml");
        if !actual_ui_manifest_path.exists() {
            continue;
        }

        let surface_dir = actual_ui_manifest_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str());
        if surface_dir != Some(surface) {
            continue;
        }

        let Some(module_root) = actual_ui_manifest_path.parent().and_then(Path::parent) else {
            continue;
        };
        if !module_root.join("rustok-module.toml").exists() {
            continue;
        }

        let Some((owner_slug, expected_ui_manifest_path)) = declared_ui_crates.get(crate_name)
        else {
            anyhow::bail!(
                "Host UI inventory drift: {} depends on '{}' at '{}', but no module manifest declares it as {} UI",
                host_manifest_path.display(),
                crate_name,
                actual_ui_manifest_path.display(),
                surface
            );
        };

        let actual_ui_manifest_path =
            fs::canonicalize(&actual_ui_manifest_path).unwrap_or(actual_ui_manifest_path);
        let expected_ui_manifest_path = fs::canonicalize(expected_ui_manifest_path)
            .unwrap_or_else(|_| expected_ui_manifest_path.clone());
        if actual_ui_manifest_path != expected_ui_manifest_path {
            anyhow::bail!(
                "Host UI inventory drift: {} depends on '{}' for module '{}', but path '{}' does not match canonical '{}'",
                host_manifest_path.display(),
                crate_name,
                owner_slug,
                actual_ui_manifest_path.display(),
                expected_ui_manifest_path.display()
            );
        }
    }

    let host_features = host_manifest
        .get("features")
        .and_then(TomlValue::as_table)
        .cloned()
        .unwrap_or_default();
    validate_host_ui_feature_inventory(
        host_manifest_path,
        host_dependencies,
        &host_features,
        &declared_ui_crates,
        surface,
    )?;

    Ok(())
}

fn collect_declared_ui_crates_for_surface(
    manifest_path: &Path,
    manifest: &Manifest,
    surface: &str,
) -> Result<HashMap<String, (String, PathBuf)>> {
    let mut declared = HashMap::new();

    for (slug, spec) in &manifest.modules {
        let Some(package_manifest_path) = module_package_manifest_path(manifest_path, spec) else {
            continue;
        };
        if !package_manifest_path.exists() {
            continue;
        }

        let package_manifest = load_module_package_manifest(&package_manifest_path)?;
        let ui_crate = match surface {
            "admin" => package_manifest
                .provides
                .admin_ui
                .as_ref()
                .and_then(|ui| ui.leptos_crate.as_deref()),
            "storefront" => package_manifest
                .provides
                .storefront_ui
                .as_ref()
                .and_then(|ui| ui.leptos_crate.as_deref()),
            _ => None,
        };
        let Some(ui_crate) = ui_crate.map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };

        let expected_manifest_path =
            expected_module_ui_manifest_path(manifest_path, spec, surface)?;
        if let Some((existing_slug, _)) = declared.insert(
            ui_crate.to_string(),
            (slug.to_string(), expected_manifest_path),
        ) {
            anyhow::bail!(
                "UI crate '{}' is declared by multiple module manifests: '{}' and '{}'",
                ui_crate,
                existing_slug,
                slug
            );
        }
    }

    Ok(declared)
}
