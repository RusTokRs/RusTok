use super::*;
use crate::modules::{CatalogManifestModule, CatalogModuleVersion};
use crate::services::registry_principal::RegistryPrincipalRef;
use rustok_modules::{
    ModuleCommandContext, ModuleGovernanceActorContext,
    ModuleGovernancePublishArtifactDownloadSnapshot, ModuleGovernancePublishRequestStatusSnapshot,
    ModuleOwnerTransferCommand, ModuleReleaseYankCommand, ModuleReleaseYankResult,
};
use std::collections::HashMap;

impl RegistryGovernanceService {
    /// Maps canonical owner marketplace facts onto the host's public registry
    /// transport shape. This adapter performs no registry reads or policy
    /// derivation; the module owner owns the durable release projection.
    pub async fn apply_catalog_projection(
        &self,
        modules: Vec<CatalogManifestModule>,
        preferred_locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let entries = modules
            .iter()
            .map(catalog_entry_for_owner_projection)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let projected = self
            .release_service()
            .apply_marketplace_projection(entries, preferred_locale, fallback_locale)
            .await
            .map_err(anyhow::Error::new)?;
        let projected_by_slug = projected
            .into_iter()
            .map(|entry| (entry.slug.clone(), entry))
            .collect::<HashMap<_, _>>();

        let mut modules = modules;
        for module in &mut modules {
            let Some(entry) = projected_by_slug.get(&module.slug) else {
                continue;
            };
            if !entry.latest_version.is_empty() {
                module.version = Some(entry.latest_version.clone());
                module.name = Some(entry.name.clone());
                module.description = Some(entry.description.clone());
                module.publisher = entry.publisher.clone();
                module.checksum_sha256 = entry.checksum_sha256.clone();
            }
            module.versions = entry
                .versions
                .iter()
                .map(|version| CatalogModuleVersion {
                    version: version.version.clone(),
                    changelog: version.changelog.clone(),
                    yanked: version.yanked,
                    published_at: version.published_at.clone(),
                    checksum_sha256: version.checksum_sha256.clone(),
                    signature: None,
                    artifact: version.artifact.clone(),
                })
                .collect();
        }
        Ok(modules)
    }

    pub async fn yank_release(
        &self,
        slug: &str,
        version: &str,
        reason: &str,
        reason_code: &str,
        authority: &RegistryAuthority,
        context: ModuleCommandContext,
    ) -> anyhow::Result<ModuleReleaseYankResult> {
        let normalized_reason = normalize_required_reason(reason, "Registry yank")?;
        let normalized_reason_code =
            normalize_reason_code(reason_code, REGISTRY_YANK_REASON_CODES, "Registry yank")?;

        self.release_service()
            .yank_release(ModuleReleaseYankCommand {
                slug: slug.to_string(),
                version: version.to_string(),
                reason: normalized_reason,
                reason_code: normalized_reason_code,
                context,
                actor_principal: authority.principal.to_json_value(),
                actor_can_manage_modules: authority.can_manage_modules,
            })
            .await
            .map_err(anyhow::Error::new)
    }

    pub async fn transfer_registry_slug_owner(
        &self,
        slug: &str,
        new_owner: &RegistryPrincipalRef,
        reason: &str,
        reason_code: &str,
        authority: &RegistryAuthority,
        context: ModuleCommandContext,
    ) -> anyhow::Result<()> {
        if !new_owner.is_user() {
            return Err(malformed_error(format!(
                "Registry owner transfer for slug '{}' requires a valid new owner user principal",
                slug
            )));
        }
        let normalized_reason = normalize_required_reason(reason, "Registry owner transfer")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
            "Registry owner transfer",
        )?;

        self.release_service()
            .transfer_owner(ModuleOwnerTransferCommand {
                slug: slug.to_string(),
                new_owner_principal: new_owner.to_json_value(),
                context,
                actor_principal: authority.principal.to_json_value(),
                actor_can_manage_modules: authority.can_manage_modules,
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)
    }

    /// Loads one exact owner-derived publish status projection without querying
    /// registry persistence, deriving lifecycle policy, or copying it into a
    /// server-local snapshot.
    pub async fn publish_request_status_snapshot_for_authority(
        &self,
        request_id: &str,
        authority: Option<&RegistryAuthority>,
    ) -> anyhow::Result<Option<ModuleGovernancePublishRequestStatusSnapshot>> {
        let actor = authority.map(|authority| ModuleGovernanceActorContext {
            principal: authority.principal.to_json_value(),
            can_manage_modules: authority.can_manage_modules,
        });
        self.release_service()
            .publish_request_status_snapshot(request_id, actor.as_ref())
            .await
            .map_err(anyhow::Error::new)
    }

    /// Resolves only the owner-managed artifact delivery facts required by the
    /// storage host. It intentionally does not expose a SeaORM request model.
    pub async fn publish_artifact_download_snapshot(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<ModuleGovernancePublishArtifactDownloadSnapshot>> {
        self.release_service()
            .publish_artifact_download_snapshot(request_id)
            .await
            .map_err(anyhow::Error::new)
    }

    pub(crate) async fn authorized_publish_request_status_snapshot(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        required_permission: RegistryPublishRequestPermission,
        action: &str,
    ) -> anyhow::Result<ModuleGovernancePublishRequestStatusSnapshot> {
        let snapshot = self
            .publish_request_status_snapshot_for_authority(request_id, Some(authority))
            .await?
            .ok_or_else(|| {
                not_found_error(format!(
                    "Registry publish request '{request_id}' was not found"
                ))
            })?;
        let permitted = match required_permission {
            RegistryPublishRequestPermission::Manage => snapshot.authorization.can_manage,
            RegistryPublishRequestPermission::Review => snapshot.authorization.can_review,
        };
        if permitted {
            return Ok(snapshot);
        }

        let requirement = match required_permission {
            RegistryPublishRequestPermission::Manage => {
                "management actions require either MODULES_MANAGE, the current persisted owner binding, or (before owner binding exists) the original requester identity"
            }
            RegistryPublishRequestPermission::Review => {
                "review actions require either MODULES_MANAGE or the current persisted owner binding"
            }
        };
        Err(forbidden_error(format!(
            "Principal '{}' is not allowed to {} registry publish request '{}' for slug '{}'; {requirement}",
            authority_actor(authority),
            action,
            snapshot.request.id,
            snapshot.request.slug
        )))
    }
}

fn catalog_entry_for_owner_projection(
    module: &CatalogManifestModule,
) -> anyhow::Result<rustok_modules::ModuleMarketplaceEntry> {
    let settings_schema = serde_json::from_value(serde_json::to_value(&module.settings_schema)?)?;
    Ok(rustok_modules::ModuleMarketplaceEntry {
        slug: module.slug.clone(),
        name: module.name.clone().unwrap_or_default(),
        latest_version: module.version.clone().unwrap_or_default(),
        description: module.description.clone().unwrap_or_default(),
        source: module.source.clone(),
        kind: if module.required { "core" } else { "optional" }.to_string(),
        category: module.category.clone().unwrap_or_default(),
        tags: module.tags.clone(),
        icon_url: module.icon_url.clone(),
        banner_url: module.banner_url.clone(),
        screenshots: module.screenshots.clone(),
        crate_name: module.crate_name.clone(),
        dependencies: module.depends_on.clone(),
        ownership: module.ownership.clone(),
        trust_level: module.trust_level.clone(),
        rustok_min_version: module.rustok_min_version.clone(),
        rustok_max_version: module.rustok_max_version.clone(),
        publisher: module.publisher.clone(),
        checksum_sha256: module.checksum_sha256.clone(),
        signature_present: module.signature.is_some(),
        versions: module
            .versions
            .iter()
            .map(|version| rustok_modules::ModuleMarketplaceVersion {
                version: version.version.clone(),
                changelog: version.changelog.clone(),
                yanked: version.yanked,
                published_at: version.published_at.clone(),
                checksum_sha256: version.checksum_sha256.clone(),
                signature_present: version.signature.is_some(),
                artifact: version.artifact.clone(),
            })
            .collect(),
        has_admin_ui: module.has_admin_ui,
        has_storefront_ui: module.has_storefront_ui,
        ui_classification: module.ui_classification.clone(),
        registry_lifecycle: None,
        compatible: true,
        recommended_admin_surfaces: module.recommended_admin_surfaces.clone(),
        showcase_admin_surfaces: module.showcase_admin_surfaces.clone(),
        settings_schema,
        installed: false,
        installed_version: None,
        update_available: false,
    })
}
