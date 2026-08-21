use super::*;
use crate::modules::{CatalogManifestModule, CatalogModuleVersion};
use rustok_modules::{
    ModuleGovernanceActorContext, ModuleGovernanceOwnerSnapshot,
    ModuleGovernancePublishArtifactDownloadSnapshot, ModuleGovernancePublishRequestStatusSnapshot,
    ModuleGovernanceRequestSnapshot, ModuleOwnerTransferCommand, ModuleReleaseYankCommand,
    ModuleReleaseYankResult,
};
use std::collections::HashMap;

fn map_owner_binding_snapshot(owner: ModuleGovernanceOwnerSnapshot) -> RegistryModuleOwnerSnapshot {
    RegistryModuleOwnerSnapshot {
        owner: RegistryPrincipalRef::from_json_value(&owner.owner_principal),
        bound_by: RegistryPrincipalRef::from_json_value(&owner.bound_by_principal),
        bound_at: owner.bound_at,
        updated_at: owner.updated_at,
    }
}

fn map_owner_request_snapshot(
    request: ModuleGovernanceRequestSnapshot,
) -> RegistryPublishRequestSnapshot {
    RegistryPublishRequestSnapshot {
        id: request.id,
        revision: request.revision,
        slug: request.slug,
        version: request.version,
        status: request.status,
        artifact_origin: request.artifact_origin,
        requested_by: RegistryPrincipalRef::from_json_value(&request.requested_by_principal),
        publisher: request
            .publisher_principal
            .as_ref()
            .map(RegistryPrincipalRef::from_json_value),
        approved_by: request
            .approved_by_principal
            .as_ref()
            .map(RegistryPrincipalRef::from_json_value),
        rejected_by: request
            .rejected_by_principal
            .as_ref()
            .map(RegistryPrincipalRef::from_json_value),
        rejection_reason: request.rejection_reason,
        changes_requested_by: request
            .changes_requested_by_principal
            .as_ref()
            .map(RegistryPrincipalRef::from_json_value),
        changes_requested_reason: request.changes_requested_reason,
        changes_requested_reason_code: request.changes_requested_reason_code,
        changes_requested_at: request.changes_requested_at,
        held_by: request
            .held_by_principal
            .as_ref()
            .map(RegistryPrincipalRef::from_json_value),
        held_reason: request.held_reason,
        held_reason_code: request.held_reason_code,
        held_at: request.held_at,
        held_from_status: request.held_from_status,
        warnings: request.warnings,
        errors: request.errors,
        created_at: request.created_at,
        updated_at: request.updated_at,
        published_at: request.published_at,
    }
}

pub(crate) fn map_owner_lifecycle_snapshot(
    snapshot: rustok_modules::ModuleGovernanceLifecycleSnapshot,
) -> RegistryModuleLifecycleSnapshot {
    RegistryModuleLifecycleSnapshot {
        owner_binding: snapshot.owner_binding.map(map_owner_binding_snapshot),
        latest_request: snapshot.latest_request.map(map_owner_request_snapshot),
        latest_release: snapshot
            .latest_release
            .map(|release| RegistryModuleReleaseSnapshot {
                version: release.version,
                status: release.status,
                publisher: RegistryPrincipalRef::from_json_value(&release.publisher_principal),
                checksum_sha256: release.checksum_sha256,
                published_at: release.published_at,
                yanked_reason: release.yanked_reason,
                yanked_by: release
                    .yanked_by_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value),
                yanked_at: release.yanked_at,
            }),
        recent_events: snapshot
            .recent_events
            .into_iter()
            .map(|event| RegistryGovernanceEventSnapshot {
                id: event.id,
                event_type: event.event_type,
                actor: RegistryPrincipalRef::from_json_value(&event.actor_principal),
                publisher: event
                    .publisher_principal
                    .as_ref()
                    .map(RegistryPrincipalRef::from_json_value),
                payload: RegistryGovernanceEventPayload {
                    reason: event.payload.reason,
                    reason_code: event.payload.reason_code,
                    detail: event.payload.detail,
                    version: event.payload.version,
                    stage_key: event.payload.stage_key,
                    attempt_number: event.payload.attempt_number,
                    owner_transition: event.payload.owner_transition.map(|transition| {
                        RegistryOwnerTransitionPayload {
                            previous_owner: transition
                                .previous_owner_principal
                                .as_ref()
                                .map(RegistryPrincipalRef::from_json_value),
                            new_owner: transition
                                .new_owner_principal
                                .as_ref()
                                .map(RegistryPrincipalRef::from_json_value),
                            bound_by: transition
                                .bound_by_principal
                                .as_ref()
                                .map(RegistryPrincipalRef::from_json_value),
                        }
                    }),
                    warnings: event.payload.warnings,
                    errors: event.payload.errors,
                    mode: event.payload.mode,
                },
                created_at: event.created_at,
            })
            .collect(),
        follow_up_gates: snapshot
            .follow_up_gates
            .into_iter()
            .map(|gate| RegistryFollowUpGateSnapshot {
                key: gate.key,
                status: gate.status,
                detail: gate.detail,
                updated_at: gate.updated_at,
            })
            .collect(),
        validation_stages: snapshot
            .validation_stages
            .into_iter()
            .map(|stage| RegistryValidationStageSnapshot {
                key: stage.key,
                status: stage.status,
                detail: stage.detail,
                attempt_number: stage.attempt_number,
                updated_at: stage.updated_at,
                started_at: stage.started_at,
                finished_at: stage.finished_at,
            })
            .collect(),
        governance_actions: snapshot
            .governance_actions
            .into_iter()
            .map(|action| RegistryGovernanceActionSnapshot {
                key: action.key,
                reason_required: action.reason_required,
                reason_code_required: action.reason_code_required,
                reason_codes: action.reason_codes,
                destructive: action.destructive,
            })
            .collect(),
    }
}

fn map_owner_request_status_snapshot(
    snapshot: ModuleGovernancePublishRequestStatusSnapshot,
) -> RegistryPublishRequestStatusSnapshot {
    RegistryPublishRequestStatusSnapshot {
        request: map_owner_request_snapshot(snapshot.request),
        authorization: RegistryPublishRequestAuthorizationSnapshot {
            can_manage: snapshot.authorization.can_manage,
            can_review: snapshot.authorization.can_review,
        },
        effective_publisher_principal: snapshot.effective_publisher_principal,
        rejected_retry_allowed: snapshot.rejected_retry_allowed,
        follow_up_gates: snapshot
            .follow_up_gates
            .into_iter()
            .map(|gate| RegistryFollowUpGateSnapshot {
                key: gate.key,
                status: gate.status,
                detail: gate.detail,
                updated_at: gate.updated_at,
            })
            .collect(),
        validation_stages: snapshot
            .validation_stages
            .into_iter()
            .map(|stage| RegistryValidationStageSnapshot {
                key: stage.key,
                status: stage.status,
                detail: stage.detail,
                attempt_number: stage.attempt_number,
                updated_at: stage.updated_at,
                started_at: stage.started_at,
                finished_at: stage.finished_at,
            })
            .collect(),
        approval_override_required: snapshot.approval_override_required,
        approval_override_reason_codes: snapshot.approval_override_reason_codes,
        approval_override_warning: snapshot.approval_override_warning,
        governance_actions: snapshot
            .governance_actions
            .into_iter()
            .map(|action| RegistryGovernanceActionSnapshot {
                key: action.key,
                reason_required: action.reason_required,
                reason_code_required: action.reason_code_required,
                reason_codes: action.reason_codes,
                destructive: action.destructive,
            })
            .collect(),
        accepted: snapshot.accepted,
        next_action: snapshot.next_action,
    }
}

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
                actor_principal: authority.principal.to_json_value(),
                actor_can_manage_modules: authority.can_manage_modules,
                reason: normalized_reason,
                reason_code: normalized_reason_code,
            })
            .await
            .map_err(anyhow::Error::new)
    }

    pub async fn lifecycle_snapshot(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<RegistryModuleLifecycleSnapshot>> {
        let snapshot = self
            .release_service()
            .lifecycle_snapshot(slug)
            .await
            .map_err(anyhow::Error::new)?;
        Ok(snapshot.map(map_owner_lifecycle_snapshot))
    }

    /// Loads one exact owner-derived publish status projection. The adapter
    /// maps owner facts into the server transport shape without querying
    /// registry persistence or deriving lifecycle policy.
    pub async fn publish_request_status_snapshot_for_authority(
        &self,
        request_id: &str,
        authority: Option<&RegistryAuthority>,
    ) -> anyhow::Result<Option<RegistryPublishRequestStatusSnapshot>> {
        let actor = authority.map(|authority| ModuleGovernanceActorContext {
            principal: authority.principal.to_json_value(),
            can_manage_modules: authority.can_manage_modules,
        });
        self.release_service()
            .publish_request_status_snapshot(request_id, actor.as_ref())
            .await
            .map_err(anyhow::Error::new)
            .map(|snapshot| snapshot.map(map_owner_request_status_snapshot))
    }

    /// Resolves only the owner-managed artifact delivery facts required by the
    /// storage host. It intentionally does not expose a SeaORM request model.
    pub async fn publish_artifact_download_snapshot(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<RegistryPublishArtifactDownloadSnapshot>> {
        self.release_service()
            .publish_artifact_download_snapshot(request_id)
            .await
            .map_err(anyhow::Error::new)
            .map(|snapshot| {
                snapshot.map(
                    |ModuleGovernancePublishArtifactDownloadSnapshot {
                         storage_key,
                         content_type,
                     }| RegistryPublishArtifactDownloadSnapshot {
                        storage_key,
                        content_type,
                    },
                )
            })
    }

    pub(crate) async fn authorized_publish_request_status_snapshot(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        required_permission: RegistryPublishRequestPermission,
        action: &str,
    ) -> anyhow::Result<RegistryPublishRequestStatusSnapshot> {
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
