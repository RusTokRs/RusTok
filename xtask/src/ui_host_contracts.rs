use super::*;

pub(crate) fn validate_module_host_ui_contract(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
    admin_ui_crate: Option<&str>,
    storefront_ui_crate: Option<&str>,
) -> Result<()> {
    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let root_manifest = load_manifest_from(manifest_path)?;

    if let Some(crate_name) = admin_ui_crate
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let admin_manifest_path = workspace_root.join("apps").join("admin").join("Cargo.toml");
        let expected_admin_manifest_path =
            expected_module_ui_manifest_path(manifest_path, spec, "admin")?;
        validate_host_ui_crate_wiring(
            slug,
            crate_name,
            &admin_manifest_path,
            Some(&expected_admin_manifest_path),
            &["hydrate", "ssr"],
        )?;
        for (dependency_slug, dependency_ui_crate) in
            collect_dependency_ui_crates_for_surface(&root_manifest, spec, manifest_path, "admin")?
        {
            let dependency_spec =
                root_manifest
                    .modules
                    .get(&dependency_slug)
                    .with_context(|| {
                        format!(
                            "Module dependency '{dependency_slug}' is missing from modules.toml"
                        )
                    })?;
            let expected_dependency_manifest_path =
                expected_module_ui_manifest_path(manifest_path, dependency_spec, "admin")?;
            validate_host_ui_crate_wiring(
                slug,
                &dependency_ui_crate,
                &admin_manifest_path,
                Some(&expected_dependency_manifest_path),
                &["hydrate", "ssr"],
            )
            .with_context(|| {
                format!(
                    "Module '{slug}' admin host composition is missing UI dependency from module '{dependency_slug}'"
                )
            })?;
        }
    }

    if let Some(crate_name) = storefront_ui_crate
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let storefront_manifest_path = workspace_root
            .join("apps")
            .join("storefront")
            .join("Cargo.toml");
        let expected_storefront_manifest_path =
            expected_module_ui_manifest_path(manifest_path, spec, "storefront")?;
        validate_host_ui_crate_wiring(
            slug,
            crate_name,
            &storefront_manifest_path,
            Some(&expected_storefront_manifest_path),
            &["ssr"],
        )?;
        for (dependency_slug, dependency_ui_crate) in collect_dependency_ui_crates_for_surface(
            &root_manifest,
            spec,
            manifest_path,
            "storefront",
        )? {
            let dependency_spec =
                root_manifest
                    .modules
                    .get(&dependency_slug)
                    .with_context(|| {
                        format!(
                            "Module dependency '{dependency_slug}' is missing from modules.toml"
                        )
                    })?;
            let expected_dependency_manifest_path =
                expected_module_ui_manifest_path(manifest_path, dependency_spec, "storefront")?;
            validate_host_ui_crate_wiring(
                slug,
                &dependency_ui_crate,
                &storefront_manifest_path,
                Some(&expected_dependency_manifest_path),
                &["ssr"],
            )
            .with_context(|| {
                format!(
                    "Module '{slug}' storefront host composition is missing UI dependency from module '{dependency_slug}'"
                )
            })?;
        }
    }

    Ok(())
}
