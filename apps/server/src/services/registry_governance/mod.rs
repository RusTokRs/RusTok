#![allow(clippy::too_many_arguments, clippy::unnecessary_lazy_evaluations)]

use anyhow::{Context, anyhow};
use object_store::path::Path;
use rustok_api::{PLATFORM_FALLBACK_LOCALE, build_locale_candidates, locale_tags_match};
use rustok_modules::{ModuleControlPlane, SeaOrmModuleGovernanceService};
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone, StorageRuntime};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::models::registry_governance_event::{self, Entity as RegistryGovernanceEventEntity};
use crate::models::registry_module_owner::{self, Entity as RegistryModuleOwnerEntity};
use crate::models::registry_module_release::{
    self, Entity as RegistryModuleReleaseEntity, RegistryModuleReleaseStatus,
};
use crate::models::registry_module_release_translation::{
    self as registry_module_release_translation, Entity as RegistryModuleReleaseTranslationEntity,
};
use crate::models::registry_publish_request::{
    self, Entity as RegistryPublishRequestEntity, RegistryPublishRequestStatus,
};
use crate::models::registry_validation_stage::{
    self, Entity as RegistryValidationStageEntity, RegistryValidationStageStatus,
};
use crate::modules::{CatalogManifestModule, CatalogModuleVersion};
use crate::services::marketplace_catalog::{RegistryPublishArtifactOrigin, RegistryPublishRequest};
use crate::services::registry_principal::{RegistryAuthority, RegistryPrincipalRef};
use thiserror::Error;

pub use rustok_modules::MODULE_PUBLISH_ARTIFACT_MAX_BYTES;
const REGISTRY_VALIDATION_FOLLOW_UP_GATES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];
const PLATFORM_BUILT_VALIDATION_FOLLOW_UP_GATES: &[&str] = &["compile_smoke", "targeted_tests"];
const EXTERNAL_PREBUILT_VALIDATION_FOLLOW_UP_GATES: &[&str] = &["security_policy_review"];
const ALLOY_AUTHORED_VALIDATION_FOLLOW_UP_GATES: &[&str] = &["security_policy_review"];
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
    pub request: registry_publish_request::Model,
    pub queued: bool,
    pub validation_job_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryValidationStageMutationResult {
    pub request: registry_publish_request::Model,
    pub stage: registry_validation_stage::Model,
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
pub struct RegistryPublishRequestFollowUpSnapshot {
    pub follow_up_gates: Vec<RegistryFollowUpGateSnapshot>,
    pub validation_stages: Vec<RegistryValidationStageSnapshot>,
    pub approval_override_required: bool,
    pub governance_actions: Vec<RegistryGovernanceActionSnapshot>,
}

#[derive(Debug, Clone)]
struct RegistryLocalizedMetadata {
    name: String,
    description: String,
}

pub mod publishing;
pub mod releases;
pub mod validation;

// #[cfg(test)]
// mod tests;

pub use publishing::request_status_label;
pub use releases::release_status_label;
pub use validation::validation_stage_status_label;

pub(crate) use publishing::{
    publish_request_governance_actions,
    publish_request_governance_actions_for_authority,
};
pub(crate) use validation::{
    compare_semver_desc, derive_follow_up_gate_snapshots, derive_validation_stage_snapshots,
    validation_stage_details_value,
};

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
        request: &registry_publish_request::Model,
        artifact: &RegistryArtifactUpload,
    ) -> anyhow::Result<StoredRegistryArtifact> {
        let artifact_storage_key = registry_artifact_storage_key(&request.id, request.created_at)?;
        self.require_storage()?
            .objects
            .put_opts(
                &Path::from(artifact_storage_key.as_str()),
                artifact.bytes.clone().into(),
                self.require_storage()?.put_options(&artifact.content_type),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to store registry artifact for request '{}' at '{}'",
                    request.id, artifact_storage_key
                )
            })?;

        Ok(StoredRegistryArtifact {
            artifact_storage_key,
            artifact_size: artifact.bytes.len() as i64,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StoredRegistryArtifact {
    artifact_storage_key: String,
    artifact_size: i64,
}

fn registry_artifact_storage_key(
    request_id: &str,
    created_at: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<String> {
    let digest = Sha256::digest(request_id.as_bytes());
    let mut identity = [0_u8; 16];
    identity.copy_from_slice(&digest[..16]);
    ObjectKey::chronological(
        "registry-artifact",
        ObjectZone::Objects,
        ObjectScope::Platform,
        created_at,
        Uuid::from_bytes(identity),
        "crate",
    )
    .map(|key| key.to_string())
    .map_err(Into::into)
}

fn registry_artifact_download_path(request_id: &str) -> String {
    format!("/v2/catalog/publish/{request_id}/artifact/download")
}

fn validation_follow_up_gates_for_origin(artifact_origin: &str) -> &'static [&'static str] {
    match artifact_origin {
        "platform_built" => PLATFORM_BUILT_VALIDATION_FOLLOW_UP_GATES,
        "external_prebuilt" => EXTERNAL_PREBUILT_VALIDATION_FOLLOW_UP_GATES,
        "alloy_authored" => ALLOY_AUTHORED_VALIDATION_FOLLOW_UP_GATES,
        _ => &[],
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

async fn load_release_translation_rows<C>(
    db: &C,
    release_id: &str,
) -> anyhow::Result<Vec<registry_module_release_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(RegistryModuleReleaseTranslationEntity::find()
        .filter(registry_module_release_translation::Column::ReleaseId.eq(release_id))
        .order_by_asc(registry_module_release_translation::Column::Locale)
        .all(db)
        .await?)
}

fn resolve_registry_metadata<T, FName, FDescription, FLocale>(
    translations: &[T],
    preferred_locale: Option<&str>,
    fallback_locale: Option<&str>,
    locale_of: FLocale,
    name_of: FName,
    description_of: FDescription,
) -> Option<RegistryLocalizedMetadata>
where
    FLocale: Fn(&T) -> &str,
    FName: Fn(&T) -> &str,
    FDescription: Fn(&T) -> &str,
{
    let candidates = build_locale_candidates(
        [
            preferred_locale,
            fallback_locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        ],
        true,
    );

    for candidate in candidates {
        if let Some(translation) = translations
            .iter()
            .find(|translation| locale_tags_match(locale_of(translation), &candidate))
        {
            return Some(RegistryLocalizedMetadata {
                name: name_of(translation).to_string(),
                description: description_of(translation).to_string(),
            });
        }
    }

    translations
        .first()
        .map(|translation| RegistryLocalizedMetadata {
            name: name_of(translation).to_string(),
            description: description_of(translation).to_string(),
        })
}

async fn load_release_metadata<C>(
    db: &C,
    release_id: &str,
    preferred_locale: Option<&str>,
    fallback_locale: Option<&str>,
) -> anyhow::Result<RegistryLocalizedMetadata>
where
    C: ConnectionTrait,
{
    let translations = load_release_translation_rows(db, release_id).await?;
    resolve_registry_metadata(
        &translations,
        preferred_locale,
        fallback_locale,
        |translation| translation.locale.as_str(),
        |translation| translation.name.as_str(),
        |translation| translation.description.as_str(),
    )
    .ok_or_else(|| anyhow!("Registry release '{release_id}' is missing metadata translations"))
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

fn governance_event_payload(details: &serde_json::Value) -> RegistryGovernanceEventPayload {
    let warnings = details
        .get("warnings")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(ToString::to_string))
        .collect();
    let errors = details
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(ToString::to_string))
        .collect();
    let attempt_number = details
        .get("attempt_number")
        .and_then(serde_json::Value::as_i64)
        .map(|value| value as i32);

    let owner_transition = details
        .get("owner_transition")
        .and_then(serde_json::Value::as_object)
        .map(|transition| RegistryOwnerTransitionPayload {
            previous_owner: transition
                .get("previous_owner")
                .map(RegistryPrincipalRef::from_json_value),
            new_owner: transition
                .get("new_owner")
                .map(RegistryPrincipalRef::from_json_value),
            bound_by: transition
                .get("bound_by")
                .map(RegistryPrincipalRef::from_json_value),
        });

    RegistryGovernanceEventPayload {
        reason: details
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        reason_code: details
            .get("reason_code")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        detail: details
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        version: details
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        stage_key: details
            .get("stage_key")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        attempt_number,
        owner_transition,
        warnings,
        errors,
        mode: details
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
    }
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

fn authority_can_manage_publish_request(
    authority: &RegistryAuthority,
    request: &registry_publish_request::Model,
    owner: Option<&registry_module_owner::Model>,
) -> bool {
    let principal_matches_request =
        principal_matches_ref(&request.requested_by, &authority.principal)
            || optional_principal_matches_ref(&request.publisher_principal, &authority.principal);
    let principal_matches_owner = owner
        .is_some_and(|owner| principal_matches_ref(&owner.owner_principal, &authority.principal));

    authority.can_manage_modules
        || principal_matches_owner
        || (owner.is_none() && principal_matches_request)
}

fn authority_can_review_publish_request(
    authority: &RegistryAuthority,
    owner: Option<&registry_module_owner::Model>,
) -> bool {
    authority.can_manage_modules
        || owner.is_some_and(|owner| {
            principal_matches_ref(&owner.owner_principal, &authority.principal)
        })
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
    binding: &registry_module_owner::Model,
) -> bool {
    authority.can_manage_modules
        || principal_matches_ref(&binding.owner_principal, &authority.principal)
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
