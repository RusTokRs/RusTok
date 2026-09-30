//! Governance service definition, marketplace projections, and lifecycle snapshots.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value,
};
use semver::Version;

use super::*;
use crate::marketplace::{ModuleMarketplaceEntry, ModuleMarketplaceVersion};
use crate::marketplace_content::ModuleMarketplaceContentProjection;
use crate::ControlPlaneInfrastructure;

#[derive(Clone)]
pub struct SeaOrmModuleGovernanceService {
    pub(crate) db: DatabaseConnection,
    pub(crate) infrastructure: ControlPlaneInfrastructure,
}

#[derive(Debug)]
pub(crate) struct RegistryMarketplaceReleaseProjection {
    pub(crate) id: String,
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) status: String,
    pub(crate) publisher_principal: serde_json::Value,
    pub(crate) checksum_sha256: Option<String>,
    pub(crate) default_locale: String,
    pub(crate) published_at: String,
}

#[derive(Debug)]
pub(crate) struct RegistryMarketplaceTranslationProjection {
    pub(crate) locale: String,
    pub(crate) name: String,
    pub(crate) description: String,
}

impl SeaOrmModuleGovernanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_infrastructure(db, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }

    /// Projects durable registry releases onto host-supplied marketplace
    /// entries. The host provides only its static catalog facts; release
    /// versions, trusted artifact references, localized metadata, and
    /// publisher identity remain registry-owner reads.
    pub async fn apply_marketplace_projection(
        &self,
        entries: Vec<ModuleMarketplaceEntry>,
        preferred_locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> Result<Vec<ModuleMarketplaceEntry>, ModuleGovernanceError> {
        let releases = self.marketplace_release_projections().await?;
        if releases.is_empty() {
            return Ok(entries);
        }

        let artifact_contracts = self
            .published_artifact_contracts()
            .await?
            .into_iter()
            .map(|contract| (contract.release_id, contract.artifact))
            .collect::<std::collections::HashMap<_, _>>();
        if releases.iter().any(|release| {
            release.status == "active" && !artifact_contracts.contains_key(&release.id)
        }) {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }

        let mut releases_by_slug =
            std::collections::HashMap::<String, Vec<RegistryMarketplaceReleaseProjection>>::new();
        for release in releases {
            releases_by_slug
                .entry(release.slug.clone())
                .or_default()
                .push(release);
        }

        let mut projected = entries;
        for entry in &mut projected {
            let Some(releases) = releases_by_slug.get(&entry.slug) else {
                continue;
            };

            let mut versions = releases
                .iter()
                .map(|release| ModuleMarketplaceVersion {
                    version: release.version.clone(),
                    changelog: None,
                    yanked: release.status == "yanked",
                    published_at: Some(release.published_at.clone()),
                    checksum_sha256: release.checksum_sha256.clone(),
                    signature_present: false,
                    artifact: artifact_contracts.get(&release.id).cloned(),
                })
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| {
                left.yanked
                    .cmp(&right.yanked)
                    .then_with(|| right.published_at.cmp(&left.published_at))
                    .then_with(|| marketplace_version_desc(&left.version, &right.version))
                    .then_with(|| right.version.cmp(&left.version))
            });

            if let Some(active_release) = releases.iter().find(|release| release.status == "active")
            {
                let metadata = self
                    .marketplace_release_metadata(
                        &active_release.id,
                        preferred_locale,
                        fallback_locale.or(Some(active_release.default_locale.as_str())),
                    )
                    .await?;
                if let Ok(content) = ModuleMarketplaceContentProjection::try_new(
                    &metadata.name,
                    &metadata.description,
                ) {
                    entry.name = content.name;
                    entry.description = content.description;
                }
                entry.latest_version = active_release.version.clone();
                entry.publisher = Some(marketplace_principal_label(
                    &active_release.publisher_principal,
                )?);
                entry.checksum_sha256 = active_release.checksum_sha256.clone();
                entry.update_available = entry
                    .installed_version
                    .as_ref()
                    .is_some_and(|installed_version| installed_version != &entry.latest_version);
            }
            entry.versions = versions;
        }

        Ok(projected)
    }

    /// Loads the complete owner-derived registry lifecycle projection for one
    /// module slug. Transport adapters must map this DTO without querying
    /// registry tables or recreating governance and validation-stage policy.
    pub async fn lifecycle_snapshot(
        &self,
        slug: &str,
    ) -> Result<Option<ModuleGovernanceLifecycleSnapshot>, ModuleGovernanceError> {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(ModuleGovernanceError::InvalidLifecycleQuery);
        }

        let backend = self.db.get_database_backend();
        let mark = |position| placeholder(backend, position);
        let owner_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal, \
                            CAST(bound_by_principal AS TEXT) AS bound_by_principal, \
                            bound_at, updated_at \
                     FROM registry_module_owners WHERE slug = {} LIMIT 1",
                    mark(1)
                ),
                vec![slug.into()],
            ))
            .await
            .map_err(store_error)?;
        let request_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id, revision, slug, version, status, artifact_origin, \
                            CAST(requested_by_principal AS TEXT) AS requested_by_principal, \
                            CAST(publisher_principal AS TEXT) AS publisher_principal, \
                            CAST(approved_by_principal AS TEXT) AS approved_by_principal, \
                            CAST(rejected_by_principal AS TEXT) AS rejected_by_principal, \
                            rejection_reason, \
                            CAST(changes_requested_by_principal AS TEXT) AS changes_requested_by_principal, \
                            changes_requested_reason, changes_requested_reason_code, \
                            changes_requested_at, \
                            CAST(held_by_principal AS TEXT) AS held_by_principal, \
                            held_reason, held_reason_code, held_at, held_from_status, \
                            CAST(validation_warnings AS TEXT) AS validation_warnings, \
                            CAST(validation_errors AS TEXT) AS validation_errors, \
                            validated_at, approved_at, created_at, updated_at, published_at \
                     FROM registry_publish_requests WHERE slug = {} \
                     ORDER BY created_at DESC LIMIT 1",
                    mark(1)
                ),
                vec![slug.into()],
            ))
            .await
            .map_err(store_error)?;
        let release_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT version, status, \
                            CAST(publisher_principal AS TEXT) AS publisher_principal, \
                            checksum_sha256, published_at, yanked_reason, \
                            CAST(yanked_by_principal AS TEXT) AS yanked_by_principal, yanked_at \
                     FROM registry_module_releases WHERE slug = {} \
                     ORDER BY published_at DESC LIMIT 1",
                    mark(1)
                ),
                vec![slug.into()],
            ))
            .await
            .map_err(store_error)?;
        let event_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id, event_type, \
                            CAST(actor_principal AS TEXT) AS actor_principal, \
                            CAST(publisher_principal AS TEXT) AS publisher_principal, \
                            CAST(details AS TEXT) AS details, created_at \
                     FROM registry_governance_events WHERE slug = {} \
                     ORDER BY created_at DESC, id DESC LIMIT 10",
                    mark(1)
                ),
                vec![slug.into()],
            ))
            .await
            .map_err(store_error)?;

        let owner_binding = owner_row
            .as_ref()
            .map(map_governance_owner_snapshot)
            .transpose()?;
        let latest_request = request_row
            .as_ref()
            .map(map_governance_request_row)
            .transpose()?;
        let latest_release = release_row
            .as_ref()
            .map(map_governance_release_snapshot)
            .transpose()?;
        let recent_event_rows = event_rows
            .iter()
            .map(map_governance_event_row)
            .collect::<Result<Vec<_>, _>>()?;

        let stage_rows = if let Some(request) = latest_request.as_ref() {
            self.db
                .query_all_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT stage_key, status, detail, attempt_number, updated_at, \
                                started_at, finished_at \
                         FROM registry_validation_stages WHERE request_id = {} \
                         ORDER BY attempt_number DESC, created_at DESC",
                        mark(1)
                    ),
                    vec![request.snapshot.id.clone().into()],
                ))
                .await
                .map_err(store_error)?
        } else {
            Vec::new()
        };

        if owner_binding.is_none()
            && latest_request.is_none()
            && latest_release.is_none()
            && recent_event_rows.is_empty()
            && stage_rows.is_empty()
        {
            return Ok(None);
        }

        let validation_stages = derive_governance_validation_stages(
            latest_request.as_ref(),
            &recent_event_rows,
            &stage_rows,
        )?;
        let follow_up_gates = derive_governance_follow_up_gates(
            latest_request.as_ref(),
            &recent_event_rows,
            &validation_stages,
        )?;
        let governance_actions = derive_governance_actions(
            latest_request.as_ref().map(|request| &request.snapshot),
            latest_release.as_ref(),
            owner_binding.as_ref(),
            &validation_stages,
        );

        Ok(Some(ModuleGovernanceLifecycleSnapshot {
            moderation_policy: ModuleGovernanceModerationPolicy {
                mode: "registry".to_string(),
                live_publish_supported: true,
                live_governance_supported: true,
                manual_review_required: true,
                restriction_reason_code: None,
                restriction_reason: String::new(),
            },
            owner_binding,
            latest_request: latest_request.map(|request| request.snapshot),
            latest_release,
            recent_events: recent_event_rows
                .into_iter()
                .map(|event| event.snapshot)
                .collect(),
            follow_up_gates,
            validation_stages,
            governance_actions,
        }))
    }

    /// Loads the exact durable owner binding for one module slug. Host
    /// adapters use this narrow query for authorization and transport mapping
    /// instead of reading `registry_module_owners` themselves.
    pub async fn owner_binding_snapshot(
        &self,
        slug: &str,
    ) -> Result<Option<ModuleGovernanceOwnerSnapshot>, ModuleGovernanceError> {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(ModuleGovernanceError::InvalidOwnerBindingQuery);
        }
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal, \
                            CAST(bound_by_principal AS TEXT) AS bound_by_principal, \
                            bound_at, updated_at \
                     FROM registry_module_owners WHERE slug = {} LIMIT 1",
                    placeholder(backend, 1)
                ),
                vec![slug.into()],
            ))
            .await
            .map_err(store_error)?;
        row.as_ref().map(map_governance_owner_snapshot).transpose()
    }

    /// Loads the exact host-only artifact-delivery facts for one publish
    /// request. A request without an attached artifact is intentionally
    /// indistinguishable from an absent request to a download transport.
    pub async fn publish_artifact_download_snapshot(
        &self,
        request_id: &str,
    ) -> Result<Option<ModuleGovernancePublishArtifactDownloadSnapshot>, ModuleGovernanceError>
    {
        let request_id = request_id.trim();
        if request_id.is_empty() {
            return Err(ModuleGovernanceError::InvalidPublishArtifactDownloadQuery);
        }
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT artifact_storage_key, artifact_content_type \
                     FROM registry_publish_requests WHERE id = {} LIMIT 1",
                    placeholder(backend, 1)
                ),
                vec![request_id.into()],
            ))
            .await
            .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(storage_key) = optional_column::<String>(&row, "artifact_storage_key")?
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        let content_type = optional_column::<String>(&row, "artifact_content_type")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        Ok(Some(ModuleGovernancePublishArtifactDownloadSnapshot {
            storage_key,
            content_type,
        }))
    }

    /// Loads the complete owner-derived status projection for one exact
    /// publish request. Unlike [`Self::lifecycle_snapshot`], this method never
    /// substitutes the latest request for the module slug.
    pub async fn publish_request_status_snapshot(
        &self,
        request_id: &str,
        actor: Option<&ModuleGovernanceActorContext>,
    ) -> Result<Option<ModuleGovernancePublishRequestStatusSnapshot>, ModuleGovernanceError> {
        let request_id = request_id.trim();
        if request_id.is_empty() {
            return Err(ModuleGovernanceError::InvalidPublishRequestStatusQuery);
        }

        let backend = self.db.get_database_backend();
        let mark = |position| placeholder(backend, position);
        let request_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, id, revision, version, status, artifact_origin, \
                            CAST(requested_by_principal AS TEXT) AS requested_by_principal, \
                            CAST(publisher_principal AS TEXT) AS publisher_principal, \
                            CAST(approved_by_principal AS TEXT) AS approved_by_principal, \
                            CAST(rejected_by_principal AS TEXT) AS rejected_by_principal, \
                            rejection_reason, \
                            CAST(changes_requested_by_principal AS TEXT) AS changes_requested_by_principal, \
                            changes_requested_reason, changes_requested_reason_code, \
                            changes_requested_at, \
                            CAST(held_by_principal AS TEXT) AS held_by_principal, \
                            held_reason, held_reason_code, held_at, held_from_status, \
                            CAST(validation_warnings AS TEXT) AS validation_warnings, \
                            CAST(validation_errors AS TEXT) AS validation_errors, \
                            validated_at, approved_at, created_at, updated_at, published_at \
                     FROM registry_publish_requests WHERE id = {} LIMIT 1",
                    mark(1)
                ),
                vec![request_id.into()],
            ))
            .await
            .map_err(store_error)?;
        let Some(request_row) = request_row else {
            return Ok(None);
        };
        let slug: String = required_column(&request_row, "slug")?;
        let request = map_governance_request_row(&request_row)?;

        let owner_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal, \
                            CAST(bound_by_principal AS TEXT) AS bound_by_principal, \
                            bound_at, updated_at \
                     FROM registry_module_owners WHERE slug = {} LIMIT 1",
                    mark(1)
                ),
                vec![slug.into()],
            ))
            .await
            .map_err(store_error)?;
        let owner_binding = owner_row
            .as_ref()
            .map(map_governance_owner_snapshot)
            .transpose()?;
        let stage_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT stage_key, status, detail, attempt_number, updated_at, \
                            started_at, finished_at \
                     FROM registry_validation_stages WHERE request_id = {} \
                     ORDER BY attempt_number DESC, created_at DESC",
                    mark(1)
                ),
                vec![request.snapshot.id.clone().into()],
            ))
            .await
            .map_err(store_error)?;
        let validation_stages =
            derive_governance_validation_stages(Some(&request), &[], &stage_rows)?;
        let follow_up_gates =
            derive_governance_follow_up_gates(Some(&request), &[], &validation_stages)?;
        let approval_override_required = request.snapshot.status == "approved"
            && validation_stages
                .iter()
                .any(|stage| !stage.status.eq_ignore_ascii_case("passed"));
        let pending_stage_labels = governance_pending_stage_labels(&validation_stages);
        let next_action =
            publish_request_next_action(&request.snapshot, approval_override_required)?;
        let approval_override_warning = approval_override_required.then(|| {
            format!(
                "Approval override is required because these follow-up validation stages are not passed yet: {}. Live approve must include both reason and reason_code ({}).",
                pending_stage_labels.join(", "),
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
            )
        });
        let authorization = ModuleGovernanceRequestAuthorizationSnapshot {
            can_manage: governance_actor_can_manage_request(
                &request.snapshot,
                owner_binding.as_ref(),
                actor,
            ),
            can_review: governance_actor_can_review_request(owner_binding.as_ref(), actor),
        };
        let effective_publisher_principal = owner_binding
            .as_ref()
            .map(|owner| owner.owner_principal.clone())
            .or_else(|| request.snapshot.publisher_principal.clone())
            .or_else(|| actor.map(|actor| actor.principal.clone()));
        let rejected_retry_allowed = if request.snapshot.status == "rejected" {
            let latest_event_type = self
                .db
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT event_type FROM registry_governance_events \
                         WHERE request_id = {} ORDER BY created_at DESC LIMIT 1",
                        mark(1)
                    ),
                    vec![request.snapshot.id.clone().into()],
                ))
                .await
                .map_err(store_error)?
                .map(|row| required_column::<String>(&row, "event_type"))
                .transpose()?;
            governance_rejected_request_can_retry(
                latest_event_type.as_deref(),
                request.snapshot.rejection_reason.as_deref(),
            )
        } else {
            false
        };
        let governance_actions = derive_governance_request_actions(
            &request.snapshot,
            &authorization,
            approval_override_required,
        );
        let accepted = request.snapshot.status != "rejected";

        Ok(Some(ModuleGovernancePublishRequestStatusSnapshot {
            request: request.snapshot,
            authorization,
            effective_publisher_principal,
            rejected_retry_allowed,
            follow_up_gates,
            validation_stages: validation_stages.clone(),
            approval_override_required,
            approval_override_reason_codes: REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            approval_override_warning,
            governance_actions,
            accepted,
            next_action,
        }))
    }

}
