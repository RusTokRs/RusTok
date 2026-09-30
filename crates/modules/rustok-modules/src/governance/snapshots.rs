//! Governance lifecycle snapshots, policy, and audit models.

use serde::{Deserialize, Serialize};

/// Canonical, transport-neutral registry moderation policy exposed with the
/// lifecycle snapshot. Hosts render these facts but do not reconstruct them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceModerationPolicy {
    pub mode: String,
    pub live_publish_supported: bool,
    pub live_governance_supported: bool,
    pub manual_review_required: bool,
    pub restriction_reason_code: Option<String>,
    pub restriction_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceOwnerSnapshot {
    pub owner_principal: serde_json::Value,
    pub bound_by_principal: serde_json::Value,
    pub bound_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceRequestSnapshot {
    pub id: String,
    /// Monotonically increasing aggregate revision. Every owner mutation of
    /// this request advances it in the same transaction as the state change.
    pub revision: i64,
    pub slug: String,
    pub version: String,
    pub status: String,
    pub artifact_origin: String,
    pub requested_by_principal: serde_json::Value,
    pub publisher_principal: Option<serde_json::Value>,
    pub approved_by_principal: Option<serde_json::Value>,
    pub rejected_by_principal: Option<serde_json::Value>,
    pub rejection_reason: Option<String>,
    pub changes_requested_by_principal: Option<serde_json::Value>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<String>,
    pub held_by_principal: Option<serde_json::Value>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceReleaseSnapshot {
    pub version: String,
    pub status: String,
    pub publisher_principal: serde_json::Value,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by_principal: Option<serde_json::Value>,
    pub yanked_at: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceEventPayload {
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
    pub version: Option<String>,
    pub stage_key: Option<String>,
    pub attempt_number: Option<i32>,
    pub owner_transition: Option<ModuleGovernanceOwnerTransition>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub mode: Option<String>,
    pub automated_checks: Vec<ModuleGovernanceAutomatedCheck>,
}

/// One immutable result produced by an automated validation check.
///
/// The modules owner validates and normalizes this evidence before it becomes
/// part of a durable governance event or a browser-safe lifecycle projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceAutomatedCheck {
    pub key: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceOwnerTransition {
    pub previous_owner_principal: Option<serde_json::Value>,
    pub new_owner_principal: Option<serde_json::Value>,
    pub bound_by_principal: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceEventSnapshot {
    pub id: String,
    pub event_type: String,
    pub actor_principal: serde_json::Value,
    pub publisher_principal: Option<serde_json::Value>,
    pub payload: ModuleGovernanceEventPayload,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceGateSnapshot {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceValidationStageSnapshot {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceAction {
    pub key: String,
    pub reason_required: bool,
    pub reason_code_required: bool,
    pub reason_codes: Vec<String>,
    pub destructive: bool,
}

/// Authenticated host facts used solely to constrain the actions exposed by a
/// request-scoped governance projection. The owner still reloads the durable
/// request and owner binding before deriving the action list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceActorContext {
    pub principal: serde_json::Value,
    pub can_manage_modules: bool,
}

/// Exact durable permissions for an authenticated governance actor. Hosts use
/// these facts to authorize mutations without reading registry request or
/// owner-binding persistence models.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceRequestAuthorizationSnapshot {
    pub can_manage: bool,
    pub can_review: bool,
}

/// Owner-derived next operation for an exact publish request. The owner owns
/// the lifecycle decision; transports map this semantic operation onto their
/// own routes and presentation text without reinterpreting durable state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleGovernancePublishRequestNextAction {
    UploadArtifact,
    UploadArtifactRevision,
    TriggerValidation,
    PollStatus,
    StageExternalPrebuilt,
    StagePlatformBuild,
    StageAlloyRelease,
    FinalizePublication,
    Resume,
    RetryRejected,
}

/// Complete owner-derived public status projection for one exact publish
/// request. This is deliberately request-scoped and contains all lifecycle
/// interpretation required by status transports, including override guidance
/// and the semantic next operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernancePublishRequestStatusSnapshot {
    pub request: ModuleGovernanceRequestSnapshot,
    pub authorization: ModuleGovernanceRequestAuthorizationSnapshot,
    pub effective_publisher_principal: Option<serde_json::Value>,
    pub rejected_retry_allowed: bool,
    pub follow_up_gates: Vec<ModuleGovernanceGateSnapshot>,
    pub validation_stages: Vec<ModuleGovernanceValidationStageSnapshot>,
    pub approval_override_required: bool,
    pub approval_override_reason_codes: Vec<String>,
    pub approval_override_warning: Option<String>,
    pub governance_actions: Vec<ModuleGovernanceAction>,
    pub accepted: bool,
    pub next_action: Option<ModuleGovernancePublishRequestNextAction>,
}

/// Host-only delivery facts for an uploaded publish artifact. This narrow
/// projection deliberately stays separate from the public status snapshot so
/// storage topology never becomes a status transport field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernancePublishArtifactDownloadSnapshot {
    pub storage_key: String,
    pub content_type: String,
}

/// Complete registry lifecycle projection for one module slug. All policy and
/// stage metadata is owner-derived so transports cannot drift.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernanceLifecycleSnapshot {
    pub moderation_policy: ModuleGovernanceModerationPolicy,
    pub owner_binding: Option<ModuleGovernanceOwnerSnapshot>,
    pub latest_request: Option<ModuleGovernanceRequestSnapshot>,
    pub latest_release: Option<ModuleGovernanceReleaseSnapshot>,
    pub recent_events: Vec<ModuleGovernanceEventSnapshot>,
    pub follow_up_gates: Vec<ModuleGovernanceGateSnapshot>,
    pub validation_stages: Vec<ModuleGovernanceValidationStageSnapshot>,
    pub governance_actions: Vec<ModuleGovernanceAction>,
}

