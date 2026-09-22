use super::*;

pub(crate) fn build_publish_artifact_bytes(
    preview: &ModulePublishDryRunPreview,
) -> Result<Vec<u8>> {
    let package_manifest_path = workspace_root().join(&preview.package_manifest_path);
    let package_manifest = fs::read_to_string(&package_manifest_path).with_context(|| {
        format!(
            "Failed to read publish package manifest {}",
            package_manifest_path.display()
        )
    })?;
    let module_root = package_manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve module root for {}",
            package_manifest_path.display()
        )
    })?;
    let crate_manifest_path = module_root.join("Cargo.toml");
    let crate_manifest = fs::read_to_string(&crate_manifest_path).with_context(|| {
        format!(
            "Failed to read crate manifest {}",
            crate_manifest_path.display()
        )
    })?;

    let admin_manifest = preview
        .ui_packages
        .admin
        .as_ref()
        .map(|ui| {
            let path = workspace_root().join(&ui.manifest_path);
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read admin UI manifest {}", path.display()))?;
            Ok::<String, anyhow::Error>(content)
        })
        .transpose()?;
    let storefront_manifest = preview
        .ui_packages
        .storefront
        .as_ref()
        .map(|ui| {
            let path = workspace_root().join(&ui.manifest_path);
            let content = fs::read_to_string(&path).with_context(|| {
                format!("Failed to read storefront UI manifest {}", path.display())
            })?;
            Ok::<String, anyhow::Error>(content)
        })
        .transpose()?;

    let payload = serde_json::json!({
        "schema_version": REGISTRY_MUTATION_SCHEMA_VERSION,
        "artifact_type": "rustok-module-publish-bundle",
        "module": preview,
        "files": {
            "rustok-module.toml": package_manifest,
            "Cargo.toml": crate_manifest,
            "admin/Cargo.toml": admin_manifest,
            "storefront/Cargo.toml": storefront_manifest,
        }
    });

    serde_json::to_vec_pretty(&payload).context("Failed to serialize publish artifact bundle")
}
