use super::*;

pub(crate) fn build_module_publish_preview(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
    workspace_manifest: &TomlValue,
) -> Result<ModulePublishDryRunPreview> {
    if spec.source != "path" {
        anyhow::bail!(
            "Module '{slug}' uses source='{}'; publish dry-run currently supports only local path modules",
            spec.source
        );
    }

    let package_manifest_path = module_package_manifest_path(manifest_path, spec)
        .with_context(|| format!("Module '{slug}' has source='path' but no path specified"))?;
    if !package_manifest_path.exists() {
        anyhow::bail!(
            "Module '{slug}' requires rustok-module.toml at {}",
            package_manifest_path.display()
        );
    }

    let package_manifest = load_module_package_manifest(&package_manifest_path)?;

    let module_root = package_manifest_path
        .parent()
        .map(PathBuf::from)
        .with_context(|| {
            format!(
                "Failed to resolve module root for '{}'",
                package_manifest_path.display()
            )
        })?;
    validate_module_publish_readiness(manifest_path, slug, spec, &package_manifest, &module_root)?;
    let crate_manifest_path = module_root.join("Cargo.toml");
    let crate_package = load_resolved_cargo_package(&crate_manifest_path, workspace_manifest)?;

    if crate_package.name != spec.crate_name {
        anyhow::bail!(
            "Module '{slug}' points to crate '{}' in modules.toml, but Cargo.toml declares '{}'",
            spec.crate_name,
            crate_package.name
        );
    }

    if crate_package.version != package_manifest.module.version {
        anyhow::bail!(
            "Module '{slug}' version mismatch: rustok-module.toml has '{}', Cargo.toml resolves to '{}'",
            package_manifest.module.version,
            crate_package.version
        );
    }

    let license = crate_package.license.clone().with_context(|| {
        format!(
            "Module '{slug}' must resolve package.license via {}",
            crate_package.manifest_path.display()
        )
    })?;

    validate_module_ui_surface_contract(
        slug,
        &module_root,
        "admin",
        package_manifest
            .provides
            .admin_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
    )?;
    validate_module_ui_surface_contract(
        slug,
        &module_root,
        "storefront",
        package_manifest
            .provides
            .storefront_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
    )?;

    let admin_preview = validate_module_ui_package(
        slug,
        &module_root,
        "admin",
        package_manifest
            .provides
            .admin_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        &package_manifest.module.version,
        workspace_manifest,
    )?;
    let storefront_preview = validate_module_ui_package(
        slug,
        &module_root,
        "storefront",
        package_manifest
            .provides
            .storefront_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        &package_manifest.module.version,
        workspace_manifest,
    )?;
    validate_module_host_ui_contract(
        manifest_path,
        slug,
        spec,
        package_manifest
            .provides
            .admin_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        package_manifest
            .provides
            .storefront_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
    )?;
    validate_module_docs_navigation_contract(
        manifest_path,
        slug,
        spec,
        package_manifest
            .provides
            .admin_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        package_manifest
            .provides
            .storefront_ui
            .as_ref()
            .and_then(|ui| ui.leptos_crate.as_deref()),
        &package_manifest.module.showcase_admin_surfaces,
    )?;
    let module_default_locale = package_manifest
        .provides
        .admin_ui
        .as_ref()
        .and_then(|ui| ui.i18n.as_ref())
        .and_then(|i18n| i18n.default_locale.clone())
        .or_else(|| {
            package_manifest
                .provides
                .storefront_ui
                .as_ref()
                .and_then(|ui| ui.i18n.as_ref())
                .and_then(|i18n| i18n.default_locale.clone())
        })
        .unwrap_or_else(|| "en".to_string());

    Ok(ModulePublishDryRunPreview {
        slug: slug.to_string(),
        version: package_manifest.module.version.clone(),
        crate_name: crate_package.name,
        module_default_locale,
        module_name: package_manifest.module.name.clone(),
        module_description: package_manifest.module.description.clone(),
        ownership: package_manifest.module.ownership.clone(),
        trust_level: package_manifest.module.trust_level.clone(),
        license,
        manifest_path: manifest_path.display().to_string(),
        package_manifest_path: package_manifest_path.display().to_string(),
        module_entry_type: package_manifest.crate_contract.entry_type.clone(),
        marketplace: ModuleMarketplacePreview {
            category: package_manifest.marketplace.category.clone(),
            tags: package_manifest.marketplace.tags.clone(),
        },
        ui_packages: ModuleUiPackagesPreview {
            admin: admin_preview,
            storefront: storefront_preview,
        },
    })
}
