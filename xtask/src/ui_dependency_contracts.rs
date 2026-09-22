use super::*;

pub(crate) fn collect_dependency_ui_crates_for_surface(
    manifest: &Manifest,
    spec: &ModuleSpec,
    manifest_path: &Path,
    surface: &str,
) -> Result<Vec<(String, String)>> {
    let mut result = Vec::new();

    for dependency_slug in spec.depends_on.as_deref().unwrap_or(&[]) {
        let dependency_slug = dependency_slug.trim();
        if dependency_slug.is_empty() {
            continue;
        }

        let dependency_spec = manifest.modules.get(dependency_slug).with_context(|| {
            format!("Module dependency '{dependency_slug}' is missing from modules.toml")
        })?;
        let Some(dependency_manifest_path) =
            module_package_manifest_path(manifest_path, dependency_spec)
        else {
            continue;
        };
        let dependency_manifest = load_module_package_manifest(&dependency_manifest_path)?;
        let dependency_ui_crate = match surface {
            "admin" => dependency_manifest
                .provides
                .admin_ui
                .as_ref()
                .and_then(|ui| ui.leptos_crate.as_deref()),
            "storefront" => dependency_manifest
                .provides
                .storefront_ui
                .as_ref()
                .and_then(|ui| ui.leptos_crate.as_deref()),
            _ => None,
        };

        if let Some(crate_name) = dependency_ui_crate
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            result.push((dependency_slug.to_string(), crate_name.to_string()));
        }
    }

    Ok(result)
}

pub(crate) fn expected_module_ui_manifest_path(
    manifest_path: &Path,
    spec: &ModuleSpec,
    surface: &str,
) -> Result<PathBuf> {
    let module_path = spec.path.as_deref().with_context(|| {
        format!(
            "Module '{}' has source='path' but no path specified",
            spec.crate_name
        )
    })?;
    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    Ok(workspace_root
        .join(module_path)
        .join(surface)
        .join("Cargo.toml"))
}
