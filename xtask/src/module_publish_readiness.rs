use super::*;

// Shared source is mounted at crate root from ../../crates/rustok-build/src/module_manifest_contribution.rs.
pub(crate) fn validate_module_publish_readiness(
    manifest_path: &Path,
    slug: &str,
    spec: &ModuleSpec,
    package_manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    validate_module_package_metadata(slug, &package_manifest.module)?;
    validate_module_publish_contract(slug, package_manifest)?;

    let declared_slug = package_manifest.module.slug.trim();
    if declared_slug != slug {
        anyhow::bail!(
            "Module '{slug}' declares slug '{}' in rustok-module.toml",
            declared_slug
        );
    }

    validate_module_local_docs_contract(slug, module_root)?;
    validate_module_entry_type_contract(slug, package_manifest, module_root)?;
    validate_module_runtime_metadata_contract(slug, package_manifest, module_root)?;
    validate_module_semantics_contract(slug, spec, &package_manifest.module)?;
    validate_module_kind_contract(slug, spec, module_root)?;
    validate_module_transport_surface_contract(slug, package_manifest, module_root)?;
    validate_module_server_http_surface_contract(manifest_path, slug, package_manifest)?;
    validate_module_ui_classification_contract(slug, package_manifest)?;
    validate_module_ui_metadata_contract(slug, package_manifest)?;
    validate_module_contribution_manifest_contract(slug, package_manifest, module_root)?;
    validate_module_admin_surface_contract(slug, package_manifest)?;
    validate_module_dependency_contract(slug, spec, package_manifest, module_root)?;
    validate_module_permission_contract(slug, module_root)?;
    validate_module_event_listener_contract(slug, module_root)?;
    validate_module_event_ingress_contract(manifest_path, slug, module_root)?;
    validate_module_index_search_boundary_contract(slug, module_root)?;
    validate_module_search_operator_surface_contract(slug, module_root)?;
    validate_module_registry_docs_contract(manifest_path, slug, spec)?;
    validate_module_server_registry_contract(
        manifest_path,
        slug,
        spec,
        package_manifest,
        module_root,
    )?;

    Ok(())
}

pub(crate) fn validate_module_publish_contract(
    slug: &str,
    manifest: &ModulePackageManifest,
) -> Result<()> {
    let module_slug = manifest.module.slug.trim();
    if module_slug.is_empty() {
        anyhow::bail!("Module '{slug}' is missing module.slug in rustok-module.toml");
    }

    if manifest.module.name.trim().is_empty() {
        anyhow::bail!("Module '{slug}' is missing module.name in rustok-module.toml");
    }

    let version = manifest.module.version.trim();
    if version.is_empty() {
        anyhow::bail!("Module '{slug}' is missing module.version in rustok-module.toml");
    }
    Version::parse(version)
        .with_context(|| format!("Module '{slug}' has non-semver module.version '{version}'"))?;

    if manifest.module.description.trim().len() < 20 {
        anyhow::bail!(
            "Module '{slug}' description must be at least 20 characters for publish readiness"
        );
    }

    Ok(())
}

fn validate_module_contribution_manifest_contract(
    slug: &str,
    manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    let path = module_root.join("rustok-module.toml");
    let source = fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read {} for contribution validation",
            path.display()
        )
    })?;
    let Some(normalized) =
        crate::module_manifest_contribution::normalize_module_contribution_manifest(&source)
            .map_err(|error| {
                anyhow::anyhow!("Module '{slug}' has invalid contribution metadata: {error}")
            })?
    else {
        return Ok(());
    };

    if normalized.module_id != manifest.module.slug.trim() {
        anyhow::bail!(
            "Module '{slug}' contribution metadata resolves module_id='{}', expected '{}'",
            normalized.module_id,
            manifest.module.slug.trim()
        );
    }
    if normalized.owner_version != manifest.module.version.trim() {
        anyhow::bail!(
            "Module '{slug}' contribution metadata resolves owner_version='{}', expected module.version='{}'",
            normalized.owner_version,
            manifest.module.version.trim()
        );
    }
    if !normalized.admin.is_empty() && manifest.provides.admin_ui.is_none() {
        anyhow::bail!(
            "Module '{slug}' declares admin contribution metadata without [provides.admin_ui]"
        );
    }
    if !normalized.storefront.is_empty() && manifest.provides.storefront_ui.is_none() {
        anyhow::bail!(
            "Module '{slug}' declares storefront contribution metadata without [provides.storefront_ui]"
        );
    }

    Ok(())
}
