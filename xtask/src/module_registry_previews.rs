use super::*;

pub(crate) fn load_module_publish_preview_for_slug(
    slug: &str,
) -> Result<ModulePublishDryRunPreview> {
    let manifest_path = manifest_path();
    let manifest = load_manifest_from(&manifest_path)?;
    let workspace_manifest = load_workspace_manifest()?;
    let spec = manifest
        .modules
        .get(slug)
        .with_context(|| format!("Unknown module slug '{slug}'"))?;
    build_module_publish_preview(&manifest_path, slug, &spec, &workspace_manifest)
}

pub(crate) fn build_module_yank_preview_for_slug(
    slug: &str,
    version: &str,
) -> Result<ModuleYankDryRunPreview> {
    let preview = load_module_publish_preview_for_slug(slug)?;

    Ok(ModuleYankDryRunPreview {
        action: "yank".to_string(),
        slug: slug.to_string(),
        version: version.to_string(),
        crate_name: preview.crate_name.clone(),
        current_local_version: preview.version.clone(),
        matches_local_version: preview.version == version,
        package_manifest_path: preview.package_manifest_path,
    })
}

pub(crate) fn build_module_owner_transfer_preview_for_slug(
    slug: &str,
    new_owner_user_id: &str,
    reason: Option<String>,
    reason_code: Option<String>,
) -> Result<ModuleOwnerTransferDryRunPreview> {
    let preview = load_module_publish_preview_for_slug(slug)?;

    Ok(ModuleOwnerTransferDryRunPreview {
        action: "owner_transfer".to_string(),
        slug: slug.to_string(),
        crate_name: preview.crate_name,
        current_local_version: preview.version,
        package_manifest_path: preview.package_manifest_path,
        new_owner_user_id: new_owner_user_id.to_string(),
        reason,
        reason_code,
    })
}
