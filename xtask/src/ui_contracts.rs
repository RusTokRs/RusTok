use super::*;

pub(crate) fn validate_module_admin_surface_contract(
    slug: &str,
    manifest: &ModulePackageManifest,
) -> Result<()> {
    let has_admin_ui = manifest.provides.admin_ui.is_some();
    let recommended = &manifest.module.recommended_admin_surfaces;
    let showcase = &manifest.module.showcase_admin_surfaces;

    if !has_admin_ui && (!recommended.is_empty() || !showcase.is_empty()) {
        anyhow::bail!(
            "Module '{slug}' declares recommended/showcase admin surfaces but does not declare [provides.admin_ui]"
        );
    }

    let Some(admin_ui) = manifest.provides.admin_ui.as_ref() else {
        return Ok(());
    };

    if recommended.is_empty() {
        anyhow::bail!(
            "Module '{slug}' declares [provides.admin_ui] and must declare at least one recommended_admin_surface"
        );
    }

    if admin_ui
        .leptos_crate
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        && !recommended
            .iter()
            .any(|surface| surface.trim() == "leptos-admin")
    {
        anyhow::bail!(
            "Module '{slug}' declares admin_ui.leptos_crate and must include 'leptos-admin' in recommended_admin_surfaces"
        );
    }

    Ok(())
}

pub(crate) fn validate_module_docs_navigation_contract(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
    admin_ui_crate: Option<&str>,
    storefront_ui_crate: Option<&str>,
    showcase_admin_surfaces: &[String],
) -> Result<()> {
    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let docs_modules_dir = workspace_root.join("docs").join("modules");
    let module_index_path = docs_modules_dir.join("_index.md");
    let ui_index_path = docs_modules_dir.join("UI_PACKAGES_INDEX.md");
    let module_index = fs::read_to_string(&module_index_path)
        .with_context(|| format!("Failed to read {}", module_index_path.display()))?;
    let ui_index = fs::read_to_string(&ui_index_path)
        .with_context(|| format!("Failed to read {}", ui_index_path.display()))?;
    let module_path = spec
        .path
        .as_deref()
        .with_context(|| format!("Module '{slug}' has source='path' but no path specified"))?;

    let docs_link = format!("../../{module_path}/docs/README.md");
    let plan_link = format!("../../{module_path}/docs/implementation-plan.md");
    if !module_index.contains(&docs_link) {
        anyhow::bail!(
            "Module '{slug}' is missing docs navigation link '{}' in {}",
            docs_link,
            module_index_path.display()
        );
    }
    if !module_index.contains(&plan_link) {
        anyhow::bail!(
            "Module '{slug}' is missing implementation plan link '{}' in {}",
            plan_link,
            module_index_path.display()
        );
    }

    if admin_ui_crate.is_some() {
        let admin_link = format!("../../{module_path}/admin/README.md");
        if !ui_index.contains(&admin_link) {
            anyhow::bail!(
                "Module '{slug}' declares admin UI, but {} is missing '{}'",
                ui_index_path.display(),
                admin_link
            );
        }
    }

    if storefront_ui_crate.is_some() {
        let storefront_link = format!("../../{module_path}/storefront/README.md");
        if !ui_index.contains(&storefront_link) {
            anyhow::bail!(
                "Module '{slug}' declares storefront UI, but {} is missing '{}'",
                ui_index_path.display(),
                storefront_link
            );
        }
    }

    if showcase_admin_surfaces
        .iter()
        .any(|surface| surface.trim() == "next-admin")
    {
        let next_admin_package_names = [slug.to_string(), format!("rustok-{slug}")];
        let next_admin_path_fragments = next_admin_package_names
            .iter()
            .map(|package| format!("apps/next-admin/packages/{package}/"))
            .collect::<Vec<_>>();
        if !next_admin_path_fragments
            .iter()
            .any(|fragment| ui_index.contains(fragment))
        {
            anyhow::bail!(
                "Module '{slug}' declares showcase_admin_surfaces=['next-admin'], but {} is missing one of {:?}",
                ui_index_path.display(),
                next_admin_path_fragments
            );
        }

        let next_admin_package_dirs = next_admin_package_names
            .iter()
            .map(|package| {
                workspace_root
                    .join("apps")
                    .join("next-admin")
                    .join("packages")
                    .join(package)
            })
            .collect::<Vec<_>>();
        if !next_admin_package_dirs.iter().any(|path| path.exists()) {
            anyhow::bail!(
                "Module '{slug}' declares showcase_admin_surfaces=['next-admin'], but none of {:?} exists",
                next_admin_package_dirs
            );
        }
    }

    Ok(())
}
