use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::models::registry_governance_event::{
    self, ActiveModel as RegistryGovernanceEventActiveModel,
    Entity as RegistryGovernanceEventEntity,
};
use crate::models::registry_module_owner::{
    self, ActiveModel as RegistryModuleOwnerActiveModel, Entity as RegistryModuleOwnerEntity,
};
use crate::models::registry_module_release::{
    self, ActiveModel as RegistryModuleReleaseActiveModel, Entity as RegistryModuleReleaseEntity,
    RegistryModuleReleaseStatus,
};
use crate::models::registry_publish_request::{
    self, ActiveModel as RegistryPublishRequestActiveModel, Entity as RegistryPublishRequestEntity,
    RegistryPublishRequestStatus,
};
use crate::models::registry_validation_job::{
    self, ActiveModel as RegistryValidationJobActiveModel, Entity as RegistryValidationJobEntity,
    RegistryValidationJobStatus,
};
use crate::models::registry_validation_stage::{
    self, ActiveModel as RegistryValidationStageActiveModel,
    Entity as RegistryValidationStageEntity, RegistryValidationStageStatus,
};
use crate::modules::{CatalogManifestModule, CatalogModuleVersion};
use crate::services::marketplace_catalog::{
    RegistryPublishMarketplaceRequest, RegistryPublishRequest, RegistryPublishUiPackagesRequest,
    REGISTRY_MUTATION_SCHEMA_VERSION,
};

const REGISTRY_ARTIFACT_BUNDLE_TYPE: &str = "rustok-module-publish-bundle";
const REGISTRY_VALIDATION_FOLLOW_UP_GATES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];
const REGISTRY_VALIDATION_LOAD_RETRY_DELAYS_SECONDS: &[u64] = &[1, 3, 5];
pub const REGISTRY_YANK_REASON_CODES: &[&str] = &[
    "security",
    "legal",
    "malware",
    "critical_regression",
    "rollback",
    "other",
];
pub const REGISTRY_REJECT_REASON_CODES: &[&str] = &[
    "policy_mismatch",
    "quality_gate_failed",
    "ownership_mismatch",
    "security_risk",
    "legal",
    "other",
];
pub const REGISTRY_OWNER_TRANSFER_REASON_CODES: &[&str] = &[
    "maintenance_handoff",
    "team_restructure",
    "publisher_rotation",
    "security_emergency",
    "governance_override",
    "other",
];
pub const REGISTRY_APPROVE_OVERRIDE_REASON_CODES: &[&str] = &[
    "manual_review_complete",
    "trusted_first_party",
    "expedited_release",
    "governance_override",
    "other",
];
pub const REGISTRY_REQUEST_CHANGES_REASON_CODES: &[&str] = &[
    "artifact_mismatch",
    "quality_gap",
    "policy_gap",
    "docs_gap",
    "other",
];
pub const REGISTRY_HOLD_REASON_CODES: &[&str] = &[
    "release_window",
    "incident",
    "legal_hold",
    "security_review",
    "other",
];
pub const REGISTRY_RESUME_REASON_CODES: &[&str] = &[
    "review_complete",
    "incident_closed",
    "legal_cleared",
    "other",
];
pub const REGISTRY_MODERATION_POLICY_REASON_CODE_THIRD_PARTY_FLOW_PENDING: &str =
    "third_party_flow_pending";
pub const REGISTRY_VALIDATION_STAGE_REASON_CODES: &[&str] = &[
    "local_runner_passed",
    "manual_review_complete",
    "build_failure",
    "test_failure",
    "policy_preflight_failed",
    "security_findings",
    "policy_exception",
    "license_issue",
    "manual_override",
    "other",
];

#[derive(Debug, Clone)]
pub struct RegistryArtifactUpload {
    pub content_type: String,
    pub bytes: bytes::Bytes,
}

#[derive(Debug, Clone)]
pub struct RegistryGovernanceService {
    db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationQueueResult {
    pub request: registry_publish_request::Model,
    pub queued: bool,
    pub validation_job_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RegistryValidationJobClaim {
    job: registry_validation_job::Model,
    request: registry_publish_request::Model,
    should_run: bool,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationStageClaim {
    pub request: registry_publish_request::Model,
    pub stage: registry_validation_stage::Model,
    pub claim_id: String,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationStageMutationResult {
    pub request: registry_publish_request::Model,
    pub stage: registry_validation_stage::Model,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishRequestSnapshot {
    pub id: String,
    pub status: String,
    pub requested_by: String,
    pub publisher_identity: Option<String>,
    pub approved_by: Option<String>,
    pub rejected_by: Option<String>,
    pub rejection_reason: Option<String>,
    pub changes_requested_by: Option<String>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<String>,
    pub held_by: Option<String>,
    pub held_reason: Option<String>,
    pub held_reason_code: Option<String>,
    pub held_at: Option<String>,
    pub held_from_status: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleReleaseSnapshot {
    pub version: String,
    pub status: String,
    pub publisher: String,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<String>,
    pub yanked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleOwnerSnapshot {
    pub owner_actor: String,
    pub bound_by: String,
    pub bound_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RegistryGovernanceEventSnapshot {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub publisher: Option<String>,
    pub details: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct RegistryFollowUpGateSnapshot {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationStageSnapshot {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub attempt_number: i32,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub execution_mode: String,
    pub runnable: bool,
    pub requires_manual_confirmation: bool,
    pub allowed_terminal_reason_codes: Vec<String>,
    pub suggested_pass_reason_code: Option<String>,
    pub suggested_failure_reason_code: Option<String>,
    pub suggested_blocked_reason_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModerationPolicySnapshot {
    pub mode: String,
    pub live_publish_supported: bool,
    pub live_governance_supported: bool,
    pub manual_review_required: bool,
    pub restriction_reason_code: Option<String>,
    pub restriction_reason: String,
}

#[derive(Debug, Clone)]
pub struct RegistryGovernanceActionSnapshot {
    pub key: String,
    pub enabled: bool,
    pub reason: Option<String>,
    pub supported_reason_codes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleLifecycleSnapshot {
    pub moderation_policy: RegistryModerationPolicySnapshot,
    pub owner_binding: Option<RegistryModuleOwnerSnapshot>,
    pub latest_request: Option<RegistryPublishRequestSnapshot>,
    pub latest_release: Option<RegistryModuleReleaseSnapshot>,
    pub recent_events: Vec<RegistryGovernanceEventSnapshot>,
    pub follow_up_gates: Vec<RegistryFollowUpGateSnapshot>,
    pub validation_stages: Vec<RegistryValidationStageSnapshot>,
    pub governance_actions: Vec<RegistryGovernanceActionSnapshot>,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishRequestFollowUpSnapshot {
    pub follow_up_gates: Vec<RegistryFollowUpGateSnapshot>,
    pub validation_stages: Vec<RegistryValidationStageSnapshot>,
    pub approval_override_required: bool,
}

#[derive(Debug, Clone)]
pub struct RegistryRemoteValidationStageClaim {
    pub claim_id: String,
    pub runner_id: String,
    pub request: registry_publish_request::Model,
    pub stage: registry_validation_stage::Model,
}

#[derive(Debug, Default)]
struct RegistryArtifactValidation {
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RegistryValidationCheckDetail {
    key: String,
    status: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactBundle {
    schema_version: u32,
    artifact_type: String,
    module: RegistryPublishArtifactModule,
    files: RegistryPublishArtifactFiles,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactModule {
    slug: String,
    version: String,
    crate_name: String,
    module_name: String,
    module_description: String,
    ownership: String,
    trust_level: String,
    license: String,
    module_entry_type: Option<String>,
    marketplace: RegistryPublishArtifactMarketplace,
    ui_packages: RegistryPublishArtifactUiPackages,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryPublishArtifactMarketplace {
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryPublishArtifactUiPackages {
    admin: Option<RegistryPublishArtifactUiPackage>,
    storefront: Option<RegistryPublishArtifactUiPackage>,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactUiPackage {
    crate_name: String,
    #[allow(dead_code)]
    manifest_path: String,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactFiles {
    #[serde(rename = "rustok-module.toml")]
    package_manifest: Option<String>,
    #[serde(rename = "Cargo.toml")]
    crate_manifest: Option<String>,
    #[serde(rename = "admin/Cargo.toml")]
    admin_manifest: Option<String>,
    #[serde(rename = "storefront/Cargo.toml")]
    storefront_manifest: Option<String>,
}

impl RegistryGovernanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create_publish_request(
        &self,
        request: &RegistryPublishRequest,
        requested_by: &str,
        publisher: Option<&str>,
        warnings: &[String],
    ) -> anyhow::Result<registry_publish_request::Model> {
        self.ensure_actor_can_create_publish_request(requested_by, publisher, &request.module.slug)
            .await?;

        let existing_active_release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(&request.module.slug))
            .filter(registry_module_release::Column::Version.eq(&request.module.version))
            .filter(registry_module_release::Column::Status.eq(RegistryModuleReleaseStatus::Active))
            .one(&self.db)
            .await?;
        if existing_active_release.is_some() {
            anyhow::bail!(
                "Published release '{}@{}' already exists",
                request.module.slug,
                request.module.version
            );
        }

        let now = Utc::now();
        let request_id = format!("rpr_{}", uuid::Uuid::new_v4().simple());
        let model = registry_publish_request::Model {
            id: request_id.clone(),
            slug: request.module.slug.clone(),
            version: request.module.version.clone(),
            crate_name: request.module.crate_name.clone(),
            module_name: request.module.name.clone(),
            description: request.module.description.clone(),
            ownership: request.module.ownership.clone(),
            trust_level: request.module.trust_level.clone(),
            license: request.module.license.clone(),
            entry_type: request.module.entry_type.clone(),
            marketplace: serde_json::to_value(&request.module.marketplace)
                .context("failed to serialize registry publish marketplace metadata")?,
            ui_packages: serde_json::to_value(&request.module.ui_packages)
                .context("failed to serialize registry publish ui_packages metadata")?,
            status: RegistryPublishRequestStatus::Draft,
            requested_by: normalize_actor(requested_by),
            publisher_identity: normalize_optional_actor(publisher),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            changes_requested_by: None,
            changes_requested_reason: None,
            changes_requested_reason_code: None,
            changes_requested_at: None,
            held_by: None,
            held_reason: None,
            held_reason_code: None,
            held_at: None,
            held_from_status: None,
            validation_warnings: serde_json::to_value(warnings)
                .context("failed to serialize registry publish warnings")?,
            validation_errors: serde_json::json!([]),
            artifact_path: None,
            artifact_url: None,
            artifact_checksum_sha256: None,
            artifact_size: None,
            artifact_content_type: None,
            submitted_at: None,
            validated_at: None,
            approved_at: None,
            published_at: None,
            created_at: now,
            updated_at: now,
        };

        let active_model = RegistryPublishRequestActiveModel {
            id: Set(model.id.clone()),
            slug: Set(model.slug.clone()),
            version: Set(model.version.clone()),
            crate_name: Set(model.crate_name.clone()),
            module_name: Set(model.module_name.clone()),
            description: Set(model.description.clone()),
            ownership: Set(model.ownership.clone()),
            trust_level: Set(model.trust_level.clone()),
            license: Set(model.license.clone()),
            entry_type: Set(model.entry_type.clone()),
            marketplace: Set(model.marketplace.clone()),
            ui_packages: Set(model.ui_packages.clone()),
            status: Set(model.status.clone()),
            requested_by: Set(model.requested_by.clone()),
            publisher_identity: Set(model.publisher_identity.clone()),
            approved_by: Set(None),
            rejected_by: Set(None),
            rejection_reason: Set(None),
            changes_requested_by: Set(None),
            changes_requested_reason: Set(None),
            changes_requested_reason_code: Set(None),
            changes_requested_at: Set(None),
            held_by: Set(None),
            held_reason: Set(None),
            held_reason_code: Set(None),
            held_at: Set(None),
            held_from_status: Set(None),
            validation_warnings: Set(model.validation_warnings.clone()),
            validation_errors: Set(model.validation_errors.clone()),
            artifact_path: Set(None),
            artifact_url: Set(None),
            artifact_checksum_sha256: Set(None),
            artifact_size: Set(None),
            artifact_content_type: Set(None),
            submitted_at: Set(None),
            validated_at: Set(None),
            approved_at: Set(None),
            published_at: Set(None),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
        };

        active_model.insert(&self.db).await?;
        self.record_governance_event(
            &model.slug,
            Some(&model.id),
            None,
            "request_created",
            requested_by,
            publisher,
            serde_json::json!({
                "version": model.version.clone(),
                "status": request_status_label(RegistryPublishRequestStatus::Draft),
                "warnings": warnings,
            }),
        )
        .await?;
        self.get_publish_request(&request_id)
            .await?
            .ok_or_else(|| anyhow!("registry publish request disappeared after insert"))
    }

    pub async fn get_publish_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_publish_request::Model>> {
        Ok(RegistryPublishRequestEntity::find_by_id(request_id)
            .one(&self.db)
            .await?)
    }

    pub async fn upload_publish_artifact(
        &self,
        request_id: &str,
        actor: &str,
        artifact: RegistryArtifactUpload,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_manage_publish_request(actor, &request, "upload an artifact for")
            .await?;
        let was_changes_requested =
            request.status == RegistryPublishRequestStatus::ChangesRequested;
        if !matches!(
            request.status,
            RegistryPublishRequestStatus::Draft | RegistryPublishRequestStatus::ChangesRequested
        ) {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and can no longer accept an artifact upload",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let checksum = hex::encode(Sha256::digest(&artifact.bytes));
        let stored = store_registry_artifact(&request, &artifact)
            .await
            .context("failed to persist registry artifact")?;
        let artifact_uploaded_at = Utc::now();

        let mut request_active: RegistryPublishRequestActiveModel = request.clone().into();
        request_active.status = Set(RegistryPublishRequestStatus::ArtifactUploaded);
        request_active.artifact_path = Set(Some(stored.artifact_path.clone()));
        request_active.artifact_url = Set(Some(stored.artifact_url.clone()));
        request_active.artifact_checksum_sha256 = Set(Some(checksum.clone()));
        request_active.artifact_size = Set(Some(stored.artifact_size));
        request_active.artifact_content_type = Set(Some(artifact.content_type.clone()));
        request_active.updated_at = Set(artifact_uploaded_at);
        let request = request_active.update(&self.db).await?;

        let mut warnings = deserialize_message_list(&request.validation_warnings);
        let upload_actor = normalize_actor(actor);
        if upload_actor != request.requested_by {
            warnings.push(format!(
                "Artifact was uploaded by '{}' for publish request originally created by '{}'.",
                upload_actor, request.requested_by
            ));
        }
        let warnings = dedupe_message_list(warnings);

        let submitted_at = Utc::now();
        RegistryValidationJobEntity::delete_many()
            .filter(registry_validation_job::Column::RequestId.eq(request.id.clone()))
            .exec(&self.db)
            .await?;
        RegistryValidationStageEntity::delete_many()
            .filter(registry_validation_stage::Column::RequestId.eq(request.id.clone()))
            .exec(&self.db)
            .await?;

        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Submitted);
        request_active.submitted_at = Set(Some(submitted_at));
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.approved_by = Set(None);
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.changes_requested_by = Set(None);
        request_active.changes_requested_reason = Set(None);
        request_active.changes_requested_reason_code = Set(None);
        request_active.changes_requested_at = Set(None);
        request_active.held_by = Set(None);
        request_active.held_reason = Set(None);
        request_active.held_reason_code = Set(None);
        request_active.held_at = Set(None);
        request_active.held_from_status = Set(None);
        request_active.validated_at = Set(None);
        request_active.approved_at = Set(None);
        request_active.published_at = Set(None);
        request_active.updated_at = Set(submitted_at);
        let request = request_active
            .update(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            if was_changes_requested {
                "artifact_reuploaded_after_changes_requested"
            } else {
                "artifact_uploaded"
            },
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "artifact_size": request.artifact_size,
                "content_type": request.artifact_content_type.clone(),
                "checksum_sha256": request.artifact_checksum_sha256.clone(),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn validate_publish_request(
        &self,
        request_id: &str,
        actor: &str,
    ) -> anyhow::Result<RegistryValidationQueueResult> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_manage_publish_request(actor, &request, "validate")
            .await?;

        let was_requeued = match request.status {
            RegistryPublishRequestStatus::Rejected => {
                let latest_event_type = self.latest_request_event_type(&request.id).await?;
                if rejected_publish_request_can_retry(
                    latest_event_type.as_deref(),
                    request.rejection_reason.as_deref(),
                ) {
                    true
                } else {
                    anyhow::bail!(
                        "Registry publish request '{}' was manually rejected by governance review and cannot be revalidated; create a new publish request instead",
                        request_id
                    );
                }
            }
            _ => false,
        };

        match request.status {
            RegistryPublishRequestStatus::Draft => {
                anyhow::bail!(
                    "Registry publish request '{}' is still in draft status and must receive an artifact upload before validation",
                    request_id
                );
            }
            RegistryPublishRequestStatus::ChangesRequested => {
                anyhow::bail!(
                    "Registry publish request '{}' requires a fresh artifact upload before validation can resume",
                    request_id
                );
            }
            RegistryPublishRequestStatus::OnHold => {
                anyhow::bail!(
                    "Registry publish request '{}' is currently on hold and must be resumed before validation can continue",
                    request_id
                );
            }
            RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published => {
                return Ok(RegistryValidationQueueResult {
                    request,
                    queued: false,
                    validation_job_id: None,
                });
            }
            RegistryPublishRequestStatus::Validating => {
                if let Some(existing_job) = self.latest_active_validation_job(&request.id).await? {
                    return Ok(RegistryValidationQueueResult {
                        request,
                        queued: false,
                        validation_job_id: Some(existing_job.id),
                    });
                }

                let job = self
                    .create_validation_job(&request, actor, "validation_resumed")
                    .await?;
                self.record_governance_event(
                    &request.slug,
                    Some(&request.id),
                    None,
                    "validation_job_queued",
                    actor,
                    None,
                    serde_json::json!({
                        "job_id": job.id.clone(),
                        "attempt_number": job.attempt_number,
                        "queue_reason": job.queue_reason.clone(),
                        "request_status": request_status_label(request.status.clone()),
                        "version": request.version.clone(),
                    }),
                )
                .await?;
                return Ok(RegistryValidationQueueResult {
                    request,
                    queued: true,
                    validation_job_id: Some(job.id),
                });
            }
            RegistryPublishRequestStatus::Rejected
            | RegistryPublishRequestStatus::ArtifactUploaded
            | RegistryPublishRequestStatus::Submitted => {}
        }

        let validating_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Validating);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.changes_requested_by = Set(None);
        request_active.changes_requested_reason = Set(None);
        request_active.changes_requested_reason_code = Set(None);
        request_active.changes_requested_at = Set(None);
        request_active.validated_at = Set(None);
        request_active.updated_at = Set(validating_at);
        let request = request_active.update(&self.db).await?;
        let job = self
            .create_validation_job(
                &request,
                actor,
                if was_requeued {
                    "requeued_after_validation_failed"
                } else {
                    "initial_validation"
                },
            )
            .await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            if was_requeued {
                "validation_requeued"
            } else {
                "validation_queued"
            },
            actor,
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "requeued": was_requeued,
                "follow_up_gates": follow_up_validation_gate_details(),
            }),
        )
        .await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_job_queued",
            actor,
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
            }),
        )
        .await?;

        Ok(RegistryValidationQueueResult {
            request,
            queued: true,
            validation_job_id: Some(job.id),
        })
    }

    pub async fn run_publish_validation_job(
        &self,
        validation_job_id: &str,
        actor: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let claim = self
            .claim_validation_job(validation_job_id, actor)
            .await?
            .ok_or_else(|| {
                anyhow!("Registry validation job '{validation_job_id}' was not found")
            })?;
        if !claim.should_run {
            return Ok(claim.request);
        }
        let job = claim.job;
        let request = claim.request;

        let mut artifact_load_attempt = 1usize;
        let artifact = loop {
            match load_registry_artifact(&request).await {
                Ok(artifact) => break artifact,
                Err(error) => {
                    let existing_warnings = deserialize_message_list(&request.validation_warnings);
                    if let Some(retry_after_seconds) =
                        validation_retry_delay_seconds(artifact_load_attempt)
                    {
                        self.record_governance_event(
                            &request.slug,
                            Some(&request.id),
                            None,
                            "validation_retry_scheduled",
                            actor,
                            None,
                            serde_json::json!({
                                "job_id": job.id.clone(),
                                "job_attempt": job.attempt_number,
                                "version": request.version.clone(),
                                "status": request_status_label(request.status.clone()),
                                "attempt": artifact_load_attempt,
                                "next_attempt": artifact_load_attempt + 1,
                                "retry_after_seconds": retry_after_seconds,
                                "error": error.to_string(),
                            }),
                        )
                        .await?;
                        tokio::time::sleep(std::time::Duration::from_secs(retry_after_seconds))
                            .await;
                        artifact_load_attempt += 1;
                        continue;
                    }

                    self.record_governance_event(
                        &request.slug,
                        Some(&request.id),
                        None,
                        "validation_retry_exhausted",
                        actor,
                        None,
                        serde_json::json!({
                            "job_id": job.id.clone(),
                            "job_attempt": job.attempt_number,
                            "version": request.version.clone(),
                            "status": request_status_label(request.status.clone()),
                            "attempt": artifact_load_attempt,
                            "max_attempts": artifact_load_attempt,
                            "error": error.to_string(),
                        }),
                    )
                    .await?;
                    let errors = vec![format!(
                        "Validation job exhausted artifact-load retries before bundle checks: {error}"
                    )];
                    let request = self
                        .mark_validation_job_failed(
                            job.clone(),
                            actor,
                            Some(errors[0].as_str()),
                            &request,
                        )
                        .await?;
                    return self
                        .store_validation_rejection(
                            request,
                            actor,
                            &existing_warnings,
                            &errors,
                            validation_failed_check_details(&errors),
                        )
                        .await;
                }
            }
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);
        let mut warnings = deserialize_message_list(&request.validation_warnings);
        if artifact_load_attempt > 1 {
            warnings.push(format!(
                "Validation artifact load succeeded after retry attempt {}.",
                artifact_load_attempt
            ));
        }
        warnings.extend(validation.warnings);
        let warnings = dedupe_message_list(warnings);

        if !validation.errors.is_empty() {
            let errors = dedupe_message_list(validation.errors);
            let request = self
                .mark_validation_job_failed(
                    job.clone(),
                    actor,
                    errors.first().map(String::as_str),
                    &request,
                )
                .await?;
            return self
                .store_validation_rejection(
                    request,
                    actor,
                    &warnings,
                    &errors,
                    validation_failed_check_details(&errors),
                )
                .await;
        }

        let mut warnings = warnings;
        warnings.push(follow_up_validation_warning().to_string());
        let warnings = dedupe_message_list(warnings);
        let automated_checks = validation_passed_check_details();
        let follow_up_gates = follow_up_validation_gate_details();
        let approved_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Approved);
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.changes_requested_by = Set(None);
        request_active.changes_requested_reason = Set(None);
        request_active.changes_requested_reason_code = Set(None);
        request_active.changes_requested_at = Set(None);
        request_active.held_by = Set(None);
        request_active.held_reason = Set(None);
        request_active.held_reason_code = Set(None);
        request_active.held_at = Set(None);
        request_active.held_from_status = Set(None);
        request_active.validated_at = Set(Some(approved_at));
        request_active.approved_by = Set(Some(normalize_actor(actor)));
        request_active.approved_at = Set(Some(approved_at));
        request_active.updated_at = Set(approved_at);
        let request = request_active.update(&self.db).await?;
        let queued_stages = self
            .queue_follow_up_validation_stages(&request, actor, "validation_passed")
            .await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_passed",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "warnings": warnings.clone(),
                "automated_checks": automated_checks,
                "follow_up_gates": follow_up_gates,
                "validation_stages": queued_stages
                    .iter()
                    .map(validation_stage_details_value)
                    .collect::<Vec<_>>(),
            }),
        )
        .await?;
        self.mark_validation_job_succeeded(job, actor, &request)
            .await?;
        Ok(request)
    }

    pub async fn report_validation_stage(
        &self,
        request_id: &str,
        actor: &str,
        stage_key: &str,
        status: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
        requeue: bool,
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let stage_key = normalize_validation_stage_key(stage_key)?;
        let requested_status = parse_validation_stage_status(status)?;
        let normalized_reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        if let Some(reason_code) = normalized_reason_code.as_deref() {
            if !REGISTRY_VALIDATION_STAGE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
            {
                anyhow::bail!(
                    "Validation stage reason_code '{}' is not supported; expected one of {}",
                    reason_code,
                    REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
                );
            }
            if matches!(
                requested_status,
                RegistryValidationStageStatus::Passed
                    | RegistryValidationStageStatus::Failed
                    | RegistryValidationStageStatus::Blocked
            ) && !validation_stage_allowed_terminal_reason_codes(stage_key)
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
            {
                anyhow::bail!(
                    "Validation stage '{}' reason_code '{}' is not supported for terminal status '{}'; expected one of {}",
                    stage_key,
                    reason_code,
                    validation_stage_status_label(requested_status.clone()),
                    validation_stage_allowed_terminal_reason_codes(stage_key).join(", ")
                );
            }
        }
        if requeue && requested_status != RegistryValidationStageStatus::Queued {
            anyhow::bail!(
                "Validation stage requeue for '{}' requires status 'queued'",
                stage_key
            );
        }
        if !requeue && requested_status == RegistryValidationStageStatus::Queued {
            anyhow::bail!(
                "Validation stage '{}' can only move back to 'queued' via requeue=true",
                stage_key
            );
        }

        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_review_publish_request(actor, &request, "update validation stage")
            .await?;
        if !matches!(
            request.status,
            RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
        ) {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot accept follow-up stage updates",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let detail = detail
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| default_validation_stage_detail(stage_key, &requested_status));

        let stage = if requeue {
            self.queue_validation_stage_attempt(
                &request,
                stage_key,
                actor,
                "manual_requeue",
                &detail,
            )
            .await?
        } else {
            let latest_stage = self
                .latest_validation_stage(&request.id, stage_key)
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "Validation stage '{}' has not been queued yet for request '{}'",
                        stage_key,
                        request_id
                    )
                })?;
            self.update_validation_stage_status(
                latest_stage,
                &request,
                actor,
                requested_status,
                &detail,
                normalized_reason_code.as_deref(),
            )
            .await?
        };

        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub async fn claim_next_queued_validation_stage(
        &self,
        actor: &str,
        supported_stages: &[&str],
    ) -> anyhow::Result<Option<RegistryValidationStageClaim>> {
        self.claim_next_validation_stage_with_runner(
            actor,
            supported_stages,
            "local",
            chrono::Duration::seconds(60),
        )
        .await
    }

    pub async fn claim_next_validation_stage_with_runner(
        &self,
        actor: &str,
        supported_stages: &[&str],
        runner_kind: &str,
        lease_ttl: chrono::Duration,
    ) -> anyhow::Result<Option<RegistryValidationStageClaim>> {
        if supported_stages.is_empty() {
            return Ok(None);
        }

        self.requeue_expired_validation_stage_claims(actor).await?;

        let queued_stages = RegistryValidationStageEntity::find()
            .filter(
                registry_validation_stage::Column::Status.eq(RegistryValidationStageStatus::Queued),
            )
            .order_by_asc(registry_validation_stage::Column::CreatedAt)
            .all(&self.db)
            .await?;

        for queued_stage in queued_stages {
            if !supported_stages
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(&queued_stage.stage_key))
            {
                continue;
            }

            let Some(request) = self.get_publish_request(&queued_stage.request_id).await? else {
                continue;
            };
            if !matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            ) {
                continue;
            }

            let Some(latest_stage) = self
                .latest_validation_stage(&request.id, &queued_stage.stage_key)
                .await?
            else {
                continue;
            };
            if latest_stage.id != queued_stage.id {
                continue;
            }

            let stage = self
                .update_validation_stage_status(
                    queued_stage,
                    &request,
                    actor,
                    RegistryValidationStageStatus::Running,
                    &default_validation_stage_detail(
                        latest_stage.stage_key.as_str(),
                        &RegistryValidationStageStatus::Running,
                    ),
                    None,
                )
                .await?;
            let claim_id = format!("rvc_{}", uuid::Uuid::new_v4().simple());
            let lease_expiry = Utc::now() + lease_ttl;
            let mut stage_active: RegistryValidationStageActiveModel = stage.into();
            stage_active.claim_id = Set(Some(claim_id.clone()));
            stage_active.claimed_by = Set(Some(normalize_actor(actor)));
            stage_active.claim_expires_at = Set(Some(lease_expiry));
            stage_active.last_heartbeat_at = Set(Some(Utc::now()));
            stage_active.runner_kind = Set(Some(runner_kind.trim().to_ascii_lowercase()));
            stage_active.updated_at = Set(Utc::now());
            let stage = stage_active.update(&self.db).await?;
            return Ok(Some(RegistryValidationStageClaim {
                request,
                stage,
                claim_id,
            }));
        }

        Ok(None)
    }

    pub async fn complete_claimed_validation_stage(
        &self,
        claim: RegistryValidationStageClaim,
        actor: &str,
        status: RegistryValidationStageStatus,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        let Some(latest_stage) = self
            .latest_validation_stage(&claim.request.id, &claim.stage.stage_key)
            .await?
        else {
            return Ok(None);
        };
        if latest_stage.id != claim.stage.id
            || latest_stage
                .claim_id
                .as_deref()
                .is_none_or(|value| value != claim.claim_id)
        {
            return Ok(None);
        }

        let stage = self
            .update_validation_stage_status(
                latest_stage,
                &claim.request,
                actor,
                status,
                detail,
                reason_code,
            )
            .await?;
        let mut stage_active: RegistryValidationStageActiveModel = stage.into();
        stage_active.claim_id = Set(None);
        stage_active.claimed_by = Set(None);
        stage_active.claim_expires_at = Set(None);
        stage_active.last_heartbeat_at = Set(None);
        stage_active.runner_kind = Set(None);
        stage_active.updated_at = Set(Utc::now());
        Ok(Some(stage_active.update(&self.db).await?))
    }

    pub async fn claim_next_remote_validation_stage(
        &self,
        runner_id: &str,
        supported_stages: &[String],
        lease_ttl: chrono::Duration,
    ) -> anyhow::Result<Option<RegistryRemoteValidationStageClaim>> {
        let runner_actor = remote_runner_actor(runner_id);
        let supported_refs = supported_stages
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let Some(claim) = self
            .claim_next_validation_stage_with_runner(
                &runner_actor,
                &supported_refs,
                "remote",
                lease_ttl,
            )
            .await?
        else {
            return Ok(None);
        };

        Ok(Some(RegistryRemoteValidationStageClaim {
            claim_id: claim.claim_id,
            runner_id: runner_id.trim().to_string(),
            request: claim.request,
            stage: claim.stage,
        }))
    }

    pub async fn heartbeat_remote_validation_stage_claim(
        &self,
        claim_id: &str,
        runner_id: &str,
        lease_ttl: chrono::Duration,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        let stage = self
            .validation_stage_by_claim_id(claim_id)
            .await?
            .ok_or_else(|| anyhow!("Registry validation claim '{}' was not found", claim_id))?;
        self.ensure_remote_claim_belongs_to_runner(&stage, claim_id, runner_id)?;
        let now = Utc::now();
        let mut active: RegistryValidationStageActiveModel = stage.into();
        active.claim_expires_at = Set(Some(now + lease_ttl));
        active.last_heartbeat_at = Set(Some(now));
        active.updated_at = Set(now);
        Ok(active.update(&self.db).await?)
    }

    pub async fn complete_remote_validation_stage_claim(
        &self,
        claim_id: &str,
        runner_id: &str,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        self.finish_remote_validation_stage_claim(
            claim_id,
            runner_id,
            RegistryValidationStageStatus::Passed,
            detail,
            reason_code,
        )
        .await
    }

    pub async fn fail_remote_validation_stage_claim(
        &self,
        claim_id: &str,
        runner_id: &str,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        self.finish_remote_validation_stage_claim(
            claim_id,
            runner_id,
            RegistryValidationStageStatus::Failed,
            detail,
            reason_code,
        )
        .await
    }

    pub async fn requeue_expired_validation_stage_claims(
        &self,
        actor: &str,
    ) -> anyhow::Result<usize> {
        let now = Utc::now();
        let running = RegistryValidationStageEntity::find()
            .filter(
                registry_validation_stage::Column::Status
                    .eq(RegistryValidationStageStatus::Running),
            )
            .filter(registry_validation_stage::Column::ClaimExpiresAt.lte(now))
            .order_by_asc(registry_validation_stage::Column::ClaimExpiresAt)
            .all(&self.db)
            .await?;
        let mut requeued = 0usize;
        for stage in running {
            let Some(request) = self.get_publish_request(&stage.request_id).await? else {
                continue;
            };
            let latest = self
                .latest_validation_stage(&request.id, &stage.stage_key)
                .await?;
            if latest.as_ref().is_none_or(|latest| latest.id != stage.id) {
                continue;
            }
            let detail = format!(
                "Validation stage '{}' lease expired; the claim was returned to the queue.",
                stage.stage_key
            );
            let stage = self
                .update_validation_stage_status(
                    stage,
                    &request,
                    actor,
                    RegistryValidationStageStatus::Queued,
                    &detail,
                    None,
                )
                .await?;
            let mut active: RegistryValidationStageActiveModel = stage.into();
            active.claim_id = Set(None);
            active.claimed_by = Set(None);
            active.claim_expires_at = Set(None);
            active.last_heartbeat_at = Set(None);
            active.runner_kind = Set(None);
            active.updated_at = Set(Utc::now());
            active.update(&self.db).await?;
            requeued += 1;
        }
        Ok(requeued)
    }

    pub async fn approve_publish_request(
        &self,
        request_id: &str,
        actor: &str,
        publisher: Option<&str>,
        reason: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_review_publish_request(actor, &request, "approve")
            .await?;
        if request.status != RegistryPublishRequestStatus::Approved {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot be approved",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let stored = StoredRegistryArtifact {
            artifact_path: request.artifact_path.clone().ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_path")
            })?,
            artifact_url: request.artifact_url.clone().ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_url")
            })?,
            artifact_size: request.artifact_size.ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_size")
            })?,
        };
        let checksum = request.artifact_checksum_sha256.clone().ok_or_else(|| {
            anyhow!("Registry publish request '{request_id}' is missing artifact_checksum_sha256")
        })?;
        let latest_validation_stages = self
            .latest_validation_stages_for_request(&request.id)
            .await?;
        let override_stages = latest_validation_stages
            .iter()
            .filter(|stage| stage.status != RegistryValidationStageStatus::Passed)
            .cloned()
            .collect::<Vec<_>>();
        let effective_publisher = self
            .resolve_effective_publisher(&request, actor, publisher)
            .await?;
        if !override_stages.is_empty() {
            let reason = reason
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "Registry publish request '{}' still has non-passed follow-up validation stages; approval override requires a non-empty reason",
                        request_id
                    )
                })?;
            let reason_code = reason_code
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "Registry publish request '{}' still has non-passed follow-up validation stages; approval override requires a non-empty reason_code",
                        request_id
                    )
                })?;
            if !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
            {
                anyhow::bail!(
                    "Registry publish approval override reason_code '{}' is not supported; expected one of {}",
                    reason_code,
                    REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
                );
            }
            self.record_governance_event(
                &request.slug,
                Some(&request.id),
                None,
                "publish_approval_override",
                actor,
                Some(&effective_publisher),
                serde_json::json!({
                    "version": request.version.clone(),
                    "reason": reason,
                    "reason_code": reason_code.to_ascii_lowercase(),
                    "validation_stages": override_stages
                        .iter()
                        .map(validation_stage_details_value)
                        .collect::<Vec<_>>(),
                }),
            )
            .await?;
        }
        let published_at = Utc::now();
        let release = self
            .upsert_release_from_request(
                request_id,
                actor,
                &effective_publisher,
                checksum,
                stored,
                published_at,
                &request,
            )
            .await?;
        self.bind_registry_slug_owner(&request.slug, &effective_publisher, actor)
            .await?;

        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Published);
        request_active.approved_by = Set(Some(normalize_actor(actor)));
        request_active.approved_at = Set(Some(published_at));
        request_active.published_at = Set(Some(published_at));
        request_active.updated_at = Set(published_at);
        let request = request_active
            .update(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            Some(&release.id),
            "release_published",
            actor,
            Some(&effective_publisher),
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "publisher": effective_publisher.clone(),
                "checksum_sha256": release.checksum_sha256.clone(),
                "release_status": release_status_label(release.status.clone()),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn reject_publish_request(
        &self,
        request_id: &str,
        actor: &str,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_review_publish_request(actor, &request, "reject")
            .await?;
        if matches!(
            request.status,
            RegistryPublishRequestStatus::Published | RegistryPublishRequestStatus::Rejected
        ) {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot be rejected",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let rejected_at = Utc::now();
        let mut errors = deserialize_message_list(&request.validation_errors);
        if !reason.trim().is_empty() {
            errors.push(format!("Governance rejection reason: {}", reason.trim()));
        }
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Rejected);
        request_active.rejected_by = Set(Some(normalize_actor(actor)));
        request_active.rejection_reason = Set(Some(reason.trim().to_string()));
        request_active.validation_errors = Set(serde_json::to_value(dedupe_message_list(errors))?);
        request_active.updated_at = Set(rejected_at);
        let request = request_active
            .update(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "request_rejected",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": request.rejection_reason.clone(),
                "reason_code": reason_code.trim(),
                "errors": deserialize_message_list(&request.validation_errors),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn request_publish_changes(
        &self,
        request_id: &str,
        actor: &str,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_review_publish_request(actor, &request, "request changes for")
            .await?;
        if request.status != RegistryPublishRequestStatus::Approved {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot transition to changes_requested",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        if !REGISTRY_REQUEST_CHANGES_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            anyhow::bail!(
                "Registry request-changes reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_REQUEST_CHANGES_REASON_CODES.join(", ")
            );
        }

        let changed_at = Utc::now();
        let mut errors = deserialize_message_list(&request.validation_errors);
        errors.push(format!("Governance requested changes: {}", reason.trim()));

        RegistryValidationJobEntity::delete_many()
            .filter(registry_validation_job::Column::RequestId.eq(request.id.clone()))
            .exec(&self.db)
            .await?;
        RegistryValidationStageEntity::delete_many()
            .filter(registry_validation_stage::Column::RequestId.eq(request.id.clone()))
            .exec(&self.db)
            .await?;

        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::ChangesRequested);
        request_active.changes_requested_by = Set(Some(normalize_actor(actor)));
        request_active.changes_requested_reason = Set(Some(reason.trim().to_string()));
        request_active.changes_requested_reason_code =
            Set(Some(reason_code.trim().to_ascii_lowercase()));
        request_active.changes_requested_at = Set(Some(changed_at));
        request_active.approved_by = Set(None);
        request_active.approved_at = Set(None);
        request_active.validation_errors = Set(serde_json::to_value(dedupe_message_list(errors))?);
        request_active.updated_at = Set(changed_at);
        let request = request_active.update(&self.db).await?;

        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "changes_requested",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": request.changes_requested_reason.clone(),
                "reason_code": request.changes_requested_reason_code.clone(),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn hold_publish_request(
        &self,
        request_id: &str,
        actor: &str,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_review_publish_request(actor, &request, "put on hold")
            .await?;
        if !matches!(
            request.status,
            RegistryPublishRequestStatus::Submitted
                | RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::ChangesRequested
        ) {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot be put on hold",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        if !REGISTRY_HOLD_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            anyhow::bail!(
                "Registry hold reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_HOLD_REASON_CODES.join(", ")
            );
        }

        let held_at = Utc::now();
        let previous_status = request_status_label(request.status.clone()).to_string();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::OnHold);
        request_active.held_by = Set(Some(normalize_actor(actor)));
        request_active.held_reason = Set(Some(reason.trim().to_string()));
        request_active.held_reason_code = Set(Some(reason_code.trim().to_ascii_lowercase()));
        request_active.held_at = Set(Some(held_at));
        request_active.held_from_status = Set(Some(previous_status.clone()));
        request_active.updated_at = Set(held_at);
        let request = request_active.update(&self.db).await?;

        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "request_held",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": request.held_reason.clone(),
                "reason_code": request.held_reason_code.clone(),
                "held_from_status": previous_status,
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn resume_publish_request(
        &self,
        request_id: &str,
        actor: &str,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        self.ensure_actor_can_review_publish_request(actor, &request, "resume")
            .await?;
        if request.status != RegistryPublishRequestStatus::OnHold {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot be resumed",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        if !REGISTRY_RESUME_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            anyhow::bail!(
                "Registry resume reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_RESUME_REASON_CODES.join(", ")
            );
        }

        let target_status = request
            .held_from_status
            .as_deref()
            .and_then(parse_request_status_label)
            .ok_or_else(|| {
                anyhow!(
                    "Registry publish request '{}' is on hold but held_from_status is missing or invalid",
                    request_id
                )
            })?;
        let resumed_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(target_status.clone());
        request_active.held_by = Set(None);
        request_active.held_reason = Set(None);
        request_active.held_reason_code = Set(None);
        request_active.held_at = Set(None);
        request_active.held_from_status = Set(None);
        request_active.updated_at = Set(resumed_at);
        let request = request_active.update(&self.db).await?;

        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "request_resumed",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": reason.trim(),
                "reason_code": reason_code.trim().to_ascii_lowercase(),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn yank_release(
        &self,
        slug: &str,
        version: &str,
        reason: &str,
        reason_code: &str,
        actor: &str,
    ) -> anyhow::Result<registry_module_release::Model> {
        let release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(slug))
            .filter(registry_module_release::Column::Version.eq(version))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Published release '{slug}@{version}' was not found"))?;
        self.ensure_actor_can_manage_release(actor, &release, "yank")
            .await?;

        let mut active: RegistryModuleReleaseActiveModel = release.into();
        active.status = Set(RegistryModuleReleaseStatus::Yanked);
        active.yanked_reason = Set(Some(reason.trim().to_string()));
        active.yanked_by = Set(Some(normalize_actor(actor)));
        active.yanked_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());
        let release = active.update(&self.db).await?;
        self.record_governance_event(
            &release.slug,
            release.request_id.as_deref(),
            Some(&release.id),
            "release_yanked",
            actor,
            Some(&release.publisher),
            serde_json::json!({
                "version": release.version.clone(),
                "status": release_status_label(release.status.clone()),
                "reason_code": reason_code.trim(),
                "reason": release.yanked_reason.clone(),
            }),
        )
        .await?;
        Ok(release)
    }

    pub async fn transfer_registry_slug_owner(
        &self,
        slug: &str,
        new_owner_actor: &str,
        reason: &str,
        reason_code: &str,
        actor: &str,
    ) -> anyhow::Result<registry_module_owner::Model> {
        let existing = self
            .registry_slug_owner(slug)
            .await?
            .ok_or_else(|| anyhow!("Registry owner binding for slug '{slug}' was not found"))?;
        self.ensure_actor_can_transfer_registry_owner(actor, &existing, "transfer ownership")
            .await?;

        let next_owner = normalize_actor(new_owner_actor);
        if next_owner == "anonymous" {
            anyhow::bail!(
                "Registry owner transfer for slug '{}' requires a non-empty new_owner_actor",
                slug
            );
        }
        if existing.owner_actor == next_owner {
            anyhow::bail!(
                "Registry owner for slug '{}' is already bound to '{}'",
                slug,
                next_owner
            );
        }

        let previous_owner = existing.owner_actor.clone();
        let normalized_actor = normalize_actor(actor);
        let now = Utc::now();
        let mut active: RegistryModuleOwnerActiveModel = existing.into();
        active.owner_actor = Set(next_owner.clone());
        active.bound_by = Set(normalized_actor.clone());
        active.bound_at = Set(now);
        active.updated_at = Set(now);
        let binding = active.update(&self.db).await?;
        self.record_governance_event(
            slug,
            None,
            None,
            "owner_transferred",
            &normalized_actor,
            Some(&binding.owner_actor),
            serde_json::json!({
                "previous_owner_actor": previous_owner,
                "new_owner_actor": binding.owner_actor.clone(),
                "bound_by": binding.bound_by.clone(),
                "reason": reason.trim(),
                "reason_code": reason_code.trim(),
            }),
        )
        .await?;
        Ok(binding)
    }

    pub async fn apply_catalog_projection(
        &self,
        modules: Vec<CatalogManifestModule>,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let releases = RegistryModuleReleaseEntity::find()
            .order_by_desc(registry_module_release::Column::PublishedAt)
            .all(&self.db)
            .await?;

        if releases.is_empty() {
            return Ok(modules);
        }

        let mut release_map: HashMap<String, Vec<registry_module_release::Model>> = HashMap::new();
        for release in releases {
            release_map
                .entry(release.slug.clone())
                .or_default()
                .push(release);
        }

        let mut projected = modules;
        for module in &mut projected {
            let Some(releases) = release_map.get(&module.slug) else {
                continue;
            };

            let mut versions = releases
                .iter()
                .map(|release| CatalogModuleVersion {
                    version: release.version.clone(),
                    changelog: None,
                    yanked: release.status == RegistryModuleReleaseStatus::Yanked,
                    published_at: Some(release.published_at.to_rfc3339()),
                    checksum_sha256: release.checksum_sha256.clone(),
                    signature: None,
                })
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| {
                left.yanked
                    .cmp(&right.yanked)
                    .then_with(|| right.published_at.cmp(&left.published_at))
                    .then_with(|| compare_semver_desc(&left.version, &right.version))
                    .then_with(|| right.version.cmp(&left.version))
            });

            if let Some(latest_active) = releases
                .iter()
                .find(|release| release.status == RegistryModuleReleaseStatus::Active)
            {
                module.version = Some(latest_active.version.clone());
                module.publisher = Some(latest_active.publisher.clone());
                module.checksum_sha256 = latest_active.checksum_sha256.clone();
            }
            module.versions = versions;
        }

        Ok(projected)
    }

    pub async fn lifecycle_snapshot(
        &self,
        slug: &str,
        ownership: &str,
    ) -> anyhow::Result<Option<RegistryModuleLifecycleSnapshot>> {
        let owner_binding = RegistryModuleOwnerEntity::find_by_id(slug)
            .one(&self.db)
            .await?;
        let latest_request = RegistryPublishRequestEntity::find()
            .filter(registry_publish_request::Column::Slug.eq(slug))
            .order_by_desc(registry_publish_request::Column::CreatedAt)
            .one(&self.db)
            .await?;
        let latest_release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(slug))
            .order_by_desc(registry_module_release::Column::PublishedAt)
            .one(&self.db)
            .await?;
        let recent_events = RegistryGovernanceEventEntity::find()
            .filter(registry_governance_event::Column::Slug.eq(slug))
            .order_by_desc(registry_governance_event::Column::CreatedAt)
            .limit(10)
            .all(&self.db)
            .await?;
        let validation_stage_rows = if let Some(request) = latest_request.as_ref() {
            self.validation_stage_rows(&request.id).await?
        } else {
            Vec::new()
        };

        let moderation_policy = moderation_policy_snapshot(ownership);
        let validation_stages = derive_validation_stage_snapshots(
            latest_request.as_ref(),
            &recent_events,
            &validation_stage_rows,
        );
        let follow_up_gates = derive_follow_up_gate_snapshots(
            latest_request.as_ref(),
            &recent_events,
            &validation_stages,
        );
        let governance_actions = derive_registry_governance_actions(
            &moderation_policy,
            latest_request.as_ref(),
            latest_release.as_ref(),
            owner_binding.as_ref(),
            &recent_events,
            &validation_stages,
        );

        Ok(Some(RegistryModuleLifecycleSnapshot {
            moderation_policy,
            owner_binding: owner_binding.map(|binding| RegistryModuleOwnerSnapshot {
                owner_actor: binding.owner_actor,
                bound_by: binding.bound_by,
                bound_at: binding.bound_at.to_rfc3339(),
                updated_at: binding.updated_at.to_rfc3339(),
            }),
            latest_request: latest_request.map(|request| RegistryPublishRequestSnapshot {
                id: request.id,
                status: request_status_label(request.status).to_string(),
                requested_by: request.requested_by,
                publisher_identity: request.publisher_identity,
                approved_by: request.approved_by,
                rejected_by: request.rejected_by,
                rejection_reason: request.rejection_reason,
                changes_requested_by: request.changes_requested_by,
                changes_requested_reason: request.changes_requested_reason,
                changes_requested_reason_code: request.changes_requested_reason_code,
                changes_requested_at: request.changes_requested_at.map(|value| value.to_rfc3339()),
                held_by: request.held_by,
                held_reason: request.held_reason,
                held_reason_code: request.held_reason_code,
                held_at: request.held_at.map(|value| value.to_rfc3339()),
                held_from_status: request.held_from_status,
                warnings: deserialize_message_list(&request.validation_warnings),
                errors: deserialize_message_list(&request.validation_errors),
                created_at: request.created_at.to_rfc3339(),
                updated_at: request.updated_at.to_rfc3339(),
                published_at: request.published_at.map(|value| value.to_rfc3339()),
            }),
            latest_release: latest_release.map(|release| RegistryModuleReleaseSnapshot {
                version: release.version,
                status: release_status_label(release.status).to_string(),
                publisher: release.publisher,
                checksum_sha256: release.checksum_sha256,
                published_at: release.published_at.to_rfc3339(),
                yanked_reason: release.yanked_reason,
                yanked_by: release.yanked_by,
                yanked_at: release.yanked_at.map(|value| value.to_rfc3339()),
            }),
            recent_events: recent_events
                .into_iter()
                .map(|event| RegistryGovernanceEventSnapshot {
                    id: event.id,
                    event_type: event.event_type,
                    actor: event.actor,
                    publisher: event.publisher,
                    details: event.details,
                    created_at: event.created_at.to_rfc3339(),
                })
                .collect(),
            follow_up_gates,
            validation_stages,
            governance_actions,
        }))
    }

    pub async fn publish_request_follow_up_snapshot(
        &self,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<RegistryPublishRequestFollowUpSnapshot> {
        let validation_stage_rows = self.validation_stage_rows(&request.id).await?;
        let validation_stages =
            derive_validation_stage_snapshots(Some(request), &[], &validation_stage_rows);
        let follow_up_gates =
            derive_follow_up_gate_snapshots(Some(request), &[], &validation_stages);
        let approval_override_required = request.status == RegistryPublishRequestStatus::Approved
            && validation_stages
                .iter()
                .any(|stage| !stage.status.eq_ignore_ascii_case("passed"));

        Ok(RegistryPublishRequestFollowUpSnapshot {
            follow_up_gates,
            validation_stages,
            approval_override_required,
        })
    }

    async fn upsert_release_from_request(
        &self,
        request_id: &str,
        _actor: &str,
        publisher: &str,
        checksum: String,
        stored: StoredRegistryArtifact,
        published_at: chrono::DateTime<Utc>,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_module_release::Model> {
        let existing = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(&request.slug))
            .filter(registry_module_release::Column::Version.eq(&request.version))
            .one(&self.db)
            .await?;

        let marketplace = request.marketplace.clone();
        let ui_packages = request.ui_packages.clone();
        let publisher = normalize_actor(publisher);

        if let Some(existing) = existing {
            let mut active: RegistryModuleReleaseActiveModel = existing.into();
            active.request_id = Set(Some(request_id.to_string()));
            active.crate_name = Set(request.crate_name.clone());
            active.module_name = Set(request.module_name.clone());
            active.description = Set(request.description.clone());
            active.ownership = Set(request.ownership.clone());
            active.trust_level = Set(request.trust_level.clone());
            active.license = Set(request.license.clone());
            active.entry_type = Set(request.entry_type.clone());
            active.marketplace = Set(marketplace);
            active.ui_packages = Set(ui_packages);
            active.status = Set(RegistryModuleReleaseStatus::Active);
            active.publisher = Set(publisher);
            active.artifact_path = Set(Some(stored.artifact_path));
            active.artifact_url = Set(Some(stored.artifact_url));
            active.checksum_sha256 = Set(Some(checksum));
            active.artifact_size = Set(Some(stored.artifact_size));
            active.yanked_reason = Set(None);
            active.yanked_by = Set(None);
            active.yanked_at = Set(None);
            active.published_at = Set(published_at);
            active.updated_at = Set(Utc::now());
            return Ok(active.update(&self.db).await?);
        }

        let id = format!("rrel_{}", uuid::Uuid::new_v4().simple());
        let active = RegistryModuleReleaseActiveModel {
            id: Set(id),
            request_id: Set(Some(request_id.to_string())),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            crate_name: Set(request.crate_name.clone()),
            module_name: Set(request.module_name.clone()),
            description: Set(request.description.clone()),
            ownership: Set(request.ownership.clone()),
            trust_level: Set(request.trust_level.clone()),
            license: Set(request.license.clone()),
            entry_type: Set(request.entry_type.clone()),
            marketplace: Set(marketplace),
            ui_packages: Set(ui_packages),
            status: Set(RegistryModuleReleaseStatus::Active),
            publisher: Set(publisher),
            artifact_path: Set(Some(stored.artifact_path)),
            artifact_url: Set(Some(stored.artifact_url)),
            checksum_sha256: Set(Some(checksum)),
            artifact_size: Set(Some(stored.artifact_size)),
            yanked_reason: Set(None),
            yanked_by: Set(None),
            yanked_at: Set(None),
            published_at: Set(published_at),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };
        Ok(active.insert(&self.db).await?)
    }

    async fn ensure_actor_can_create_publish_request(
        &self,
        actor: &str,
        publisher: Option<&str>,
        slug: &str,
    ) -> anyhow::Result<()> {
        let actor = normalize_actor(actor);
        if actor_is_registry_governance(&actor) {
            return Ok(());
        }

        let owner = self.registry_slug_owner(slug).await?;
        let requested_publisher = normalize_optional_actor(publisher);
        if owner.as_ref().is_some_and(|owner| {
            actor == owner.owner_actor
                || requested_publisher.as_deref() == Some(owner.owner_actor.as_str())
        }) {
            return Ok(());
        }

        if owner.is_none()
            && (legacy_actor_can_manage_registry_slug(&actor, slug)
                || requested_publisher.as_deref().is_some_and(|publisher| {
                    legacy_actor_can_manage_registry_slug(publisher, slug)
                }))
        {
            return Ok(());
        }

        anyhow::bail!(
            "Actor '{}' is not allowed to create registry publish requests for slug '{}'",
            actor,
            slug
        )
    }

    async fn ensure_actor_can_manage_publish_request(
        &self,
        actor: &str,
        request: &registry_publish_request::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let actor = normalize_actor(actor);
        let owner = self.registry_slug_owner(&request.slug).await?;
        let actor_matches_request = actor == request.requested_by
            || request
                .publisher_identity
                .as_ref()
                .is_some_and(|publisher| actor == publisher.as_str());
        let actor_matches_owner = owner
            .as_ref()
            .is_some_and(|owner| actor == owner.owner_actor);

        if actor_is_registry_governance(&actor) || actor_matches_owner {
            return Ok(());
        }

        if owner.is_none()
            && (actor_matches_request
                || legacy_actor_can_manage_registry_slug(&actor, &request.slug))
        {
            return Ok(());
        }

        anyhow::bail!(
            "Actor '{}' is not allowed to {} registry publish request '{}' for slug '{}'; management actions require either a governance actor, the current persisted owner binding, or (before owner binding exists) the original requester/publisher identity",
            actor,
            action,
            request.id,
            request.slug
        )
    }

    async fn ensure_actor_can_review_publish_request(
        &self,
        actor: &str,
        request: &registry_publish_request::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let actor = normalize_actor(actor);
        let owner = self.registry_slug_owner(&request.slug).await?;
        if actor_is_registry_review_governance(&actor)
            || owner
                .as_ref()
                .is_some_and(|owner| actor == owner.owner_actor)
        {
            return Ok(());
        }

        anyhow::bail!(
            "Actor '{}' is not allowed to {} registry publish request '{}' for slug '{}'; review actions require either a registry review actor (admin/moderator) or the current persisted owner binding",
            actor,
            action,
            request.id,
            request.slug
        )
    }

    async fn ensure_actor_can_manage_release(
        &self,
        actor: &str,
        release: &registry_module_release::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let actor = normalize_actor(actor);
        let owner = self.registry_slug_owner(&release.slug).await?;
        if actor_is_registry_admin(&actor)
            || actor == release.publisher
            || owner
                .as_ref()
                .is_some_and(|owner| actor == owner.owner_actor)
            || (owner.is_none() && legacy_actor_can_manage_registry_slug(&actor, &release.slug))
        {
            return Ok(());
        }

        anyhow::bail!(
            "Actor '{}' is not allowed to {} published release '{}@{}'; yank/unpublish actions require the registry admin, current persisted owner binding, or the published release actor",
            actor,
            action,
            release.slug,
            release.version
        )
    }

    async fn ensure_actor_can_transfer_registry_owner(
        &self,
        actor: &str,
        binding: &registry_module_owner::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let actor = normalize_actor(actor);
        if actor_is_registry_admin(&actor) || actor == binding.owner_actor {
            return Ok(());
        }

        anyhow::bail!(
            "Actor '{}' is not allowed to {} for slug '{}'; owner transfer requires the registry admin or the current persisted owner binding",
            actor,
            action,
            binding.slug
        )
    }

    async fn resolve_effective_publisher(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        publisher: Option<&str>,
    ) -> anyhow::Result<String> {
        if let Some(owner) = self.registry_slug_owner(&request.slug).await? {
            return Ok(owner.owner_actor);
        }

        if let Some(publisher) = request.publisher_identity.clone() {
            return Ok(publisher);
        }

        if let Some(publisher) = normalize_optional_actor(publisher) {
            return Ok(publisher);
        }

        let actor = normalize_actor(actor);
        if !actor_is_registry_governance(&actor) && actor != "anonymous" {
            return Ok(actor);
        }

        Ok(format!("publisher:{}", request.slug))
    }

    async fn bind_registry_slug_owner(
        &self,
        slug: &str,
        owner_actor: &str,
        bound_by: &str,
    ) -> anyhow::Result<registry_module_owner::Model> {
        let owner_actor = normalize_actor(owner_actor);
        let bound_by = normalize_actor(bound_by);
        let now = Utc::now();

        if let Some(existing) = RegistryModuleOwnerEntity::find_by_id(slug)
            .one(&self.db)
            .await?
        {
            if existing.owner_actor == owner_actor {
                let mut active: RegistryModuleOwnerActiveModel = existing.into();
                active.bound_by = Set(bound_by);
                active.updated_at = Set(now);
                return Ok(active.update(&self.db).await?);
            }

            if !actor_is_registry_governance(&bound_by) {
                anyhow::bail!(
                    "Actor '{}' is not allowed to rebind registry owner for slug '{}'",
                    bound_by,
                    slug
                );
            }

            let mut active: RegistryModuleOwnerActiveModel = existing.into();
            active.owner_actor = Set(owner_actor);
            active.bound_by = Set(bound_by);
            active.bound_at = Set(now);
            active.updated_at = Set(now);
            let binding = active.update(&self.db).await?;
            self.record_governance_event(
                slug,
                None,
                None,
                "owner_bound",
                &binding.bound_by,
                Some(&binding.owner_actor),
                serde_json::json!({
                    "owner_actor": binding.owner_actor.clone(),
                    "bound_by": binding.bound_by.clone(),
                    "mode": "rebind",
                }),
            )
            .await?;
            return Ok(binding);
        }

        let active = RegistryModuleOwnerActiveModel {
            slug: Set(slug.to_string()),
            owner_actor: Set(owner_actor),
            bound_by: Set(bound_by),
            bound_at: Set(now),
            updated_at: Set(now),
        };
        let binding = active.insert(&self.db).await?;
        self.record_governance_event(
            slug,
            None,
            None,
            "owner_bound",
            &binding.bound_by,
            Some(&binding.owner_actor),
            serde_json::json!({
                "owner_actor": binding.owner_actor.clone(),
                "bound_by": binding.bound_by.clone(),
                "mode": "initial",
            }),
        )
        .await?;
        Ok(binding)
    }

    async fn registry_slug_owner(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<registry_module_owner::Model>> {
        Ok(RegistryModuleOwnerEntity::find_by_id(slug)
            .one(&self.db)
            .await?)
    }

    async fn latest_validation_job(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_validation_job::Model>> {
        Ok(RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request_id))
            .order_by_desc(registry_validation_job::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn latest_active_validation_job(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_validation_job::Model>> {
        Ok(RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request_id))
            .filter(registry_validation_job::Column::Status.is_in([
                RegistryValidationJobStatus::Queued,
                RegistryValidationJobStatus::Running,
            ]))
            .order_by_desc(registry_validation_job::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn create_validation_job(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        queue_reason: &str,
    ) -> anyhow::Result<registry_validation_job::Model> {
        let now = Utc::now();
        let next_attempt_number = self
            .latest_validation_job(&request.id)
            .await?
            .map(|job| job.attempt_number + 1)
            .unwrap_or(1);
        let active = RegistryValidationJobActiveModel {
            id: Set(format!("rvj_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            status: Set(RegistryValidationJobStatus::Queued),
            triggered_by: Set(normalize_actor(actor)),
            queue_reason: Set(queue_reason.to_string()),
            attempt_number: Set(next_attempt_number),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(active.insert(&self.db).await?)
    }

    async fn claim_validation_job(
        &self,
        validation_job_id: &str,
        actor: &str,
    ) -> anyhow::Result<Option<RegistryValidationJobClaim>> {
        let Some(job) = RegistryValidationJobEntity::find_by_id(validation_job_id)
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let request = self
            .get_publish_request(&job.request_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Registry validation job '{}' points to missing publish request '{}'",
                    validation_job_id,
                    job.request_id
                )
            })?;

        match job.status {
            RegistryValidationJobStatus::Queued => {
                let started_at = Utc::now();
                let mut active: RegistryValidationJobActiveModel = job.clone().into();
                active.status = Set(RegistryValidationJobStatus::Running);
                active.started_at = Set(Some(started_at));
                active.finished_at = Set(None);
                active.last_error = Set(None);
                active.updated_at = Set(started_at);
                let job = active.update(&self.db).await?;
                self.record_governance_event(
                    &request.slug,
                    Some(&request.id),
                    None,
                    "validation_job_started",
                    actor,
                    None,
                    serde_json::json!({
                        "job_id": job.id.clone(),
                        "attempt_number": job.attempt_number,
                        "queue_reason": job.queue_reason.clone(),
                        "request_status": request_status_label(request.status.clone()),
                        "version": request.version.clone(),
                    }),
                )
                .await?;
                Ok(Some(RegistryValidationJobClaim {
                    job,
                    request,
                    should_run: true,
                }))
            }
            RegistryValidationJobStatus::Running
            | RegistryValidationJobStatus::Succeeded
            | RegistryValidationJobStatus::Failed => Ok(Some(RegistryValidationJobClaim {
                job,
                request,
                should_run: false,
            })),
        }
    }

    async fn mark_validation_job_succeeded(
        &self,
        job: registry_validation_job::Model,
        actor: &str,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_validation_job::Model> {
        let finished_at = Utc::now();
        let mut active: RegistryValidationJobActiveModel = job.clone().into();
        active.status = Set(RegistryValidationJobStatus::Succeeded);
        active.finished_at = Set(Some(finished_at));
        active.last_error = Set(None);
        active.updated_at = Set(finished_at);
        let job = active.update(&self.db).await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_job_succeeded",
            actor,
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
            }),
        )
        .await?;
        Ok(job)
    }

    async fn mark_validation_job_failed(
        &self,
        job: registry_validation_job::Model,
        actor: &str,
        last_error: Option<&str>,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let finished_at = Utc::now();
        let mut active: RegistryValidationJobActiveModel = job.clone().into();
        active.status = Set(RegistryValidationJobStatus::Failed);
        active.finished_at = Set(Some(finished_at));
        active.last_error = Set(last_error.map(ToString::to_string));
        active.updated_at = Set(finished_at);
        let job = active.update(&self.db).await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_job_failed",
            actor,
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
                "error": last_error,
            }),
        )
        .await?;
        Ok(request.clone())
    }

    async fn validation_stage_rows(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .all(&self.db)
            .await?)
    }

    async fn latest_validation_stages_for_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        let mut latest = HashMap::<String, registry_validation_stage::Model>::new();
        for stage in self.validation_stage_rows(request_id).await? {
            latest.entry(stage.stage_key.clone()).or_insert(stage);
        }
        let mut stages = latest.into_values().collect::<Vec<_>>();
        stages.sort_by(|left, right| left.stage_key.cmp(&right.stage_key));
        Ok(stages)
    }

    async fn latest_validation_stage(
        &self,
        request_id: &str,
        stage_key: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .filter(registry_validation_stage::Column::StageKey.eq(stage_key))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn latest_active_validation_stage(
        &self,
        request_id: &str,
        stage_key: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .filter(registry_validation_stage::Column::StageKey.eq(stage_key))
            .filter(registry_validation_stage::Column::Status.is_in([
                RegistryValidationStageStatus::Queued,
                RegistryValidationStageStatus::Running,
            ]))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn validation_stage_by_claim_id(
        &self,
        claim_id: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
            .one(&self.db)
            .await?)
    }

    async fn queue_follow_up_validation_stages(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        queue_reason: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        let mut stages = Vec::new();

        for stage_key in REGISTRY_VALIDATION_FOLLOW_UP_GATES {
            if self
                .latest_active_validation_stage(&request.id, stage_key)
                .await?
                .is_some()
            {
                continue;
            }

            stages.push(
                self.queue_validation_stage_attempt(
                    request,
                    stage_key,
                    actor,
                    queue_reason,
                    follow_up_gate_detail(stage_key),
                )
                .await?,
            );
        }

        Ok(stages)
    }

    async fn queue_validation_stage_attempt(
        &self,
        request: &registry_publish_request::Model,
        stage_key: &str,
        actor: &str,
        queue_reason: &str,
        detail: &str,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        let now = Utc::now();
        let next_attempt_number = self
            .latest_validation_stage(&request.id, stage_key)
            .await?
            .map(|stage| stage.attempt_number + 1)
            .unwrap_or(1);
        let active = RegistryValidationStageActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set(stage_key.to_string()),
            status: Set(RegistryValidationStageStatus::Queued),
            triggered_by: Set(normalize_actor(actor)),
            queue_reason: Set(queue_reason.to_string()),
            attempt_number: Set(next_attempt_number),
            detail: Set(detail.to_string()),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            claim_id: Set(None),
            claimed_by: Set(None),
            claim_expires_at: Set(None),
            last_heartbeat_at: Set(None),
            runner_kind: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        let stage = active.insert(&self.db).await?;
        self.record_validation_stage_event(
            request,
            actor,
            &stage,
            "validation_stage_queued",
            detail,
            None,
            None,
        )
        .await?;
        self.record_follow_up_gate_event(
            request,
            actor,
            stage_key,
            "follow_up_gate_queued",
            "pending",
            detail,
            None,
        )
        .await?;
        Ok(stage)
    }

    async fn update_validation_stage_status(
        &self,
        stage: registry_validation_stage::Model,
        request: &registry_publish_request::Model,
        actor: &str,
        status: RegistryValidationStageStatus,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        ensure_validation_stage_transition_allowed(&stage.status, &status, &stage.stage_key)?;

        let now = Utc::now();
        let existing_started_at = stage.started_at;
        let mut active: RegistryValidationStageActiveModel = stage.clone().into();
        active.status = Set(status.clone());
        active.detail = Set(detail.to_string());
        active.updated_at = Set(now);
        active.last_error = Set(match &status {
            RegistryValidationStageStatus::Failed => Some(detail.to_string()),
            _ => None,
        });
        match &status {
            RegistryValidationStageStatus::Queued => {
                active.started_at = Set(None);
                active.finished_at = Set(None);
            }
            RegistryValidationStageStatus::Running => {
                active.started_at = Set(existing_started_at.or(Some(now)));
                active.finished_at = Set(None);
            }
            RegistryValidationStageStatus::Passed
            | RegistryValidationStageStatus::Failed
            | RegistryValidationStageStatus::Blocked => {
                active.started_at = Set(existing_started_at.or(Some(now)));
                active.finished_at = Set(Some(now));
            }
        }
        let stage = active.update(&self.db).await?;
        let event_type = validation_stage_event_type(&status);
        self.record_validation_stage_event(
            request,
            actor,
            &stage,
            event_type,
            detail,
            reason_code,
            None,
        )
        .await?;
        match &status {
            RegistryValidationStageStatus::Passed => {
                self.record_follow_up_gate_event(
                    request,
                    actor,
                    &stage.stage_key,
                    "follow_up_gate_passed",
                    "passed",
                    detail,
                    reason_code,
                )
                .await?;
            }
            RegistryValidationStageStatus::Failed => {
                self.record_follow_up_gate_event(
                    request,
                    actor,
                    &stage.stage_key,
                    "follow_up_gate_failed",
                    "failed",
                    detail,
                    reason_code,
                )
                .await?;
            }
            _ => {}
        }
        Ok(stage)
    }

    async fn record_validation_stage_event(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        stage: &registry_validation_stage::Model,
        event_type: &str,
        detail: &str,
        reason_code: Option<&str>,
        extra: Option<serde_json::Value>,
    ) -> anyhow::Result<registry_governance_event::Model> {
        let mut details = serde_json::json!({
            "stage_id": stage.id.clone(),
            "stage": stage.stage_key.clone(),
            "status": validation_stage_status_label(stage.status.clone()),
            "detail": detail,
            "attempt_number": stage.attempt_number,
            "queue_reason": stage.queue_reason.clone(),
            "request_status": request_status_label(request.status.clone()),
            "version": request.version.clone(),
            "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
            "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
        });
        if let Some(reason_code) = reason_code {
            details["reason_code"] = serde_json::Value::String(reason_code.to_string());
        }
        if let Some(extra) = extra {
            merge_json_object(&mut details, extra);
        }
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            event_type,
            actor,
            None,
            details,
        )
        .await
    }

    async fn record_follow_up_gate_event(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        stage_key: &str,
        event_type: &str,
        status: &str,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_governance_event::Model> {
        let mut details = serde_json::json!({
            "gate": stage_key,
            "status": status,
            "detail": detail,
        });
        if let Some(reason_code) = reason_code {
            details["reason_code"] = serde_json::Value::String(reason_code.to_string());
        }
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            event_type,
            actor,
            None,
            details,
        )
        .await
    }

    async fn latest_request_event_type(&self, request_id: &str) -> anyhow::Result<Option<String>> {
        Ok(RegistryGovernanceEventEntity::find()
            .filter(registry_governance_event::Column::RequestId.eq(request_id))
            .order_by_desc(registry_governance_event::Column::CreatedAt)
            .one(&self.db)
            .await?
            .map(|event| event.event_type))
    }

    async fn finish_remote_validation_stage_claim(
        &self,
        claim_id: &str,
        runner_id: &str,
        status: RegistryValidationStageStatus,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        let stage = self
            .validation_stage_by_claim_id(claim_id)
            .await?
            .ok_or_else(|| anyhow!("Registry validation claim '{}' was not found", claim_id))?;
        self.ensure_remote_claim_belongs_to_runner(&stage, claim_id, runner_id)?;
        let request = self
            .get_publish_request(&stage.request_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Registry publish request '{}' for claim '{}' was not found",
                    stage.request_id,
                    claim_id
                )
            })?;
        let runner_actor = remote_runner_actor(runner_id);
        let stage = self
            .update_validation_stage_status(
                stage,
                &request,
                &runner_actor,
                status,
                detail,
                reason_code,
            )
            .await?;
        let mut active: RegistryValidationStageActiveModel = stage.into();
        active.claim_id = Set(None);
        active.claimed_by = Set(None);
        active.claim_expires_at = Set(None);
        active.last_heartbeat_at = Set(None);
        active.runner_kind = Set(None);
        active.updated_at = Set(Utc::now());
        Ok(active.update(&self.db).await?)
    }

    fn ensure_remote_claim_belongs_to_runner(
        &self,
        stage: &registry_validation_stage::Model,
        claim_id: &str,
        runner_id: &str,
    ) -> anyhow::Result<()> {
        if stage.status != RegistryValidationStageStatus::Running {
            anyhow::bail!(
                "Registry validation claim '{}' is in status '{}' and can no longer be updated",
                claim_id,
                validation_stage_status_label(stage.status.clone())
            );
        }
        let claimed_by = stage.claimed_by.as_deref().ok_or_else(|| {
            anyhow!(
                "Registry validation claim '{}' is missing claimed_by metadata",
                claim_id
            )
        })?;
        let expected_runner = remote_runner_actor(runner_id);
        if claimed_by != expected_runner {
            anyhow::bail!(
                "Registry validation claim '{}' belongs to '{}' rather than '{}'",
                claim_id,
                claimed_by,
                expected_runner
            );
        }
        Ok(())
    }

    async fn store_validation_rejection(
        &self,
        request: registry_publish_request::Model,
        actor: &str,
        warnings: &[String],
        errors: &[String],
        failed_checks: Vec<RegistryValidationCheckDetail>,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let rejected_at = Utc::now();
        let errors = dedupe_message_list(errors.to_vec());
        let warnings = dedupe_message_list(warnings.to_vec());
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Rejected);
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::to_value(&errors)?);
        request_active.rejected_by = Set(Some(normalize_actor(actor)));
        request_active.rejection_reason = Set(errors.first().cloned());
        request_active.validated_at = Set(Some(rejected_at));
        request_active.approved_by = Set(None);
        request_active.approved_at = Set(None);
        request_active.published_at = Set(None);
        request_active.updated_at = Set(rejected_at);
        let request = request_active.update(&self.db).await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_failed",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": request.rejection_reason.clone(),
                "warnings": warnings,
                "errors": errors,
                "automated_checks": failed_checks,
            }),
        )
        .await?;
        Ok(request)
    }

    async fn record_governance_event(
        &self,
        slug: &str,
        request_id: Option<&str>,
        release_id: Option<&str>,
        event_type: &str,
        actor: &str,
        publisher: Option<&str>,
        details: serde_json::Value,
    ) -> anyhow::Result<registry_governance_event::Model> {
        let active = RegistryGovernanceEventActiveModel {
            id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
            slug: Set(slug.to_string()),
            request_id: Set(request_id.map(ToString::to_string)),
            release_id: Set(release_id.map(ToString::to_string)),
            event_type: Set(event_type.to_string()),
            actor: Set(normalize_actor(actor)),
            publisher: Set(normalize_optional_actor(publisher)),
            details: Set(details),
            created_at: Set(Utc::now()),
        };
        Ok(active.insert(&self.db).await?)
    }
}

pub(crate) fn moderation_policy_snapshot(ownership: &str) -> RegistryModerationPolicySnapshot {
    if ownership.eq_ignore_ascii_case("first_party") {
        RegistryModerationPolicySnapshot {
            mode: "first_party_live".to_string(),
            live_publish_supported: true,
            live_governance_supported: true,
            manual_review_required: false,
            restriction_reason_code: None,
            restriction_reason:
                "Live registry publish/governance flow is enabled for first-party modules."
                    .to_string(),
        }
    } else {
        RegistryModerationPolicySnapshot {
            mode: "third_party_manual_only".to_string(),
            live_publish_supported: false,
            live_governance_supported: false,
            manual_review_required: true,
            restriction_reason_code: Some(
                REGISTRY_MODERATION_POLICY_REASON_CODE_THIRD_PARTY_FLOW_PENDING.to_string(),
            ),
            restriction_reason: "Third-party moderation/governance flow is not implemented yet; keep this module on manual review and dry-run registry preview paths until the broader moderation flow is finished."
                .to_string(),
        }
    }
}

fn derive_registry_governance_actions(
    moderation_policy: &RegistryModerationPolicySnapshot,
    latest_request: Option<&registry_publish_request::Model>,
    latest_release: Option<&registry_module_release::Model>,
    owner_binding: Option<&registry_module_owner::Model>,
    recent_events: &[registry_governance_event::Model],
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<RegistryGovernanceActionSnapshot> {
    let restriction_reason = || Some(moderation_policy.restriction_reason.clone());
    let action =
        |key: &str, enabled: bool, reason: Option<String>, supported_reason_codes: &[&str]| {
            RegistryGovernanceActionSnapshot {
                key: key.to_string(),
                enabled,
                reason,
                supported_reason_codes: supported_reason_codes
                    .iter()
                    .map(|code| (*code).to_string())
                    .collect(),
            }
        };

    if !moderation_policy.live_governance_supported {
        return vec![
            action("validate", false, restriction_reason(), &[]),
            action(
                "approve",
                false,
                restriction_reason(),
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
            ),
            action(
                "reject",
                false,
                restriction_reason(),
                REGISTRY_REJECT_REASON_CODES,
            ),
            action(
                "request_changes",
                false,
                restriction_reason(),
                REGISTRY_REQUEST_CHANGES_REASON_CODES,
            ),
            action(
                "hold",
                false,
                restriction_reason(),
                REGISTRY_HOLD_REASON_CODES,
            ),
            action(
                "resume",
                false,
                restriction_reason(),
                REGISTRY_RESUME_REASON_CODES,
            ),
            action("stage_report", false, restriction_reason(), &[]),
            action(
                "owner_transfer",
                false,
                restriction_reason(),
                REGISTRY_OWNER_TRANSFER_REASON_CODES,
            ),
            action(
                "yank",
                false,
                restriction_reason(),
                REGISTRY_YANK_REASON_CODES,
            ),
        ];
    }

    let validate_action = match latest_request {
        Some(request)
            if matches!(
                request.status,
                RegistryPublishRequestStatus::ArtifactUploaded
                    | RegistryPublishRequestStatus::Submitted
            ) =>
        {
            action("validate", true, None, &[])
        }
        Some(request) if request.status == RegistryPublishRequestStatus::Rejected => {
            let latest_event_type = recent_events
                .iter()
                .find(|event| event.request_id.as_deref() == Some(request.id.as_str()))
                .map(|event| event.event_type.as_str());
            if rejected_publish_request_can_retry(
                latest_event_type,
                request.rejection_reason.as_deref(),
            ) {
                action("validate", true, None, &[])
            } else {
                action(
                    "validate",
                    false,
                    Some(
                        "Manual governance rejects must be recreated as new publish requests."
                            .to_string(),
                    ),
                    &[],
                )
            }
        }
        Some(request) if request.status == RegistryPublishRequestStatus::Draft => action(
            "validate",
            false,
            Some("Artifact upload must complete before validation can start.".to_string()),
            &[],
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::ChangesRequested => {
            action(
                "validate",
                false,
                Some("Upload a fresh artifact before validation can resume.".to_string()),
                &[],
            )
        }
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => action(
            "validate",
            false,
            Some("Resume the request before validation can continue.".to_string()),
            &[],
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::Validating => action(
            "validate",
            false,
            Some("Validation is already running for this publish request.".to_string()),
            &[],
        ),
        Some(request)
            if matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            ) =>
        {
            action(
                "validate",
                false,
                Some(
                    "Artifact/manifest validation is already complete for this publish request."
                        .to_string(),
                ),
                &[],
            )
        }
        Some(_) => action("validate", false, None, &[]),
        None => action(
            "validate",
            false,
            Some("No publish request exists yet.".to_string()),
            &[],
        ),
    };

    let approve_action = match latest_request {
        Some(request) if request.status == RegistryPublishRequestStatus::Approved => {
            let reason = validation_stages
                .iter()
                .any(|stage| !stage.status.eq_ignore_ascii_case("passed"))
                .then_some(
                    "Approval is available, but non-passed follow-up stages still require an explicit override reason and reason_code."
                        .to_string(),
                );
            action(
                "approve",
                true,
                reason,
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
            )
        }
        Some(request) if request.status == RegistryPublishRequestStatus::Validating => action(
            "approve",
            false,
            Some("Wait for validation to finish before approving this request.".to_string()),
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        ),
        Some(request)
            if matches!(
                request.status,
                RegistryPublishRequestStatus::Draft
                    | RegistryPublishRequestStatus::ArtifactUploaded
                    | RegistryPublishRequestStatus::Submitted
                    | RegistryPublishRequestStatus::ChangesRequested
            ) =>
        {
            action(
                "approve",
                false,
                Some(
                    "Approval is available only after artifact validation reaches review-ready."
                        .to_string(),
                ),
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
            )
        }
        Some(request) if request.status == RegistryPublishRequestStatus::Rejected => action(
            "approve",
            false,
            Some("Rejected publish requests cannot be approved in place.".to_string()),
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => action(
            "approve",
            false,
            Some("Resume the request before approving it.".to_string()),
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::Published => action(
            "approve",
            false,
            Some("This publish request is already published.".to_string()),
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        ),
        Some(_) => action(
            "approve",
            false,
            None,
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        ),
        None => action(
            "approve",
            false,
            Some("No publish request exists yet.".to_string()),
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
        ),
    };

    let reject_action = match latest_request {
        Some(request)
            if !matches!(
                request.status,
                RegistryPublishRequestStatus::Rejected
                    | RegistryPublishRequestStatus::Published
                    | RegistryPublishRequestStatus::OnHold
            ) =>
        {
            action("reject", true, None, REGISTRY_REJECT_REASON_CODES)
        }
        Some(request) if request.status == RegistryPublishRequestStatus::Rejected => action(
            "reject",
            false,
            Some("This publish request is already rejected.".to_string()),
            REGISTRY_REJECT_REASON_CODES,
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::Published => action(
            "reject",
            false,
            Some("Published releases must be managed through yank/unpublish actions.".to_string()),
            REGISTRY_REJECT_REASON_CODES,
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => action(
            "reject",
            false,
            Some("Resume the request before rejecting it.".to_string()),
            REGISTRY_REJECT_REASON_CODES,
        ),
        Some(_) => action("reject", false, None, REGISTRY_REJECT_REASON_CODES),
        None => action(
            "reject",
            false,
            Some("No publish request exists yet.".to_string()),
            REGISTRY_REJECT_REASON_CODES,
        ),
    };

    let request_changes_action = match latest_request {
        Some(request) if request.status == RegistryPublishRequestStatus::Approved => action(
            "request_changes",
            true,
            None,
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::ChangesRequested => {
            action(
                "request_changes",
                false,
                Some("Changes are already requested; upload a fresh artifact next.".to_string()),
                REGISTRY_REQUEST_CHANGES_REASON_CODES,
            )
        }
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => action(
            "request_changes",
            false,
            Some("Resume the request before requesting changes.".to_string()),
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
        ),
        Some(_) => action(
            "request_changes",
            false,
            Some(
                "Request-changes is available only for review-ready publish requests.".to_string(),
            ),
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
        ),
        None => action(
            "request_changes",
            false,
            Some("No publish request exists yet.".to_string()),
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
        ),
    };

    let hold_action = match latest_request {
        Some(request)
            if matches!(
                request.status,
                RegistryPublishRequestStatus::Submitted
                    | RegistryPublishRequestStatus::Approved
                    | RegistryPublishRequestStatus::ChangesRequested
            ) =>
        {
            action("hold", true, None, REGISTRY_HOLD_REASON_CODES)
        }
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => action(
            "hold",
            false,
            Some("This request is already on hold.".to_string()),
            REGISTRY_HOLD_REASON_CODES,
        ),
        Some(_) => action(
            "hold",
            false,
            Some(
                "Only submitted, approved, or changes_requested requests can be put on hold."
                    .to_string(),
            ),
            REGISTRY_HOLD_REASON_CODES,
        ),
        None => action(
            "hold",
            false,
            Some("No publish request exists yet.".to_string()),
            REGISTRY_HOLD_REASON_CODES,
        ),
    };

    let resume_action = match latest_request {
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => {
            action("resume", true, None, REGISTRY_RESUME_REASON_CODES)
        }
        Some(_) => action(
            "resume",
            false,
            Some("Only on-hold requests can be resumed.".to_string()),
            REGISTRY_RESUME_REASON_CODES,
        ),
        None => action(
            "resume",
            false,
            Some("No publish request exists yet.".to_string()),
            REGISTRY_RESUME_REASON_CODES,
        ),
    };

    let stage_report_action = match latest_request {
        Some(request)
            if matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            ) =>
        {
            action("stage_report", true, None, &[])
        }
        Some(request) if request.status == RegistryPublishRequestStatus::Rejected => action(
            "stage_report",
            false,
            Some("Rejected publish requests cannot accept follow-up stage updates.".to_string()),
            &[],
        ),
        Some(request) if request.status == RegistryPublishRequestStatus::OnHold => action(
            "stage_report",
            false,
            Some("Resume the request before reporting follow-up stages.".to_string()),
            &[],
        ),
        Some(_) => action(
            "stage_report",
            false,
            Some(
                "Follow-up stages are available only after artifact validation reaches review-ready."
                    .to_string(),
            ),
            &[],
        ),
        None => action(
            "stage_report",
            false,
            Some("No publish request exists yet.".to_string()),
            &[],
        ),
    };

    let owner_transfer_action = if owner_binding.is_some() {
        action(
            "owner_transfer",
            true,
            None,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
        )
    } else {
        action(
            "owner_transfer",
            false,
            Some("Owner transfer requires a persisted owner binding.".to_string()),
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
        )
    };

    let yank_action = match latest_release {
        Some(release) if release.status == RegistryModuleReleaseStatus::Active => {
            action("yank", true, None, REGISTRY_YANK_REASON_CODES)
        }
        Some(release) if release.status == RegistryModuleReleaseStatus::Yanked => action(
            "yank",
            false,
            Some(format!(
                "Release '{}@{}' is already yanked.",
                release.slug, release.version
            )),
            REGISTRY_YANK_REASON_CODES,
        ),
        None => action(
            "yank",
            false,
            Some("No published release exists yet.".to_string()),
            REGISTRY_YANK_REASON_CODES,
        ),
        Some(_) => action("yank", false, None, REGISTRY_YANK_REASON_CODES),
    };

    vec![
        validate_action,
        approve_action,
        reject_action,
        request_changes_action,
        hold_action,
        resume_action,
        stage_report_action,
        owner_transfer_action,
        yank_action,
    ]
}

#[derive(Debug, Clone)]
struct StoredRegistryArtifact {
    artifact_path: String,
    artifact_url: String,
    artifact_size: i64,
}

async fn load_registry_artifact(
    request: &registry_publish_request::Model,
) -> anyhow::Result<RegistryArtifactUpload> {
    let artifact_path = request.artifact_path.as_deref().ok_or_else(|| {
        anyhow!(
            "Registry publish request '{}' is missing artifact_path",
            request.id
        )
    })?;
    let bytes = tokio::fs::read(artifact_path).await.with_context(|| {
        format!(
            "failed to read registry artifact for request '{}' from {}",
            request.id, artifact_path
        )
    })?;

    Ok(RegistryArtifactUpload {
        content_type: request
            .artifact_content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        bytes: bytes::Bytes::from(bytes),
    })
}

async fn store_registry_artifact(
    request: &registry_publish_request::Model,
    artifact: &RegistryArtifactUpload,
) -> anyhow::Result<StoredRegistryArtifact> {
    let artifact_dir = workspace_root()
        .join("storage")
        .join("registry-artifacts")
        .join(&request.id);
    tokio::fs::create_dir_all(&artifact_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create registry artifact dir {}",
                artifact_dir.display()
            )
        })?;

    let artifact_path = artifact_dir.join(format!("{}-{}.crate", request.slug, request.version));
    tokio::fs::write(&artifact_path, &artifact.bytes)
        .await
        .with_context(|| {
            format!(
                "failed to write registry artifact {}",
                artifact_path.display()
            )
        })?;

    Ok(StoredRegistryArtifact {
        artifact_path: artifact_path.display().to_string(),
        artifact_url: artifact_path.display().to_string(),
        artifact_size: artifact.bytes.len() as i64,
    })
}

fn follow_up_validation_warning() -> &'static str {
    "Automated artifact and manifest contract checks passed, but compile smoke, targeted test smoke, and security/policy review still remain external follow-up gates before production approval."
}

fn follow_up_gate_detail(gate: &str) -> &'static str {
    match gate {
        "compile_smoke" => "Compile smoke still runs outside the current registry validator.",
        "targeted_tests" => {
            "Targeted module tests still run outside the current registry validator."
        }
        "security_policy_review" => {
            "Security and policy review still require an external gate before production approval."
        }
        _ => "External follow-up gate is still pending.",
    }
}

fn follow_up_validation_gate_details() -> Vec<RegistryValidationCheckDetail> {
    REGISTRY_VALIDATION_FOLLOW_UP_GATES
        .iter()
        .map(|gate| RegistryValidationCheckDetail {
            key: (*gate).to_string(),
            status: "pending_follow_up".to_string(),
            detail: follow_up_gate_detail(gate).to_string(),
        })
        .collect()
}

fn validation_passed_check_details() -> Vec<RegistryValidationCheckDetail> {
    vec![RegistryValidationCheckDetail {
        key: "artifact_bundle_contract".to_string(),
        status: "passed".to_string(),
        detail:
            "Artifact bundle JSON, rustok-module.toml parity, and crate/UI manifest contract checks passed."
                .to_string(),
    }]
}

fn validation_failed_check_details(errors: &[String]) -> Vec<RegistryValidationCheckDetail> {
    vec![RegistryValidationCheckDetail {
        key: "artifact_bundle_contract".to_string(),
        status: "failed".to_string(),
        detail: errors
            .first()
            .cloned()
            .unwrap_or_else(|| "Artifact bundle contract validation failed.".to_string()),
    }]
}

fn validation_retry_delay_seconds(failed_attempt: usize) -> Option<u64> {
    REGISTRY_VALIDATION_LOAD_RETRY_DELAYS_SECONDS
        .get(failed_attempt.saturating_sub(1))
        .copied()
}

pub fn validation_stage_status_label(status: RegistryValidationStageStatus) -> &'static str {
    match status {
        RegistryValidationStageStatus::Queued => "queued",
        RegistryValidationStageStatus::Running => "running",
        RegistryValidationStageStatus::Passed => "passed",
        RegistryValidationStageStatus::Failed => "failed",
        RegistryValidationStageStatus::Blocked => "blocked",
    }
}

fn validation_stage_execution_mode(stage_key: &str) -> &'static str {
    match stage_key {
        "compile_smoke" | "targeted_tests" => "local_runner",
        "security_policy_review" => "operator_assisted",
        _ => "manual",
    }
}

fn validation_stage_is_runnable(stage_key: &str) -> bool {
    matches!(
        stage_key,
        "compile_smoke" | "targeted_tests" | "security_policy_review"
    )
}

fn validation_stage_requires_manual_confirmation(stage_key: &str) -> bool {
    stage_key.eq_ignore_ascii_case("security_policy_review")
}

pub(crate) fn validation_stage_allowed_terminal_reason_codes(
    stage_key: &str,
) -> &'static [&'static str] {
    match stage_key {
        "compile_smoke" => &[
            "local_runner_passed",
            "build_failure",
            "manual_override",
            "other",
        ],
        "targeted_tests" => &[
            "local_runner_passed",
            "test_failure",
            "manual_override",
            "other",
        ],
        "security_policy_review" => &[
            "manual_review_complete",
            "policy_preflight_failed",
            "security_findings",
            "policy_exception",
            "license_issue",
            "manual_override",
            "other",
        ],
        _ => &["manual_override", "other"],
    }
}

fn validation_stage_suggested_pass_reason_code(stage_key: &str) -> Option<&'static str> {
    match stage_key {
        "compile_smoke" | "targeted_tests" => Some("local_runner_passed"),
        "security_policy_review" => Some("manual_review_complete"),
        _ => None,
    }
}

fn validation_stage_suggested_failure_reason_code(stage_key: &str) -> Option<&'static str> {
    match stage_key {
        "compile_smoke" => Some("build_failure"),
        "targeted_tests" => Some("test_failure"),
        "security_policy_review" => Some("policy_preflight_failed"),
        _ => None,
    }
}

fn validation_stage_suggested_blocked_reason_code(stage_key: &str) -> Option<&'static str> {
    match stage_key {
        "compile_smoke" | "targeted_tests" | "security_policy_review" => Some("manual_override"),
        _ => None,
    }
}

fn validation_stage_event_type(status: &RegistryValidationStageStatus) -> &'static str {
    match status {
        RegistryValidationStageStatus::Queued => "validation_stage_queued",
        RegistryValidationStageStatus::Running => "validation_stage_running",
        RegistryValidationStageStatus::Passed => "validation_stage_passed",
        RegistryValidationStageStatus::Failed => "validation_stage_failed",
        RegistryValidationStageStatus::Blocked => "validation_stage_blocked",
    }
}

fn parse_validation_stage_status(value: &str) -> anyhow::Result<RegistryValidationStageStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "queued" => Ok(RegistryValidationStageStatus::Queued),
        "running" => Ok(RegistryValidationStageStatus::Running),
        "passed" => Ok(RegistryValidationStageStatus::Passed),
        "failed" => Ok(RegistryValidationStageStatus::Failed),
        "blocked" => Ok(RegistryValidationStageStatus::Blocked),
        other => anyhow::bail!(
            "Unsupported validation stage status '{}'; expected queued, running, passed, failed, or blocked",
            other
        ),
    }
}

fn normalize_validation_stage_key(value: &str) -> anyhow::Result<&str> {
    let value = value.trim();
    if REGISTRY_VALIDATION_FOLLOW_UP_GATES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
    {
        let canonical = REGISTRY_VALIDATION_FOLLOW_UP_GATES
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(value))
            .copied()
            .expect("validated gate must exist");
        return Ok(canonical);
    }

    anyhow::bail!(
        "Unsupported validation stage '{}'; expected one of {}",
        value,
        REGISTRY_VALIDATION_FOLLOW_UP_GATES.join(", ")
    )
}

fn default_validation_stage_detail(
    stage_key: &str,
    status: &RegistryValidationStageStatus,
) -> String {
    match status {
        RegistryValidationStageStatus::Queued => follow_up_gate_detail(stage_key).to_string(),
        RegistryValidationStageStatus::Running => {
            format!("Validation stage '{stage_key}' is now running.")
        }
        RegistryValidationStageStatus::Passed => {
            format!("Validation stage '{stage_key}' passed.")
        }
        RegistryValidationStageStatus::Failed => {
            format!("Validation stage '{stage_key}' failed.")
        }
        RegistryValidationStageStatus::Blocked => {
            format!("Validation stage '{stage_key}' is blocked on external follow-up.")
        }
    }
}

fn ensure_validation_stage_transition_allowed(
    current: &RegistryValidationStageStatus,
    next: &RegistryValidationStageStatus,
    stage_key: &str,
) -> anyhow::Result<()> {
    let allowed = match current {
        RegistryValidationStageStatus::Queued => matches!(
            next,
            RegistryValidationStageStatus::Running
                | RegistryValidationStageStatus::Passed
                | RegistryValidationStageStatus::Failed
                | RegistryValidationStageStatus::Blocked
        ),
        RegistryValidationStageStatus::Running => matches!(
            next,
            RegistryValidationStageStatus::Running
                | RegistryValidationStageStatus::Passed
                | RegistryValidationStageStatus::Failed
                | RegistryValidationStageStatus::Blocked
        ),
        RegistryValidationStageStatus::Blocked => matches!(
            next,
            RegistryValidationStageStatus::Running
                | RegistryValidationStageStatus::Passed
                | RegistryValidationStageStatus::Failed
                | RegistryValidationStageStatus::Blocked
        ),
        RegistryValidationStageStatus::Passed | RegistryValidationStageStatus::Failed => false,
    };

    if allowed {
        return Ok(());
    }

    anyhow::bail!(
        "Validation stage '{}' cannot move from '{}' to '{}' without requeue",
        stage_key,
        validation_stage_status_label(current.clone()),
        validation_stage_status_label(next.clone())
    )
}

fn validation_stage_details_value(stage: &registry_validation_stage::Model) -> serde_json::Value {
    serde_json::json!({
        "stage_id": stage.id.clone(),
        "stage": stage.stage_key.clone(),
        "status": validation_stage_status_label(stage.status.clone()),
        "detail": stage.detail.clone(),
        "attempt_number": stage.attempt_number,
        "queue_reason": stage.queue_reason.clone(),
        "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
        "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
        "updated_at": stage.updated_at.to_rfc3339(),
    })
}

fn merge_json_object(target: &mut serde_json::Value, extra: serde_json::Value) {
    let Some(target_map) = target.as_object_mut() else {
        return;
    };
    let Some(extra_map) = extra.as_object() else {
        return;
    };
    for (key, value) in extra_map {
        target_map.insert(key.clone(), value.clone());
    }
}

fn derive_validation_stage_snapshots(
    latest_request: Option<&registry_publish_request::Model>,
    recent_events: &[registry_governance_event::Model],
    stage_rows: &[registry_validation_stage::Model],
) -> Vec<RegistryValidationStageSnapshot> {
    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();
    let mut latest_by_stage = HashMap::new();

    for stage in stage_rows {
        if seen.insert(stage.stage_key.as_str()) {
            latest_by_stage.insert(stage.stage_key.as_str(), stage);
        }
    }

    for stage_key in REGISTRY_VALIDATION_FOLLOW_UP_GATES {
        if let Some(stage) = latest_by_stage.get(stage_key) {
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: validation_stage_status_label(stage.status.clone()).to_string(),
                detail: stage.detail.clone(),
                attempt_number: stage.attempt_number,
                updated_at: stage.updated_at.to_rfc3339(),
                started_at: stage.started_at.as_ref().map(|value| value.to_rfc3339()),
                finished_at: stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
                execution_mode: validation_stage_execution_mode(stage_key).to_string(),
                runnable: validation_stage_is_runnable(stage_key),
                requires_manual_confirmation: validation_stage_requires_manual_confirmation(
                    stage_key,
                ),
                allowed_terminal_reason_codes: validation_stage_allowed_terminal_reason_codes(
                    stage_key,
                )
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
                suggested_pass_reason_code: validation_stage_suggested_pass_reason_code(stage_key)
                    .map(ToString::to_string),
                suggested_failure_reason_code: validation_stage_suggested_failure_reason_code(
                    stage_key,
                )
                .map(ToString::to_string),
                suggested_blocked_reason_code: validation_stage_suggested_blocked_reason_code(
                    stage_key,
                )
                .map(ToString::to_string),
            });
            continue;
        }

        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("gate")
                .and_then(serde_json::Value::as_str)
                == Some(*stage_key)
        });

        if let Some(event) = latest_event {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "queued",
                });
            let normalized_status = if status.eq_ignore_ascii_case("pending") {
                "queued"
            } else {
                status
            };
            let detail = event
                .details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| follow_up_gate_detail(stage_key));
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: normalized_status.to_string(),
                detail: detail.to_string(),
                attempt_number: 0,
                updated_at: event.created_at.to_rfc3339(),
                started_at: None,
                finished_at: None,
                execution_mode: validation_stage_execution_mode(stage_key).to_string(),
                runnable: validation_stage_is_runnable(stage_key),
                requires_manual_confirmation: validation_stage_requires_manual_confirmation(
                    stage_key,
                ),
                allowed_terminal_reason_codes: validation_stage_allowed_terminal_reason_codes(
                    stage_key,
                )
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
                suggested_pass_reason_code: validation_stage_suggested_pass_reason_code(stage_key)
                    .map(ToString::to_string),
                suggested_failure_reason_code: validation_stage_suggested_failure_reason_code(
                    stage_key,
                )
                .map(ToString::to_string),
                suggested_blocked_reason_code: validation_stage_suggested_blocked_reason_code(
                    stage_key,
                )
                .map(ToString::to_string),
            });
            continue;
        }

        if latest_request.is_some_and(|request| {
            matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            )
        }) {
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: "queued".to_string(),
                detail: follow_up_gate_detail(stage_key).to_string(),
                attempt_number: 0,
                updated_at: latest_request
                    .and_then(|request| {
                        request
                            .validated_at
                            .as_ref()
                            .or(request.approved_at.as_ref())
                    })
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default(),
                started_at: None,
                finished_at: None,
                execution_mode: validation_stage_execution_mode(stage_key).to_string(),
                runnable: validation_stage_is_runnable(stage_key),
                requires_manual_confirmation: validation_stage_requires_manual_confirmation(
                    stage_key,
                ),
                allowed_terminal_reason_codes: validation_stage_allowed_terminal_reason_codes(
                    stage_key,
                )
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
                suggested_pass_reason_code: validation_stage_suggested_pass_reason_code(stage_key)
                    .map(ToString::to_string),
                suggested_failure_reason_code: validation_stage_suggested_failure_reason_code(
                    stage_key,
                )
                .map(ToString::to_string),
                suggested_blocked_reason_code: validation_stage_suggested_blocked_reason_code(
                    stage_key,
                )
                .map(ToString::to_string),
            });
        }
    }

    snapshots
}

fn derive_follow_up_gate_snapshots(
    latest_request: Option<&registry_publish_request::Model>,
    recent_events: &[registry_governance_event::Model],
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<RegistryFollowUpGateSnapshot> {
    if !validation_stages.is_empty() {
        return validation_stages
            .iter()
            .map(|stage| RegistryFollowUpGateSnapshot {
                key: stage.key.clone(),
                status: match stage.status.as_str() {
                    "queued" => "pending".to_string(),
                    other => other.to_string(),
                },
                detail: stage.detail.clone(),
                updated_at: stage.updated_at.clone(),
            })
            .collect();
    }

    let mut snapshots = Vec::new();

    for gate in REGISTRY_VALIDATION_FOLLOW_UP_GATES {
        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("gate")
                .and_then(serde_json::Value::as_str)
                == Some(*gate)
        });

        if let Some(event) = latest_event {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "pending",
                });
            let detail = event
                .details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| follow_up_gate_detail(gate));

            snapshots.push(RegistryFollowUpGateSnapshot {
                key: (*gate).to_string(),
                status: status.to_string(),
                detail: detail.to_string(),
                updated_at: event.created_at.to_rfc3339(),
            });
            continue;
        }

        if latest_request.is_some_and(|request| {
            matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            )
        }) {
            snapshots.push(RegistryFollowUpGateSnapshot {
                key: (*gate).to_string(),
                status: "pending".to_string(),
                detail: follow_up_gate_detail(gate).to_string(),
                updated_at: latest_request
                    .and_then(|request| {
                        request
                            .validated_at
                            .as_ref()
                            .or(request.approved_at.as_ref())
                    })
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default(),
            });
        }
    }

    snapshots
}

fn rejected_publish_request_can_retry(
    latest_event_type: Option<&str>,
    rejection_reason: Option<&str>,
) -> bool {
    if matches!(latest_event_type, Some("validation_failed")) {
        return true;
    }

    rejection_reason
        .is_some_and(|reason| !reason.trim().starts_with("Governance rejection reason:"))
}

fn normalize_actor(value: &str) -> String {
    let actor = value.trim();
    if actor.is_empty() {
        "system:auto".to_string()
    } else {
        actor.to_string()
    }
}

fn remote_runner_actor(runner_id: &str) -> String {
    let runner_id = runner_id.trim();
    if runner_id.is_empty() {
        "system:registry-remote-runner:anonymous".to_string()
    } else {
        format!("system:registry-remote-runner:{runner_id}")
    }
}

fn normalize_optional_actor(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_actor)
}

fn dedupe_message_list(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn deserialize_message_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

fn compare_semver_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/server")
}

fn parse_request_status_label(value: &str) -> Option<RegistryPublishRequestStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "draft" => Some(RegistryPublishRequestStatus::Draft),
        "artifact_uploaded" => Some(RegistryPublishRequestStatus::ArtifactUploaded),
        "submitted" => Some(RegistryPublishRequestStatus::Submitted),
        "validating" => Some(RegistryPublishRequestStatus::Validating),
        "approved" => Some(RegistryPublishRequestStatus::Approved),
        "changes_requested" => Some(RegistryPublishRequestStatus::ChangesRequested),
        "on_hold" => Some(RegistryPublishRequestStatus::OnHold),
        "rejected" => Some(RegistryPublishRequestStatus::Rejected),
        "published" => Some(RegistryPublishRequestStatus::Published),
        _ => None,
    }
}

pub fn request_status_label(status: RegistryPublishRequestStatus) -> &'static str {
    match status {
        RegistryPublishRequestStatus::Draft => "draft",
        RegistryPublishRequestStatus::ArtifactUploaded => "artifact_uploaded",
        RegistryPublishRequestStatus::Submitted => "submitted",
        RegistryPublishRequestStatus::Validating => "validating",
        RegistryPublishRequestStatus::Approved => "approved",
        RegistryPublishRequestStatus::ChangesRequested => "changes_requested",
        RegistryPublishRequestStatus::OnHold => "on_hold",
        RegistryPublishRequestStatus::Rejected => "rejected",
        RegistryPublishRequestStatus::Published => "published",
    }
}

pub fn release_status_label(status: RegistryModuleReleaseStatus) -> &'static str {
    match status {
        RegistryModuleReleaseStatus::Active => "active",
        RegistryModuleReleaseStatus::Yanked => "yanked",
    }
}

pub fn request_ui_packages(
    request: &registry_publish_request::Model,
) -> RegistryPublishUiPackagesRequest {
    serde_json::from_value(request.ui_packages.clone()).unwrap_or_default()
}

pub fn actor_can_manage_registry_slug(actor: &str, slug: &str) -> bool {
    let actor = normalize_actor(actor);
    actor_is_registry_governance(&actor) || legacy_actor_can_manage_registry_slug(&actor, slug)
}

fn legacy_actor_can_manage_registry_slug(actor: &str, slug: &str) -> bool {
    actor == "publisher:*" || actor == format!("publisher:{slug}")
}

fn validate_registry_artifact_bundle(
    request: &registry_publish_request::Model,
    artifact: &RegistryArtifactUpload,
) -> RegistryArtifactValidation {
    let mut validation = RegistryArtifactValidation::default();

    if !artifact
        .content_type
        .eq_ignore_ascii_case("application/json")
    {
        validation.warnings.push(format!(
            "Artifact upload content-type '{}' is accepted, but application/json is the canonical bundle content-type.",
            artifact.content_type
        ));
    }

    let bundle = match serde_json::from_slice::<RegistryPublishArtifactBundle>(&artifact.bytes) {
        Ok(bundle) => bundle,
        Err(error) => {
            validation.errors.push(format!(
                "Artifact bundle is not valid JSON for the registry publish contract: {error}"
            ));
            return validation;
        }
    };

    if bundle.schema_version != REGISTRY_MUTATION_SCHEMA_VERSION {
        validation.errors.push(format!(
            "Artifact bundle schema_version '{}' does not match registry mutation schema '{}'.",
            bundle.schema_version, REGISTRY_MUTATION_SCHEMA_VERSION
        ));
    }
    if bundle.artifact_type != REGISTRY_ARTIFACT_BUNDLE_TYPE {
        validation.errors.push(format!(
            "Artifact bundle type '{}' does not match expected '{}'.",
            bundle.artifact_type, REGISTRY_ARTIFACT_BUNDLE_TYPE
        ));
    }

    validate_artifact_module_contract(request, &bundle, &mut validation);
    validate_artifact_file_contract(request, &bundle, &mut validation);

    validation.warnings = dedupe_message_list(validation.warnings);
    validation.errors = dedupe_message_list(validation.errors);
    validation
}

fn validate_artifact_module_contract(
    request: &registry_publish_request::Model,
    bundle: &RegistryPublishArtifactBundle,
    validation: &mut RegistryArtifactValidation,
) {
    let request_marketplace: RegistryPublishMarketplaceRequest =
        serde_json::from_value(request.marketplace.clone()).unwrap_or_default();
    let request_ui = request_ui_packages(request);

    validate_exact_field(
        "module.slug",
        &bundle.module.slug,
        &request.slug,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.version",
        &bundle.module.version,
        &request.version,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.crate_name",
        &bundle.module.crate_name,
        &request.crate_name,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.name",
        &bundle.module.module_name,
        &request.module_name,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.description",
        &bundle.module.module_description,
        &request.description,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.ownership",
        &bundle.module.ownership,
        &request.ownership,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.trust_level",
        &bundle.module.trust_level,
        &request.trust_level,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.license",
        &bundle.module.license,
        &request.license,
        &mut validation.errors,
    );
    validate_optional_field(
        "module.entry_type",
        bundle.module.module_entry_type.as_deref(),
        request.entry_type.as_deref(),
        &mut validation.errors,
    );
    validate_optional_field(
        "module.marketplace.category",
        bundle.module.marketplace.category.as_deref(),
        request_marketplace.category.as_deref(),
        &mut validation.errors,
    );

    if normalize_string_list(&bundle.module.marketplace.tags)
        != normalize_string_list(&request_marketplace.tags)
    {
        validation.errors.push(format!(
            "Artifact bundle module.marketplace.tags {:?} does not match publish request {:?}.",
            bundle.module.marketplace.tags, request_marketplace.tags
        ));
    }

    validate_optional_field(
        "module.ui_packages.admin.crate_name",
        bundle
            .module
            .ui_packages
            .admin
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        request_ui.admin.as_ref().map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
    validate_optional_field(
        "module.ui_packages.storefront.crate_name",
        bundle
            .module
            .ui_packages
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        request_ui
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
}

fn validate_artifact_file_contract(
    request: &registry_publish_request::Model,
    bundle: &RegistryPublishArtifactBundle,
    validation: &mut RegistryArtifactValidation,
) {
    let request_marketplace: RegistryPublishMarketplaceRequest =
        serde_json::from_value(request.marketplace.clone()).unwrap_or_default();
    let request_ui = request_ui_packages(request);

    let package_manifest = require_bundle_file(
        "rustok-module.toml",
        bundle.files.package_manifest.as_deref(),
        &mut validation.errors,
    );
    let crate_manifest = require_bundle_file(
        "Cargo.toml",
        bundle.files.crate_manifest.as_deref(),
        &mut validation.errors,
    );

    match (&request_ui.admin, bundle.files.admin_manifest.as_deref()) {
        (Some(_), None) => validation.errors.push(
            "Artifact bundle must include admin/Cargo.toml because the publish request declares an admin UI package."
                .to_string(),
        ),
        (None, Some(_)) => validation.errors.push(
            "Artifact bundle includes admin/Cargo.toml, but the publish request does not declare an admin UI package."
                .to_string(),
        ),
        _ => {}
    }
    match (&request_ui.storefront, bundle.files.storefront_manifest.as_deref()) {
        (Some(_), None) => validation.errors.push(
            "Artifact bundle must include storefront/Cargo.toml because the publish request declares a storefront UI package."
                .to_string(),
        ),
        (None, Some(_)) => validation.errors.push(
            "Artifact bundle includes storefront/Cargo.toml, but the publish request does not declare a storefront UI package."
                .to_string(),
        ),
        _ => {}
    }

    if let Some(source) = package_manifest {
        validate_package_manifest_contract(
            source,
            request,
            &request_marketplace,
            &request_ui,
            validation,
        );
    }
    if let Some(source) = crate_manifest {
        validate_cargo_manifest_contract(
            "Cargo.toml",
            source,
            &request.crate_name,
            &request.version,
            Some(&request.license),
            validation,
        );
    }
    if let (Some(ui), Some(source)) = (&request_ui.admin, bundle.files.admin_manifest.as_deref()) {
        validate_cargo_manifest_contract(
            "admin/Cargo.toml",
            source,
            &ui.crate_name,
            &request.version,
            None,
            validation,
        );
    }
    if let (Some(ui), Some(source)) = (
        &request_ui.storefront,
        bundle.files.storefront_manifest.as_deref(),
    ) {
        validate_cargo_manifest_contract(
            "storefront/Cargo.toml",
            source,
            &ui.crate_name,
            &request.version,
            None,
            validation,
        );
    }
}

fn validate_package_manifest_contract(
    source: &str,
    request: &registry_publish_request::Model,
    request_marketplace: &RegistryPublishMarketplaceRequest,
    request_ui: &RegistryPublishUiPackagesRequest,
    validation: &mut RegistryArtifactValidation,
) {
    let manifest = match source.parse::<toml::Value>() {
        Ok(manifest) => manifest,
        Err(error) => {
            validation.errors.push(format!(
                "Artifact file rustok-module.toml is not valid TOML: {error}"
            ));
            return;
        }
    };

    validate_toml_string_field(
        &manifest,
        &["module", "slug"],
        "rustok-module.toml [module].slug",
        &request.slug,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "name"],
        "rustok-module.toml [module].name",
        &request.module_name,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "version"],
        "rustok-module.toml [module].version",
        &request.version,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "description"],
        "rustok-module.toml [module].description",
        &request.description,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "ownership"],
        "rustok-module.toml [module].ownership",
        &request.ownership,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "trust_level"],
        "rustok-module.toml [module].trust_level",
        &request.trust_level,
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["marketplace", "category"],
        "rustok-module.toml [marketplace].category",
        request_marketplace.category.as_deref(),
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["crate", "entry_type"],
        "rustok-module.toml [crate].entry_type",
        request.entry_type.as_deref(),
        &mut validation.errors,
    );

    if toml_string_list_field(&manifest, &["marketplace", "tags"])
        != normalize_string_list(&request_marketplace.tags)
    {
        validation.errors.push(format!(
            "Artifact file rustok-module.toml [marketplace].tags {:?} does not match publish request {:?}.",
            toml_string_list_field(&manifest, &["marketplace", "tags"]),
            request_marketplace.tags
        ));
    }

    validate_toml_optional_string_field(
        &manifest,
        &["provides", "admin_ui", "leptos_crate"],
        "rustok-module.toml [provides.admin_ui].leptos_crate",
        request_ui.admin.as_ref().map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["provides", "storefront_ui", "leptos_crate"],
        "rustok-module.toml [provides.storefront_ui].leptos_crate",
        request_ui
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
}

fn validate_cargo_manifest_contract(
    label: &str,
    source: &str,
    expected_name: &str,
    expected_version: &str,
    expected_license: Option<&str>,
    validation: &mut RegistryArtifactValidation,
) {
    let manifest = match source.parse::<toml::Value>() {
        Ok(manifest) => manifest,
        Err(error) => {
            validation
                .errors
                .push(format!("Artifact file {label} is not valid TOML: {error}"));
            return;
        }
    };

    validate_toml_string_field(
        &manifest,
        &["package", "name"],
        &format!("{label} [package].name"),
        expected_name,
        &mut validation.errors,
    );
    validate_toml_workspace_aware_string_field(
        &manifest,
        &["package", "version"],
        &format!("{label} [package].version"),
        expected_version,
        &mut validation.warnings,
        &mut validation.errors,
    );
    if let Some(expected_license) = expected_license {
        validate_toml_workspace_aware_string_field(
            &manifest,
            &["package", "license"],
            &format!("{label} [package].license"),
            expected_license,
            &mut validation.warnings,
            &mut validation.errors,
        );
    }
}

fn validate_exact_field(label: &str, actual: &str, expected: &str, errors: &mut Vec<String>) {
    if actual.trim() != expected.trim() {
        errors.push(format!(
            "Artifact bundle {label} '{}' does not match publish request '{}'.",
            actual, expected
        ));
    }
}

fn validate_optional_field(
    label: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    errors: &mut Vec<String>,
) {
    let actual = actual.map(str::trim).filter(|value| !value.is_empty());
    let expected = expected.map(str::trim).filter(|value| !value.is_empty());
    if actual != expected {
        errors.push(format!(
            "Artifact bundle {label} {:?} does not match publish request {:?}.",
            actual, expected
        ));
    }
}

fn require_bundle_file<'a>(
    label: &str,
    source: Option<&'a str>,
    errors: &mut Vec<String>,
) -> Option<&'a str> {
    match source.map(str::trim) {
        Some(source) if !source.is_empty() => Some(source),
        _ => {
            errors.push(format!(
                "Artifact bundle must include non-empty file '{label}'."
            ));
            None
        }
    }
}

fn normalize_string_list(values: &[String]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn toml_value_at_path<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn toml_string_field(value: &toml::Value, path: &[&str]) -> Option<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn toml_string_list_field(value: &toml::Value, path: &[&str]) -> Vec<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .map(|mut values| {
            values.sort();
            values.dedup();
            values
        })
        .unwrap_or_default()
}

fn toml_is_workspace_inherited(value: &toml::Value, path: &[&str]) -> bool {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

fn validate_toml_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    errors: &mut Vec<String>,
) {
    let actual = toml_string_field(manifest, path);
    if actual.as_deref() != Some(expected.trim()) {
        errors.push(format!(
            "Artifact file {label} {:?} does not match publish request '{}'.",
            actual, expected
        ));
    }
}

fn validate_toml_optional_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: Option<&str>,
    errors: &mut Vec<String>,
) {
    let actual = toml_string_field(manifest, path);
    let expected = expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if actual != expected {
        errors.push(format!(
            "Artifact file {label} {:?} does not match publish request {:?}.",
            actual, expected
        ));
    }
}

fn validate_toml_workspace_aware_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(actual) = toml_string_field(manifest, path) {
        if actual != expected.trim() {
            errors.push(format!(
                "Artifact file {label} '{}' does not match publish request '{}'.",
                actual, expected
            ));
        }
        return;
    }

    if toml_is_workspace_inherited(manifest, path) {
        warnings.push(format!(
            "Artifact file {label} uses workspace inheritance, so the registry validator cannot verify it from the uploaded bundle alone."
        ));
        return;
    }

    warnings.push(format!(
        "Artifact file {label} is missing, so the registry validator could not verify it from the uploaded bundle."
    ));
}

fn actor_is_registry_admin(actor: &str) -> bool {
    actor.starts_with("system:")
        || actor.starts_with("xtask:")
        || actor == "registry:admin"
        || actor.starts_with("registry:")
}

fn actor_is_registry_review_governance(actor: &str) -> bool {
    actor_is_registry_admin(actor)
        || actor == "governance:moderator"
        || actor.starts_with("moderator:")
}

fn actor_is_registry_governance(actor: &str) -> bool {
    actor_is_registry_review_governance(actor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    };

    #[test]
    fn artifact_bundle_validation_accepts_matching_bundle() {
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", true).into_bytes()),
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);

        assert!(validation.errors.is_empty(), "{:?}", validation.errors);
    }

    #[test]
    fn artifact_bundle_validation_rejects_mismatched_slug() {
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("forum", true).into_bytes()),
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);

        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("module.slug")),
            "{:?}",
            validation.errors
        );
    }

    #[test]
    fn artifact_bundle_validation_rejects_missing_requested_admin_manifest() {
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", false).into_bytes()),
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);

        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("admin/Cargo.toml")),
            "{:?}",
            validation.errors
        );
    }

    #[test]
    fn rejected_publish_request_can_retry_after_validation_failure() {
        assert!(rejected_publish_request_can_retry(
            Some("validation_failed"),
            Some("Validation job failed before bundle checks: missing artifact"),
        ));
        assert!(rejected_publish_request_can_retry(
            None,
            Some("Validation job failed before bundle checks: missing artifact"),
        ));
    }

    #[test]
    fn rejected_publish_request_cannot_retry_after_manual_governance_reject() {
        assert!(!rejected_publish_request_can_retry(
            Some("request_rejected"),
            Some("Governance rejection reason: owner mismatch"),
        ));
    }

    #[test]
    fn registry_review_governance_roles_include_admin_and_moderator() {
        assert!(actor_is_registry_review_governance("registry:admin"));
        assert!(actor_is_registry_review_governance("registry:ops"));
        assert!(actor_is_registry_review_governance("governance:moderator"));
        assert!(actor_is_registry_review_governance("moderator:content"));
    }

    #[test]
    fn registry_admin_role_is_stricter_than_review_governance() {
        assert!(actor_is_registry_admin("registry:admin"));
        assert!(actor_is_registry_admin("registry:ops"));
        assert!(!actor_is_registry_admin("governance:moderator"));
        assert!(!actor_is_registry_admin("moderator:content"));
    }

    #[test]
    fn validation_retry_delay_schedule_uses_backoff() {
        assert_eq!(validation_retry_delay_seconds(1), Some(1));
        assert_eq!(validation_retry_delay_seconds(2), Some(3));
        assert_eq!(validation_retry_delay_seconds(3), Some(5));
        assert_eq!(validation_retry_delay_seconds(4), None);
    }

    #[test]
    fn derive_follow_up_gate_snapshots_reads_latest_gate_events() {
        let now = Utc::now();
        let mut request = sample_publish_request_model();
        request.status = RegistryPublishRequestStatus::Approved;
        request.validated_at = Some(now);
        let events = vec![
            registry_governance_event::Model {
                id: "rge_compile".to_string(),
                slug: "blog".to_string(),
                request_id: Some(request.id.clone()),
                release_id: None,
                event_type: "follow_up_gate_queued".to_string(),
                actor: "xtask:module-publish".to_string(),
                publisher: None,
                details: serde_json::json!({
                    "gate": "compile_smoke",
                    "status": "pending",
                    "detail": "Compile smoke still runs outside the current registry validator."
                }),
                created_at: now,
            },
            registry_governance_event::Model {
                id: "rge_tests".to_string(),
                slug: "blog".to_string(),
                request_id: Some(request.id.clone()),
                release_id: None,
                event_type: "follow_up_gate_failed".to_string(),
                actor: "governance:moderator".to_string(),
                publisher: None,
                details: serde_json::json!({
                    "gate": "targeted_tests",
                    "status": "failed",
                    "detail": "Targeted tests failed in CI."
                }),
                created_at: now,
            },
        ];

        let validation_stages = derive_validation_stage_snapshots(Some(&request), &events, &[]);
        let snapshots =
            derive_follow_up_gate_snapshots(Some(&request), &events, &validation_stages);

        assert_eq!(snapshots.len(), 3);
        assert_eq!(
            snapshots
                .iter()
                .find(|gate| gate.key == "compile_smoke")
                .map(|gate| gate.status.as_str()),
            Some("pending")
        );
        assert_eq!(
            snapshots
                .iter()
                .find(|gate| gate.key == "targeted_tests")
                .map(|gate| gate.status.as_str()),
            Some("failed")
        );
        assert_eq!(
            snapshots
                .iter()
                .find(|gate| gate.key == "security_policy_review")
                .map(|gate| gate.status.as_str()),
            Some("pending")
        );
    }

    #[tokio::test]
    async fn validate_publish_request_queues_single_active_validation_job() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Submitted).await;
        let service = RegistryGovernanceService::new(db.clone());

        let queued = service
            .validate_publish_request(&request.id, &request.requested_by)
            .await
            .unwrap();
        assert!(queued.queued);
        assert!(queued.validation_job_id.is_some());
        assert_eq!(
            queued.request.status,
            RegistryPublishRequestStatus::Validating
        );

        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id.clone()))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, RegistryValidationJobStatus::Queued);
        assert_eq!(jobs[0].attempt_number, 1);

        let second = service
            .validate_publish_request(&request.id, &request.requested_by)
            .await
            .unwrap();
        assert!(!second.queued);
        assert_eq!(
            second.request.status,
            RegistryPublishRequestStatus::Validating
        );
        assert_eq!(second.validation_job_id, queued.validation_job_id);

        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[tokio::test]
    async fn validate_publish_request_requeues_after_automated_failure_with_incremented_attempt() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Rejected).await;
        insert_failed_validation_job(&db, &request).await;
        insert_validation_failed_event(&db, &request).await;
        let service = RegistryGovernanceService::new(db.clone());

        let queued = service
            .validate_publish_request(&request.id, &request.requested_by)
            .await
            .unwrap();

        assert!(queued.queued);
        assert_eq!(
            queued.request.status,
            RegistryPublishRequestStatus::Validating
        );

        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id))
            .order_by_asc(registry_validation_job::Column::AttemptNumber)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].status, RegistryValidationJobStatus::Failed);
        assert_eq!(jobs[0].attempt_number, 1);
        assert_eq!(jobs[1].status, RegistryValidationJobStatus::Queued);
        assert_eq!(jobs[1].attempt_number, 2);
        assert_eq!(jobs[1].queue_reason, "requeued_after_validation_failed");
    }

    #[tokio::test]
    async fn run_publish_validation_job_materializes_follow_up_validation_stages() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request_with_artifact(
            &db,
            RegistryPublishRequestStatus::Submitted,
            sample_publish_artifact_json("blog", true),
        )
        .await;
        let service = RegistryGovernanceService::new(db.clone());

        let queued = service
            .validate_publish_request(&request.id, &request.requested_by)
            .await
            .unwrap();
        let job_id = queued.validation_job_id.expect("validation job id");
        let validated = service
            .run_publish_validation_job(&job_id, &request.requested_by)
            .await
            .unwrap();

        assert_eq!(validated.status, RegistryPublishRequestStatus::Approved);

        let stages = RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request.id))
            .order_by_asc(registry_validation_stage::Column::StageKey)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(stages.len(), 3);
        assert!(stages
            .iter()
            .all(|stage| stage.status == RegistryValidationStageStatus::Queued));
        assert!(stages.iter().all(|stage| stage.attempt_number == 1));
    }

    #[tokio::test]
    async fn report_validation_stage_requeue_increments_attempt_number() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        insert_validation_stage(
            &db,
            &request,
            "compile_smoke",
            RegistryValidationStageStatus::Queued,
            1,
            "Compile smoke queued.",
        )
        .await;
        let service = RegistryGovernanceService::new(db.clone());

        let failed = service
            .report_validation_stage(
                &request.id,
                &request.requested_by,
                "compile_smoke",
                "failed",
                Some("Compile smoke failed in CI."),
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(failed.stage.attempt_number, 1);
        assert_eq!(failed.stage.status, RegistryValidationStageStatus::Failed);

        let requeued = service
            .report_validation_stage(
                &request.id,
                &request.requested_by,
                "compile_smoke",
                "queued",
                Some("Compile smoke queued again after fixes."),
                None,
                true,
            )
            .await
            .unwrap();
        assert_eq!(requeued.stage.attempt_number, 2);
        assert_eq!(requeued.stage.status, RegistryValidationStageStatus::Queued);

        let stages = RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request.id))
            .filter(registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_asc(registry_validation_stage::Column::AttemptNumber)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].status, RegistryValidationStageStatus::Failed);
        assert_eq!(stages[1].status, RegistryValidationStageStatus::Queued);
    }

    #[tokio::test]
    async fn lifecycle_snapshot_prefers_persisted_validation_stages() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        insert_validation_stage(
            &db,
            &request,
            "compile_smoke",
            RegistryValidationStageStatus::Blocked,
            1,
            "Compile smoke is blocked on an external runner.",
        )
        .await;
        insert_follow_up_gate_event(
            &db,
            &request,
            "compile_smoke",
            "follow_up_gate_passed",
            "passed",
            "Legacy event should be ignored once persisted stages exist.",
        )
        .await;
        let service = RegistryGovernanceService::new(db.clone());

        let snapshot = service
            .lifecycle_snapshot(&request.slug, &request.ownership)
            .await
            .unwrap()
            .expect("lifecycle snapshot");

        assert_eq!(
            snapshot
                .validation_stages
                .iter()
                .find(|stage| stage.key == "compile_smoke")
                .map(|stage| stage.status.as_str()),
            Some("blocked")
        );
        assert_eq!(
            snapshot
                .follow_up_gates
                .iter()
                .find(|stage| stage.key == "compile_smoke")
                .map(|stage| stage.status.as_str()),
            Some("blocked")
        );
    }

    fn sample_publish_request_model() -> registry_publish_request::Model {
        registry_publish_request::Model {
            id: "rpr_test".to_string(),
            slug: "blog".to_string(),
            version: "0.1.0".to_string(),
            crate_name: "rustok-blog".to_string(),
            module_name: "Blog".to_string(),
            description: "Blog module description long enough for validation.".to_string(),
            ownership: "first_party".to_string(),
            trust_level: "core".to_string(),
            license: "MIT".to_string(),
            entry_type: Some("backend".to_string()),
            marketplace: serde_json::json!({
                "category": "content",
                "tags": ["blog", "content"]
            }),
            ui_packages: serde_json::json!({
                "admin": { "crate_name": "rustok-blog-admin" },
                "storefront": { "crate_name": "rustok-blog-storefront" }
            }),
            status: RegistryPublishRequestStatus::Draft,
            requested_by: "xtask:module-publish".to_string(),
            publisher_identity: Some("publisher:blog".to_string()),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            changes_requested_by: None,
            changes_requested_reason: None,
            changes_requested_reason_code: None,
            changes_requested_at: None,
            held_by: None,
            held_reason: None,
            held_reason_code: None,
            held_at: None,
            held_from_status: None,
            validation_warnings: serde_json::json!([]),
            validation_errors: serde_json::json!([]),
            artifact_path: None,
            artifact_url: None,
            artifact_checksum_sha256: None,
            artifact_size: None,
            artifact_content_type: None,
            submitted_at: None,
            validated_at: None,
            approved_at: None,
            published_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    async fn insert_publish_request(
        db: &DatabaseConnection,
        status: RegistryPublishRequestStatus,
    ) -> registry_publish_request::Model {
        let now = Utc::now();
        let mut request = sample_publish_request_model();
        request.id = format!("rpr_{}", uuid::Uuid::new_v4().simple());
        request.status = status;
        request.created_at = now;
        request.updated_at = now;
        request.submitted_at = matches!(
            &request.status,
            RegistryPublishRequestStatus::Submitted
                | RegistryPublishRequestStatus::Validating
                | RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::Published
        )
        .then_some(now);
        request.validated_at = matches!(
            &request.status,
            RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::Published
        )
        .then_some(now);
        let active: RegistryPublishRequestActiveModel = request.clone().into();
        active.insert(db).await.unwrap()
    }

    async fn insert_publish_request_with_artifact(
        db: &DatabaseConnection,
        status: RegistryPublishRequestStatus,
        artifact_json: String,
    ) -> registry_publish_request::Model {
        let mut request = insert_publish_request(db, status).await;
        let artifact_dir = std::env::temp_dir().join(format!(
            "rustok-registry-governance-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let artifact_path = artifact_dir.join(format!("{}-{}.json", request.slug, request.version));
        std::fs::write(&artifact_path, artifact_json).unwrap();
        let mut active: RegistryPublishRequestActiveModel = request.clone().into();
        active.artifact_path = Set(Some(artifact_path.display().to_string()));
        active.artifact_url = Set(Some(artifact_path.display().to_string()));
        active.artifact_checksum_sha256 = Set(Some("checksum".to_string()));
        active.artifact_size = Set(Some(artifact_path.metadata().unwrap().len() as i64));
        active.artifact_content_type = Set(Some("application/json".to_string()));
        active.updated_at = Set(Utc::now());
        request = active.update(db).await.unwrap();
        request
    }

    async fn insert_failed_validation_job(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
    ) {
        let now = Utc::now();
        let active = RegistryValidationJobActiveModel {
            id: Set(format!("rvj_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            status: Set(RegistryValidationJobStatus::Failed),
            triggered_by: Set(request.requested_by.clone()),
            queue_reason: Set("initial_validation".to_string()),
            attempt_number: Set(1),
            started_at: Set(Some(now)),
            finished_at: Set(Some(now)),
            last_error: Set(Some("Validation failed".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_validation_failed_event(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
    ) {
        let active = RegistryGovernanceEventActiveModel {
            id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
            slug: Set(request.slug.clone()),
            request_id: Set(Some(request.id.clone())),
            release_id: Set(None),
            event_type: Set("validation_failed".to_string()),
            actor: Set(request.requested_by.clone()),
            publisher: Set(None),
            details: Set(serde_json::json!({
                "version": request.version.clone(),
                "status": "rejected",
                "errors": ["Validation failed"],
            })),
            created_at: Set(Utc::now()),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_validation_stage(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
        stage_key: &str,
        status: RegistryValidationStageStatus,
        attempt_number: i32,
        detail: &str,
    ) {
        let now = Utc::now();
        let active = RegistryValidationStageActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set(stage_key.to_string()),
            status: Set(status.clone()),
            triggered_by: Set(request.requested_by.clone()),
            queue_reason: Set("test_setup".to_string()),
            attempt_number: Set(attempt_number),
            detail: Set(detail.to_string()),
            started_at: Set(None),
            finished_at: Set(matches!(
                status,
                RegistryValidationStageStatus::Passed
                    | RegistryValidationStageStatus::Failed
                    | RegistryValidationStageStatus::Blocked
            )
            .then_some(now)),
            last_error: Set(matches!(status, RegistryValidationStageStatus::Failed)
                .then_some(detail.to_string())),
            claim_id: Set(None),
            claimed_by: Set(None),
            claim_expires_at: Set(None),
            last_heartbeat_at: Set(None),
            runner_kind: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_follow_up_gate_event(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
        gate: &str,
        event_type: &str,
        status: &str,
        detail: &str,
    ) {
        let active = RegistryGovernanceEventActiveModel {
            id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
            slug: Set(request.slug.clone()),
            request_id: Set(Some(request.id.clone())),
            release_id: Set(None),
            event_type: Set(event_type.to_string()),
            actor: Set(request.requested_by.clone()),
            publisher: Set(None),
            details: Set(serde_json::json!({
                "gate": gate,
                "status": status,
                "detail": detail,
            })),
            created_at: Set(Utc::now()),
        };
        active.insert(db).await.unwrap();
    }

    fn sample_publish_artifact_json(slug: &str, include_admin_manifest: bool) -> String {
        let package_manifest = r#"
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
description = "Blog module description long enough for validation."
ownership = "first_party"
trust_level = "core"

[marketplace]
category = "content"
tags = ["blog", "content"]

[crate]
entry_type = "backend"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
"#;
        let crate_manifest = r#"
[package]
name = "rustok-blog"
version = "0.1.0"
license = "MIT"
"#;
        let admin_manifest = include_admin_manifest.then_some(
            r#"
[package]
name = "rustok-blog-admin"
version = "0.1.0"
"#,
        );
        let storefront_manifest = Some(
            r#"
[package]
name = "rustok-blog-storefront"
version = "0.1.0"
"#,
        );

        serde_json::json!({
            "schema_version": REGISTRY_MUTATION_SCHEMA_VERSION,
            "artifact_type": REGISTRY_ARTIFACT_BUNDLE_TYPE,
            "module": {
                "slug": slug,
                "version": "0.1.0",
                "crate_name": "rustok-blog",
                "module_name": "Blog",
                "module_description": "Blog module description long enough for validation.",
                "ownership": "first_party",
                "trust_level": "core",
                "license": "MIT",
                "module_entry_type": "backend",
                "marketplace": {
                    "category": "content",
                    "tags": ["blog", "content"]
                },
                "ui_packages": {
                    "admin": {
                        "crate_name": "rustok-blog-admin",
                        "manifest_path": "crates/rustok-blog/admin/Cargo.toml"
                    },
                    "storefront": {
                        "crate_name": "rustok-blog-storefront",
                        "manifest_path": "crates/rustok-blog/storefront/Cargo.toml"
                    }
                }
            },
            "files": {
                "rustok-module.toml": package_manifest,
                "Cargo.toml": crate_manifest,
                "admin/Cargo.toml": admin_manifest,
                "storefront/Cargo.toml": storefront_manifest
            }
        })
        .to_string()
    }
}
