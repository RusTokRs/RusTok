//! Governance commands, outcomes, publication sources, and artifact types.

use serde::{Deserialize, Serialize};

use super::*;

/// Authenticated host input for a durable release-yank transition. The owner
/// derives authorization from the durable release publisher and owner binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleReleaseYankCommand {
    pub slug: String,
    pub version: String,
    pub reason: String,
    pub reason_code: String,
    /// Platform-scoped evidence for one immutable release-yank operation.
    /// Registry releases are global aggregates, so tenant scope is absent.
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    /// An authenticated host fact which the owner combines with durable
    /// publisher and owner identities.
    pub actor_can_manage_modules: bool,
}

/// Owner-issued facts from one completed release-yank transition. The result
/// intentionally exposes only the release fields a transport needs to build
/// its response and never a persistence model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleReleaseYankResult {
    pub request_id: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleOwnerTransferCommand {
    pub slug: String,
    pub new_owner_principal: serde_json::Value,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    /// An authenticated host fact. The owner still reloads and locks the
    /// durable binding before it authorizes the transfer.
    pub actor_can_manage_modules: bool,
    pub reason: String,
    pub reason_code: String,
}

/// Authenticated host input for a terminal publish-request rejection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestRejectCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

/// Authenticated host input for returning an approved publish request to the
/// publisher with a durable review reason.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestChangesCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

/// Authenticated host input for pausing an eligible publish request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestHoldCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestResumeCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

/// Evidence supplied by the host review adapter when an approved publication
/// overrides incomplete validation stages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishApprovalOverride {
    pub reason: String,
    pub reason_code: String,
    pub validation_stages: serde_json::Value,
}

/// Authenticated host input for the atomic publication write-set. The owner
/// loads the durable request and translations inside its transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestPublicationCommand {
    pub request_id: String,
    pub expected_revision: i64,
    /// Authenticated platform-scoped command identity for exactly-once final
    /// publication and durable audit correlation.
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    pub publisher_principal: serde_json::Value,
    pub allow_owner_rebind: bool,
    pub approval_override: Option<ModulePublishApprovalOverride>,
}

/// Authenticated host input for a manual validation-stage transition or a new
/// validation attempt. The owner canonicalizes the stage, status, and reason
/// before state-machine enforcement and audit persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationStageReportCommand {
    pub request_id: String,
    /// Revision observed from the authorized publish-request snapshot before
    /// the manual stage transition.
    pub expected_revision: i64,
    /// Authenticated platform-scoped evidence for exactly-once manual review.
    /// Registry validation stages belong to the global publish-request
    /// aggregate, so tenant scope is deliberately absent.
    pub context: ModuleCommandContext,
    pub stage_key: String,
    pub status: String,
    pub actor_principal: serde_json::Value,
    pub reason_code: Option<String>,
    pub requeue: bool,
}

/// Immutable terminal outcome selected by an authenticated remote validation
/// runner. The owner validates the live claim with a conditional update.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleRemoteValidationTerminalOutcome {
    Passed,
    Failed,
}

/// Host-authenticated request to renew a remote runner lease.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleRemoteValidationHeartbeatCommand {
    pub claim_id: String,
    pub runner_id: String,
    pub lease_ttl_ms: u64,
}

/// Host-authenticated request to complete a remote runner lease.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleRemoteValidationTerminalCommand {
    pub claim_id: String,
    pub runner_id: String,
    /// Revision issued with the remote claim. A terminal result cannot apply
    /// after another request-level transition has superseded that lease.
    pub expected_request_revision: i64,
    pub outcome: ModuleRemoteValidationTerminalOutcome,
    /// Untrusted runner output. The owner never persists or emits this value.
    pub detail: Option<String>,
    pub reason_code: Option<String>,
}

/// Canonical transport-neutral outcome of a remote validation lease
/// transition. The registry owner, rather than a host database read, is the
/// source of the reported stage state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRemoteValidationStageTransition {
    pub status: String,
}

/// Host-authenticated request for one eligible remote validation lease. The
/// owner selects and claims a stage atomically; the host owns runner transport
/// authentication and artifact download URL construction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleRemoteValidationClaimCommand {
    pub runner_id: String,
    pub supported_stages: Vec<String>,
    pub lease_ttl_ms: u64,
}

/// Durable information a remote runner needs after the owner has issued a
/// validation lease. It intentionally contains no host URL or credentials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleRemoteValidationClaim {
    pub claim_id: String,
    pub request_id: String,
    /// Aggregate revision after the claim transitioned its stage to running.
    /// The runner returns this value with its terminal outcome.
    pub request_revision: i64,
    pub slug: String,
    pub version: String,
    pub stage_key: String,
    pub execution_mode: String,
    pub requires_manual_confirmation: bool,
    pub allowed_terminal_reason_codes: Vec<String>,
    pub suggested_pass_reason_code: String,
    pub suggested_failure_reason_code: String,
    pub suggested_blocked_reason_code: String,
    pub artifact_checksum_sha256: String,
    pub crate_name: String,
}

/// Owner-issued observability facts for remote validation work that is still
/// leased by a runner. Terminal or locally-owned stages are deliberately not
/// included in either count.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRemoteValidationRunnerSnapshot {
    pub active_claims: u64,
    pub expired_claims: u64,
}

/// Host-authorized request to enqueue durable automated validation. The owner
/// owns request/job state and audit facts; a host worker executes the bundle
/// checks only after this transaction commits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobEnqueueCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    pub allow_rejected_retry: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobEnqueueResult {
    pub request_id: String,
    pub request_status: String,
    pub queued: bool,
    pub validation_job_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobClaimCommand {
    pub validation_job_id: String,
    pub actor_principal: serde_json::Value,
}

/// Immutable artifact-delivery facts leased to one validation worker.
///
/// The worker must fetch only `artifact_storage_key` and verify the exact
/// checksum and size before parsing. This keeps delivery independent from a
/// server-local publish-request model and prevents a later request read from
/// silently changing the bytes that a claimed job validates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobWorkItem {
    pub validation_job_id: String,
    pub request_id: String,
    /// Publish-request revision observed when this exact job was claimed.
    /// The worker must return it so a delayed result cannot overwrite a later
    /// moderator or publisher transition.
    pub expected_request_revision: i64,
    pub slug: String,
    pub version: String,
    pub crate_name: String,
    pub artifact_origin: ModulePublicationArtifactOrigin,
    /// Immutable descriptor selected from the Alloy owner-stage receipt. It is
    /// present only for Alloy-authored releases and lets the isolated worker
    /// validate the uploaded canonical workspace without rereading mutable
    /// Alloy state.
    pub alloy_descriptor: Option<crate::ModuleArtifactDescriptor>,
    pub artifact_storage_key: String,
    pub artifact_checksum_sha256: String,
    pub artifact_size: u64,
    pub artifact_content_type: String,
    pub existing_warnings: Vec<String>,
    /// Immutable request metadata used to validate the uploaded bundle. The
    /// worker does not query a mutable host request model after claiming work.
    pub contract: ModulePublishValidationContract,
}

/// Canonical publish-request facts that an uploaded registry bundle must
/// match. This owner contract is serializable so the same validation logic can
/// run in an isolated delivery worker without depending on `apps/server`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishValidationContract {
    pub slug: String,
    pub version: String,
    pub crate_name: String,
    pub module_name: String,
    pub module_description: String,
    pub ownership: String,
    pub trust_level: String,
    pub license: String,
    pub entry_type: Option<String>,
    pub marketplace_category: Option<String>,
    pub marketplace_tags: Vec<String>,
    pub admin_ui_crate_name: Option<String>,
    pub storefront_ui_crate_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobClaimResult {
    pub request_id: String,
    pub should_run: bool,
    /// Present only when this invocation changed the job from `queued` to
    /// `running`. Terminal and already-claimed redeliveries expose no work.
    pub work_item: Option<ModuleValidationJobWorkItem>,
}

/// Immutable result emitted by the host worker after it has executed registry
/// bundle checks. The owner applies the result, request transition, follow-up
/// validation stages, terminal job state, and audit facts in one transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleValidationJobResultOutcome {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobResultCommand {
    pub validation_job_id: String,
    /// Immutable request revision carried from the worker lease. Exact
    /// terminal redelivery remains idempotent; a running job must match it
    /// before the owner accepts its result.
    pub expected_request_revision: i64,
    pub actor_principal: serde_json::Value,
    pub outcome: ModuleValidationJobResultOutcome,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub automated_checks: Vec<ModuleGovernanceAutomatedCheck>,
}

/// Immutable retry observation emitted by a running host worker. It keeps
/// retry telemetry owner-owned without making the worker a governance writer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobRetryCommand {
    pub validation_job_id: String,
    pub actor_principal: serde_json::Value,
    pub attempt: u32,
    pub retry_after_seconds: Option<u64>,
    /// Untrusted delivery error used only to validate that the worker reported
    /// a failure. Durable audit data uses a stable owner-owned message.
    pub error: String,
}

/// Host-authorized immutable metadata for a new registry publish request. The
/// owner creates the request, its default-locale translation, and audit fact
/// together so a request is never observable without its metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestCreateCommand {
    pub slug: String,
    pub version: String,
    pub crate_name: String,
    pub default_locale: String,
    pub ownership: String,
    pub trust_level: String,
    pub license: String,
    pub entry_type: Option<String>,
    pub artifact_origin: ModulePublicationArtifactOrigin,
    pub marketplace: serde_json::Value,
    pub ui_packages: serde_json::Value,
    pub name: String,
    pub description: String,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    /// An authenticated host fact. The owner combines it with the durable
    /// owner binding before accepting a new request.
    pub actor_can_manage_modules: bool,
}
