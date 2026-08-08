use anyhow::{Context, anyhow};
use object_store::{ObjectStoreExt, PutMode, path::Path};
use rustok_modules::{ModuleControlPlane, SeaOrmModuleGovernanceService};
use rustok_storage::StorageRuntime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::registry_module_owner::{self, Entity as RegistryModuleOwnerEntity};
use crate::models::registry_module_release::{
    self, Entity as RegistryModuleReleaseEntity, RegistryModuleReleaseStatus,
};
use crate::models::registry_publish_request::RegistryPublishRequestStatus;
use crate::models::registry_validation_stage::RegistryValidationStageStatus;
use crate::services::marketplace_catalog::{RegistryPublishArtifactOrigin, RegistryPublishRequest};
use crate::services::registry_principal::{RegistryAuthority, RegistryPrincipalRef};
use thiserror::Error;

pub use rustok_modules::MODULE_PUBLISH_ARTIFACT_MAX_BYTES;
const REGISTRY_VALIDATION_FOLLOW_UP_GATES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];
pub use rustok_modules::REGISTRY_APPROVE_OVERRIDE_REASON_CODES;
pub use rustok_modules::REGISTRY_HOLD_REASON_CODES;
pub use rustok_modules::REGISTRY_OWNER_TRANSFER_REASON_CODES;
pub use rustok_modules::REGISTRY_REJECT_REASON_CODES;
pub use rustok_modules::REGISTRY_REQUEST_CHANGES_REASON_CODES;
pub use rustok_modules::REGISTRY_RESUME_REASON_CODES;
pub use rustok_modules::REGISTRY_VALIDATION_STAGE_REASON_CODES;
pub use rustok_modules::REGISTRY_YANK_REASON_CODES;

#[cfg(feature = "mod-alloy")]
pub(crate) fn alloy_release_governance_handle(
    db: DatabaseConnection,
) -> alloy::AlloyReleaseGovernanceHandle {
    alloy::AlloyReleaseGovernanceHandle(std::sync::Arc::new(
        ModuleControlPlane::new(db).publication(),
    ))
}

#[derive(Debug, Error)]
pub enum RegistryGovernanceError {
    #[error("{0}")]
    Malformed(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("internal registry governance error")]
    Internal(#[source] anyhow::Error),
}

fn malformed_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(RegistryGovernanceError::Malformed(message.into()))
}

fn forbidden_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(RegistryGovernanceError::Forbidden(message.into()))
}

fn not_found_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(RegistryGovernanceError::NotFound(message.into()))
}

fn conflict_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(RegistryGovernanceError::Conflict(message.into()))
}

#[derive(Debug, Clone)]
pub struct RegistryArtifactUpload {
    pub content_type: String,
    pub bytes: bytes::Bytes,
}

/// Host-normalized external prebuilt evidence. The server derives actor and
/// quarantine approver from authenticated authority rather than accepting
/// either principal from the transport payload.
#[derive(Debug, Clone)]
pub struct RegistryExternalPrebuiltStageInput {
    pub artifact_digest: String,
    pub source_evidence: rustok_modules::ModuleExternalSourceEvidence,
    pub provenance_reference: String,
    pub provenance_digest: String,
    pub provenance_policy_revision: String,
    pub quarantine_review_reference: String,
    pub quarantine_policy_revision: String,
    pub idempotency_key: Uuid,
}

/// Host-normalized platform build selection. The controller derives
/// `tenant_id` from the authenticated session, preserving the build owner's
/// tenant-RLS boundary at this cross-owner promotion point.
#[derive(Debug, Clone)]
pub struct RegistryPlatformBuildStageInput {
    pub tenant_id: Uuid,
    pub build_request_id: Uuid,
    pub idempotency_key: Uuid,
}

#[derive(Clone)]
pub struct RegistryGovernanceService {
    db: DatabaseConnection,
    storage: Option<StorageRuntime>,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationQueueResult {
    pub status: RegistryPublishRequestStatusSnapshot,
    pub queued: bool,
    pub validation_job_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationStageReportResult {
    pub status: RegistryPublishRequestStatusSnapshot,
    pub stage: RegistryValidationStageSnapshot,
}

#[derive(Debug, Clone)]
pub struct RegistryRemoteValidationClaim {
    pub claim_id: String,
    pub request_id: String,
    pub slug: String,
    pub version: String,
    pub stage_key: String,
    pub execution_mode: String,
    pub runnable: bool,
    pub requires_manual_confirmation: bool,
    pub allowed_terminal_reason_codes: Vec<String>,
    pub suggested_pass_reason_code: Option<String>,
    pub suggested_failure_reason_code: Option<String>,
    pub suggested_blocked_reason_code: Option<String>,
    pub artifact_download_url: String,
    pub artifact_checksum_sha256: String,
    pub crate_name: String,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishRequestSnapshot {
    pub id: String,
    pub slug: String,
    pub version: String,
    pub status: String,
    pub artifact_origin: String,
    pub requested_by: RegistryPrincipalRef,
    pub publisher: Option<RegistryPrincipalRef>,
    pub approved_by: Option<RegistryPrincipalRef>,
    pub rejected_by: Option<RegistryPrincipalRef>,
    pub rejection_reason: Option<String>,
    pub changes_requested_by: Option<RegistryPrincipalRef>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<String>,
    pub held_by: Option<RegistryPrincipalRef>,
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
    pub publisher: RegistryPrincipalRef,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<RegistryPrincipalRef>,
    pub yanked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleOwnerSnapshot {
    pub owner: RegistryPrincipalRef,
    pub bound_by: RegistryPrincipalRef,
    pub bound_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RegistryGovernanceEventSnapshot {
    pub id: String,
    pub event_type: String,
    pub actor: RegistryPrincipalRef,
    pub publisher: Option<RegistryPrincipalRef>,
    pub payload: RegistryGovernanceEventPayload,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct RegistryGovernanceEventPayload {
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
    pub version: Option<String>,
    pub stage_key: Option<String>,
    pub attempt_number: Option<i32>,
    pub owner_transition: Option<RegistryOwnerTransitionPayload>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryOwnerTransitionPayload {
    pub previous_owner: Option<RegistryPrincipalRef>,
    pub new_owner: Option<RegistryPrincipalRef>,
    pub bound_by: Option<RegistryPrincipalRef>,
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
}

#[derive(Debug, Clone)]
pub struct RegistryGovernanceActionSnapshot {
    pub key: String,
    pub reason_required: bool,
    pub reason_code_required: bool,
    pub reason_codes: Vec<String>,
    pub destructive: bool,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleLifecycleSnapshot {
    pub owner_binding: Option<RegistryModuleOwnerSnapshot>,
    pub latest_request: Option<RegistryPublishRequestSnapshot>,
    pub latest_release: Option<RegistryModuleReleaseSnapshot>,
    pub recent_events: Vec<RegistryGovernanceEventSnapshot>,
    pub follow_up_gates: Vec<RegistryFollowUpGateSnapshot>,
    pub validation_stages: Vec<RegistryValidationStageSnapshot>,
    pub governance_actions: Vec<RegistryGovernanceActionSnapshot>,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishRequestStatusSnapshot {
    pub request: RegistryPublishRequestSnapshot,
    pub authorization: RegistryPublishRequestAuthorizationSnapshot,
    pub effective_publisher_principal: Option<serde_json::Value>,
    pub rejected_retry_allowed: bool,
    pub follow_up_gates: Vec<RegistryFollowUpGateSnapshot>,
    pub validation_stages: Vec<RegistryValidationStageSnapshot>,
    pub approval_override_required: bool,
    pub approval_override_reason_codes: Vec<String>,
    pub approval_override_warning: Option<String>,
    pub governance_actions: Vec<RegistryGovernanceActionSnapshot>,
    pub accepted: bool,
    pub next_action: Option<rustok_modules::ModuleGovernancePublishRequestNextAction>,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishRequestAuthorizationSnapshot {
    pub can_manage: bool,
    pub can_review: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RegistryPublishRequestPermission {
    Manage,
    Review,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishArtifactDownloadSnapshot {
    pub storage_key: String,
    pub content_type: String,
}

pub mod publishing;
pub mod releases;
pub mod validation;

// #[cfg(test)]
// mod tests;

pub use publishing::request_status_label;
pub use releases::release_status_label;
pub use validation::validation_stage_status_label;

impl RegistryGovernanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db, storage: None }
    }

    pub fn with_storage(mut self, storage: StorageRuntime) -> Self {
        self.storage = Some(storage);
        self
    }

    pub(crate) fn release_service(&self) -> SeaOrmModuleGovernanceService {
        ModuleControlPlane::new(self.db.clone()).release()
    }

    pub(crate) fn publication_service(&self) -> SeaOrmModuleGovernanceService {
        ModuleControlPlane::new(self.db.clone()).publication()
    }

    fn require_storage(&self) -> anyhow::Result<&StorageRuntime> {
        self.storage
            .as_ref()
            .ok_or_else(|| anyhow!("StorageRuntime is required for registry artifact operations"))
    }

    async fn store_registry_artifact(
        &self,
        artifact_storage_key: &str,
        artifact: &RegistryArtifactUpload,
        checksum_sha256: &str,
    ) -> anyhow::Result<()> {
        let storage = self.require_storage()?;
        let mut options = storage.put_options(&artifact.content_type);
        options.mode = PutMode::Create;
        let created = match storage
            .objects
            .put_opts(
                &Path::from(artifact_storage_key),
                artifact.bytes.clone().into(),
                options,
            )
            .await
        {
            Ok(_) => true,
            Err(
                object_store::Error::AlreadyExists { .. }
                | object_store::Error::Precondition { .. },
            ) => false,
            Err(error) => {
                return Err(anyhow!(error).context(format!(
                    "failed to store registry artifact at '{artifact_storage_key}'"
                )));
            }
        };
        if created {
            return Ok(());
        }

        let existing = storage
            .objects
            .get(&Path::from(artifact_storage_key))
            .await
            .with_context(|| {
                format!("failed to read existing registry artifact at '{artifact_storage_key}'")
            })?
            .bytes()
            .await
            .with_context(|| {
                format!(
                    "failed to read existing registry artifact body at '{artifact_storage_key}'"
                )
            })?;
        if existing.len() != artifact.bytes.len()
            || hex::encode(Sha256::digest(existing.as_ref())) != checksum_sha256
        {
            return Err(conflict_error(format!(
                "immutable registry artifact slot '{artifact_storage_key}' contains different bytes"
            )));
        }

        Ok(())
    }
}

fn follow_up_gate_detail(gate: &str) -> &'static str {
    match gate {
        "compile_smoke" => "Compile smoke awaits exact platform build-worker validation evidence.",
        "targeted_tests" => "Targeted tests await exact platform build-worker validation evidence.",
        "security_policy_review" => {
            "Security and policy review await exact origin-specific owner evidence."
        }
        _ => "External follow-up gate is still pending.",
    }
}

pub(crate) fn principal_from_json(value: &serde_json::Value) -> RegistryPrincipalRef {
    RegistryPrincipalRef::from_json_value(value)
}

pub(crate) fn optional_principal_from_json(
    value: &Option<serde_json::Value>,
) -> Option<RegistryPrincipalRef> {
    value.as_ref().map(principal_from_json)
}

pub(crate) fn principal_display_label(value: &serde_json::Value) -> String {
    principal_from_json(value).label().to_string()
}

pub(crate) fn optional_principal_display_label(
    value: &Option<serde_json::Value>,
) -> Option<String> {
    optional_principal_from_json(value).map(|principal| principal.label().to_string())
}

fn principal_matches_ref(value: &serde_json::Value, principal: &RegistryPrincipalRef) -> bool {
    let left = principal_from_json(value);
    if left.is_user() && principal.is_user() {
        return left.user_id() == principal.user_id();
    }
    left.subject == principal.subject || left.persisted_label() == principal.persisted_label()
}

fn optional_principal_matches_ref(
    value: &Option<serde_json::Value>,
    principal: &RegistryPrincipalRef,
) -> bool {
    value
        .as_ref()
        .is_some_and(|persisted| principal_matches_ref(persisted, principal))
}

fn authority_actor(authority: &RegistryAuthority) -> &str {
    authority.principal.label()
}

fn authority_can_create_publish_request(
    authority: &RegistryAuthority,
    owner: Option<&registry_module_owner::Model>,
) -> bool {
    authority.can_manage_modules
        || owner.is_some_and(|owner| {
            principal_matches_ref(&owner.owner_principal, &authority.principal)
        })
        || owner.is_none() && authority.principal.is_user()
}

fn authority_can_manage_release(
    authority: &RegistryAuthority,
    release: &registry_module_release::Model,
    owner: Option<&registry_module_owner::Model>,
) -> bool {
    authority.can_manage_modules
        || principal_matches_ref(&release.publisher, &authority.principal)
        || owner.is_some_and(|owner| {
            principal_matches_ref(&owner.owner_principal, &authority.principal)
        })
}

fn authority_can_transfer_registry_owner(
    authority: &RegistryAuthority,
    binding: &RegistryModuleOwnerSnapshot,
) -> bool {
    authority.can_manage_modules || binding.owner == authority.principal
}

pub(crate) fn normalize_reason_code(
    reason_code: &str,
    allowed: &[&str],
    action_label: &str,
) -> anyhow::Result<String> {
    let normalized = reason_code.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(malformed_error(format!(
            "{action_label} requires a non-empty reason_code"
        )));
    }
    if !allowed
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&normalized))
    {
        return Err(malformed_error(format!(
            "{} reason_code '{}' is not supported; expected one of {}",
            action_label,
            reason_code.trim(),
            allowed.join(", ")
        )));
    }
    Ok(normalized)
}

pub(crate) fn normalize_required_reason(
    reason: &str,
    action_label: &str,
) -> anyhow::Result<String> {
    let normalized = reason.trim();
    if normalized.is_empty() {
        return Err(malformed_error(format!(
            "{action_label} requires a non-empty reason"
        )));
    }
    Ok(normalized.to_string())
}
