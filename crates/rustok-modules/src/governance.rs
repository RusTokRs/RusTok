//! Owner contracts for registry governance transitions.

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Stable reason-code vocabulary for a release yank.
pub const REGISTRY_YANK_REASON_CODES: &[&str] = &[
    "security",
    "legal",
    "malware",
    "critical_regression",
    "rollback",
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

pub const REGISTRY_REJECT_REASON_CODES: &[&str] = &[
    "policy_mismatch",
    "quality_gate_failed",
    "ownership_mismatch",
    "security_risk",
    "legal",
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

pub const REGISTRY_APPROVE_OVERRIDE_REASON_CODES: &[&str] = &[
    "manual_review_complete",
    "trusted_first_party",
    "expedited_release",
    "governance_override",
    "other",
];

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

const REMOTE_VALIDATION_FOLLOW_UP_STAGES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];
const MAX_REMOTE_VALIDATION_CLAIM_CANDIDATES: u64 = 128;

/// Authenticated host input for a durable release-yank transition.
/// Authorization is evaluated by the host authority adapter before this owner
/// command reaches registry persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleReleaseYankCommand {
    pub slug: String,
    pub version: String,
    pub reason: String,
    pub reason_code: String,
    pub actor_principal: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleOwnerTransferCommand {
    pub slug: String,
    pub new_owner_principal: serde_json::Value,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

/// Authenticated host input for an initial publisher binding or an authorized
/// rebind. `allow_rebind` must only be set after the host authority adapter
/// has approved replacement of an existing owner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleOwnerBindCommand {
    pub slug: String,
    pub owner_principal: serde_json::Value,
    pub actor_principal: serde_json::Value,
    pub allow_rebind: bool,
}

/// Authenticated host input for a terminal publish-request rejection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestRejectCommand {
    pub request_id: String,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

/// Authenticated host input for returning an approved publish request to the
/// publisher with a durable review reason.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestChangesCommand {
    pub request_id: String,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

/// Authenticated host input for pausing an eligible publish request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestHoldCommand {
    pub request_id: String,
    pub actor_principal: serde_json::Value,
    pub reason: String,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishRequestResumeCommand {
    pub request_id: String,
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
    pub actor_principal: serde_json::Value,
    pub publisher_principal: serde_json::Value,
    pub allow_owner_rebind: bool,
    pub approval_override: Option<ModulePublishApprovalOverride>,
}

/// Authenticated host input for a manual validation-stage transition or a new
/// validation attempt. Authorization remains a host concern; state-machine
/// enforcement and audit persistence are owner concerns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationStageReportCommand {
    pub request_id: String,
    pub stage_key: String,
    pub status: String,
    pub actor_principal: serde_json::Value,
    pub detail: String,
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
    pub outcome: ModuleRemoteValidationTerminalOutcome,
    pub detail: Option<String>,
    pub reason_code: Option<String>,
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

/// Host-authorized request to enqueue durable automated validation. The owner
/// owns request/job state and audit facts; a host worker executes the bundle
/// checks only after this transaction commits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobEnqueueCommand {
    pub request_id: String,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobClaimResult {
    pub request_id: String,
    pub should_run: bool,
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
    pub actor_principal: serde_json::Value,
    pub outcome: ModuleValidationJobResultOutcome,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub automated_checks: serde_json::Value,
}

/// Immutable retry observation emitted by a running host worker. It keeps
/// retry telemetry owner-owned without making the worker a governance writer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleValidationJobRetryCommand {
    pub validation_job_id: String,
    pub actor_principal: serde_json::Value,
    pub attempt: u32,
    pub retry_after_seconds: Option<u64>,
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
    pub marketplace: serde_json::Value,
    pub ui_packages: serde_json::Value,
    pub name: String,
    pub description: String,
    pub warnings: Vec<String>,
    pub actor_principal: serde_json::Value,
}

/// Metadata of bytes the host has durably stored before asking the owner to
/// attach that artifact to a publish request. Object storage remains a host
/// adapter; request state, retry cleanup, and audit facts remain owner-owned.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishArtifactAttachCommand {
    pub request_id: String,
    pub actor_principal: serde_json::Value,
    pub artifact_storage_key: String,
    pub checksum_sha256: String,
    pub artifact_size: i64,
    pub content_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishArtifactAttachResult {
    pub request_id: String,
    pub previous_storage_key: Option<String>,
    pub reuploaded_after_changes_requested: bool,
}

impl ModuleOwnerTransferCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.slug.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.new_owner_principal.is_null()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidOwnerTransferCommand);
        }
        if !REGISTRY_OWNER_TRANSFER_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(ModuleGovernanceError::InvalidOwnerTransferReasonCode(
                self.reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModuleOwnerBindCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.slug.trim().is_empty()
            || self.owner_principal.is_null()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidOwnerBindCommand);
        }
        Ok(())
    }
}

impl ModulePublishRequestRejectCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestRejectCommand);
        }
        if !REGISTRY_REJECT_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(
                ModuleGovernanceError::InvalidPublishRequestRejectReasonCode(
                    self.reason_code.clone(),
                ),
            );
        }
        Ok(())
    }
}

impl ModulePublishRequestChangesCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestChangesCommand);
        }
        if !REGISTRY_REQUEST_CHANGES_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(
                ModuleGovernanceError::InvalidPublishRequestChangesReasonCode(
                    self.reason_code.clone(),
                ),
            );
        }
        Ok(())
    }
}

impl ModulePublishRequestHoldCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestHoldCommand);
        }
        if !REGISTRY_HOLD_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(ModuleGovernanceError::InvalidPublishRequestHoldReasonCode(
                self.reason_code.clone(),
            ));
        }
        Ok(())
    }
}

impl ModulePublishRequestResumeCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestResumeCommand);
        }
        if !REGISTRY_RESUME_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(
                ModuleGovernanceError::InvalidPublishRequestResumeReasonCode(
                    self.reason_code.clone(),
                ),
            );
        }
        Ok(())
    }
}

impl ModulePublishRequestPublicationCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || !self.actor_principal.is_object()
            || !self.publisher_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestPublicationCommand);
        }
        if let Some(override_evidence) = &self.approval_override {
            if override_evidence.reason.trim().is_empty()
                || !override_evidence.validation_stages.is_array()
            {
                return Err(ModuleGovernanceError::InvalidPublishApprovalOverride);
            }
            if !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .contains(&override_evidence.reason_code.as_str())
            {
                return Err(
                    ModuleGovernanceError::InvalidPublishApprovalOverrideReasonCode(
                        override_evidence.reason_code.clone(),
                    ),
                );
            }
        }
        Ok(())
    }
}

impl ModuleValidationStageReportCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.stage_key.trim().is_empty()
            || self.detail.trim().is_empty()
            || !self.actor_principal.is_object()
            || !matches!(
                self.status.as_str(),
                "queued" | "running" | "passed" | "failed" | "blocked"
            )
        {
            return Err(ModuleGovernanceError::InvalidValidationStageReportCommand);
        }
        if self.requeue != (self.status == "queued") {
            return Err(ModuleGovernanceError::InvalidValidationStageRequeue);
        }
        if let Some(reason_code) = &self.reason_code {
            if !REGISTRY_VALIDATION_STAGE_REASON_CODES.contains(&reason_code.as_str()) {
                return Err(ModuleGovernanceError::InvalidValidationStageReasonCode(
                    reason_code.clone(),
                ));
            }
        }
        Ok(())
    }
}

impl ModuleRemoteValidationHeartbeatCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.claim_id.trim().is_empty() || self.runner_id.trim().is_empty() {
            return Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand);
        }
        Ok(())
    }
}

impl ModuleRemoteValidationTerminalCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.claim_id.trim().is_empty() || self.runner_id.trim().is_empty() {
            return Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand);
        }
        if let Some(reason_code) = &self.reason_code {
            if !REGISTRY_VALIDATION_STAGE_REASON_CODES.contains(&reason_code.as_str()) {
                return Err(ModuleGovernanceError::InvalidValidationStageReasonCode(
                    reason_code.clone(),
                ));
            }
        }
        Ok(())
    }
}

impl ModuleRemoteValidationClaimCommand {
    fn normalized_supported_stages(&self) -> Result<Vec<String>, ModuleGovernanceError> {
        if self.runner_id.trim().is_empty() {
            return Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand);
        }
        let mut normalized = Vec::new();
        for stage in &self.supported_stages {
            let candidate = stage.trim().to_ascii_lowercase();
            if candidate.is_empty() {
                return Err(ModuleGovernanceError::InvalidRemoteValidationClaimStage(
                    stage.clone(),
                ));
            }
            if !REMOTE_VALIDATION_FOLLOW_UP_STAGES.contains(&candidate.as_str()) {
                return Err(ModuleGovernanceError::InvalidRemoteValidationClaimStage(
                    stage.clone(),
                ));
            }
            if !normalized.contains(&candidate) {
                normalized.push(candidate);
            }
        }
        Ok(normalized)
    }
}

impl ModuleValidationJobEnqueueCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty() || !self.actor_principal.is_object() {
            return Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand);
        }
        Ok(())
    }
}

impl ModuleValidationJobClaimCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.validation_job_id.trim().is_empty() || !self.actor_principal.is_object() {
            return Err(ModuleGovernanceError::InvalidValidationJobClaimCommand);
        }
        Ok(())
    }
}

impl ModuleValidationJobResultCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.validation_job_id.trim().is_empty()
            || !self.actor_principal.is_object()
            || !self.automated_checks.is_array()
            || self
                .actor_principal
                .get("id")
                .or_else(|| self.actor_principal.get("subject"))
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(ModuleGovernanceError::InvalidValidationJobResultCommand);
        }
        let has_errors = self.errors.iter().any(|error| !error.trim().is_empty());
        match self.outcome {
            ModuleValidationJobResultOutcome::Passed if has_errors => {
                Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
            }
            ModuleValidationJobResultOutcome::Failed if !has_errors => {
                Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
            }
            _ => Ok(()),
        }
    }
}

impl ModuleValidationJobRetryCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.validation_job_id.trim().is_empty()
            || !self.actor_principal.is_object()
            || self.attempt == 0
            || self.error.trim().is_empty()
        {
            return Err(ModuleGovernanceError::InvalidValidationJobRetryCommand);
        }
        Ok(())
    }
}

impl ModulePublishRequestCreateCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.slug.trim().is_empty()
            || self.version.trim().is_empty()
            || self.crate_name.trim().is_empty()
            || self.default_locale.trim().is_empty()
            || self.ownership.trim().is_empty()
            || self.trust_level.trim().is_empty()
            || self.license.trim().is_empty()
            || self.name.trim().is_empty()
            || self.description.trim().is_empty()
            || !self.marketplace.is_object()
            || !self.ui_packages.is_array()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand);
        }
        Ok(())
    }
}

impl ModulePublishArtifactAttachCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.artifact_storage_key.trim().is_empty()
            || self.checksum_sha256.trim().is_empty()
            || self.artifact_size < 0
            || self.content_type.trim().is_empty()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPublishArtifactAttachCommand);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SeaOrmModuleGovernanceService {
    db: DatabaseConnection,
}

impl SeaOrmModuleGovernanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Creates a draft publish request, default-locale metadata, and audit fact
    /// atomically. Authorization remains a host concern.
    pub async fn create_publish_request(
        &self,
        command: ModulePublishRequestCreateCommand,
    ) -> Result<String, ModuleGovernanceError> {
        command.validate()?;
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let active_release = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_module_releases WHERE slug = {} AND version = {} AND status = 'active' LIMIT 1",
                    mark(1), mark(2)
                ),
                vec![command.slug.clone().into(), command.version.clone().into()],
            ))
            .await
            .map_err(store_error)?;
        if active_release.is_some() {
            return Err(ModuleGovernanceError::PublishRequestReleaseAlreadyActive {
                slug: command.slug,
                version: command.version,
            });
        }
        let request_id = format!("rpr_{}", Uuid::new_v4().simple());
        let warnings = dedupe_validation_messages(command.warnings);
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_requests (id, slug, version, crate_name, default_locale, ownership, trust_level, license, entry_type, marketplace, ui_packages, status, requested_by_principal, publisher_principal, approved_by_principal, rejected_by_principal, rejection_reason, changes_requested_by_principal, changes_requested_reason, changes_requested_reason_code, changes_requested_at, held_by_principal, held_reason, held_reason_code, held_at, held_from_status, validation_warnings, validation_errors, artifact_storage_key, artifact_checksum_sha256, artifact_size, artifact_content_type, submitted_at, validated_at, approved_at, published_at, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'draft', {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15)),
            vec![request_id.clone().into(), command.slug.clone().into(), command.version.clone().into(), command.crate_name.into(), command.default_locale.clone().into(), command.ownership.into(), command.trust_level.into(), command.license.into(), command.entry_type.into(), Value::Json(Some(Box::new(command.marketplace))), Value::Json(Some(Box::new(command.ui_packages))), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!([]))))],
        )).await.map_err(store_error)?;
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_request_translations (request_id, locale, name, description, created_at, updated_at) VALUES ({}, {}, {}, {}, {now}, {now})", mark(1), mark(2), mark(3), mark(4)),
            vec![request_id.clone().into(), command.default_locale.into(), command.name.trim().to_string().into(), command.description.trim().to_string().into()],
        )).await.map_err(store_error)?;
        let details =
            serde_json::json!({"version":command.version,"status":"draft","warnings":warnings});
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'request_created', {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
            vec![format!("rge_{}", Uuid::new_v4().simple()).into(), command.slug.into(), request_id.clone().into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(details)))],
        )).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(request_id)
    }

    /// Attaches host-stored artifact bytes to an eligible request. All durable
    /// status, validation-reset, and audit mutations share this transaction.
    pub async fn attach_publish_artifact(
        &self,
        command: ModulePublishArtifactAttachCommand,
    ) -> Result<ModulePublishArtifactAttachResult, ModuleGovernanceError> {
        command.validate()?;
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT slug, version, status, artifact_storage_key, CAST(validation_warnings AS TEXT) AS validation_warnings, CAST(requested_by_principal AS TEXT) AS requested_by_principal FROM registry_publish_requests WHERE id = {}", mark(1)),
            vec![command.request_id.clone().into()],
        )).await.map_err(store_error)?.ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let reuploaded = status == "changes_requested";
        if status != "draft" && !reuploaded {
            return Err(ModuleGovernanceError::PublishRequestCannotAttachArtifact(
                status,
            ));
        }
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let version: String = request.try_get("", "version").map_err(store_error)?;
        let previous_storage_key: Option<String> = request
            .try_get("", "artifact_storage_key")
            .map_err(store_error)?;
        let existing_warnings: String = request
            .try_get("", "validation_warnings")
            .map_err(store_error)?;
        let requested_by: String = request
            .try_get("", "requested_by_principal")
            .map_err(store_error)?;
        let mut warnings = if reuploaded {
            Vec::new()
        } else {
            serde_json::from_str::<serde_json::Value>(&existing_warnings)
                .ok()
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        };
        let actor = validation_stage_actor_label(&command.actor_principal)?;
        let requested_by_label = serde_json::from_str::<serde_json::Value>(&requested_by)
            .ok()
            .and_then(|value| {
                value
                    .get("id")
                    .or_else(|| value.get("subject"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or(requested_by);
        if actor != requested_by_label {
            warnings.push(format!("Artifact was uploaded by '{actor}' for publish request originally created by '{requested_by_label}'."));
        }
        let warnings = dedupe_validation_messages(warnings);
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE registry_publish_requests SET status = 'submitted', artifact_storage_key = {}, artifact_checksum_sha256 = {}, artifact_size = {}, artifact_content_type = {}, submitted_at = {now}, validation_warnings = {}, validation_errors = {}, approved_by_principal = NULL, rejected_by_principal = NULL, rejection_reason = NULL, validated_at = NULL, approved_at = NULL, published_at = NULL, updated_at = {now} WHERE id = {} AND status IN ('draft', 'changes_requested')", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7)),
            vec![command.artifact_storage_key.clone().into(), command.checksum_sha256.clone().into(), command.artifact_size.into(), command.content_type.clone().into(), Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!([])))), command.request_id.clone().into()],
        )).await.map_err(store_error)?;
        if reuploaded {
            for table in ["registry_validation_stages", "registry_validation_jobs"] {
                tx.execute(Statement::from_sql_and_values(
                    backend,
                    format!("DELETE FROM {table} WHERE request_id = {}", mark(1)),
                    vec![command.request_id.clone().into()],
                ))
                .await
                .map_err(store_error)?;
            }
        }
        let artifact_details = serde_json::json!({"version":version,"status":"submitted","artifact_size":command.artifact_size,"content_type":command.content_type,"checksum_sha256":command.checksum_sha256});
        for (event_type, details) in [
            ("artifact_uploaded", artifact_details.clone()),
            (
                "artifact_reuploaded_after_changes_requested",
                artifact_details,
            ),
        ] {
            if event_type == "artifact_reuploaded_after_changes_requested" && !reuploaded {
                continue;
            }
            tx.execute(Statement::from_sql_and_values(backend, format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)), vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.clone().into(), command.request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(details)))] )).await.map_err(store_error)?;
        }
        tx.commit().await.map_err(store_error)?;
        Ok(ModulePublishArtifactAttachResult {
            request_id: command.request_id,
            previous_storage_key,
            reuploaded_after_changes_requested: reuploaded,
        })
    }

    pub async fn yank_release(
        &self,
        command: ModuleReleaseYankCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let release = tx.query_one(Statement::from_sql_and_values(backend, format!("SELECT id, request_id, CAST(publisher_principal AS TEXT) AS publisher_principal FROM registry_module_releases WHERE slug = {} AND version = {}", mark(1), mark(2)), vec![command.slug.clone().into(), command.version.clone().into()])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::ReleaseNotFound)?;
        let release_id: String = release
            .try_get("", "id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: Option<String> = release
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let publisher: serde_json::Value = serde_json::from_str(
            &release
                .try_get::<String>("", "publisher_principal")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx.execute(Statement::from_sql_and_values(backend, format!("UPDATE registry_module_releases SET status = 'yanked', yanked_reason = {}, yanked_by_principal = {}, yanked_at = {now}, updated_at = {now} WHERE id = {}", mark(1), mark(2), mark(3)), vec![command.reason.clone().into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), release_id.clone().into()])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::ReleaseNotFound);
        }
        tx.execute(Statement::from_sql_and_values(backend, format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, {}, 'release_yanked', {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7)), vec![format!("rge_{}", Uuid::new_v4().simple()).into(), command.slug.into(), request_id.into(), release_id.into(), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(publisher))), Value::Json(Some(Box::new(serde_json::json!({"version": command.version, "status": "yanked", "reason_code": command.reason_code, "reason": command.reason}))) )])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Transfers a registry slug binding and records its immutable audit fact
    /// in the same transaction.
    pub async fn transfer_owner(
        &self,
        command: ModuleOwnerTransferCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let owner = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal \
                     FROM registry_module_owners WHERE slug = {}",
                    mark(1)
                ),
                vec![command.slug.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::OwnerBindingNotFound)?;
        let previous_owner: serde_json::Value = serde_json::from_str(
            &owner
                .try_get::<String>("", "owner_principal")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if previous_owner == command.new_owner_principal {
            return Err(ModuleGovernanceError::OwnerUnchanged);
        }

        let update = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_module_owners \
                     SET owner_principal = {}, bound_by_principal = {}, \
                         bound_at = {now}, updated_at = {now} \
                     WHERE slug = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    Value::Json(Some(Box::new(command.new_owner_principal.clone()))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.slug.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::OwnerBindingNotFound);
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, NULL, NULL, 'owner_transferred', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                command.slug.into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(command.new_owner_principal.clone()))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "owner_transition": {
                        "previous_owner": previous_owner,
                        "new_owner": command.new_owner_principal,
                        "bound_by": command.actor_principal,
                    },
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Creates or refreshes the registry publisher binding. A replacement
    /// records its governance audit fact in the same transaction.
    pub async fn bind_owner(
        &self,
        command: ModuleOwnerBindCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let existing = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal \
                     FROM registry_module_owners WHERE slug = {}",
                    mark(1)
                ),
                vec![command.slug.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;

        let (event_mode, previous_owner) = if let Some(existing) = existing {
            let previous_owner: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "owner_principal")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
            )
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if previous_owner == command.owner_principal {
                tx.execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE registry_module_owners \
                         SET bound_by_principal = {}, updated_at = {now} \
                         WHERE slug = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        Value::Json(Some(Box::new(command.actor_principal))),
                        command.slug.into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                return Ok(());
            }
            if !command.allow_rebind {
                return Err(ModuleGovernanceError::OwnerAlreadyBound);
            }
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_module_owners \
                     SET owner_principal = {}, bound_by_principal = {}, \
                         bound_at = {now}, updated_at = {now} \
                     WHERE slug = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    Value::Json(Some(Box::new(command.owner_principal.clone()))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.slug.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            ("rebind", Some(previous_owner))
        } else {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_owners \
                     (slug, owner_principal, bound_by_principal, bound_at, updated_at) \
                     VALUES ({}, {}, {}, {now}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    command.slug.clone().into(),
                    Value::Json(Some(Box::new(command.owner_principal.clone()))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            ("initial", None)
        };
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, NULL, NULL, 'owner_bound', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                command.slug.into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(command.owner_principal.clone()))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "owner_transition": {
                        "previous_owner": previous_owner,
                        "new_owner": command.owner_principal,
                        "bound_by": command.actor_principal,
                    },
                    "mode": event_mode,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Rejects a publish request and records the terminal governance fact in
    /// the same transaction.
    pub async fn reject_publish_request(
        &self,
        command: ModulePublishRequestRejectCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, status, CAST(validation_errors AS TEXT) AS validation_errors \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if matches!(status.as_str(), "published" | "rejected" | "on_hold") {
            return Err(ModuleGovernanceError::PublishRequestCannotBeRejected(
                status,
            ));
        }
        let stored_errors: serde_json::Value = serde_json::from_str(
            &request
                .try_get::<String>("", "validation_errors")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let mut errors = stored_errors
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let rejection_error = format!("Governance rejection reason: {}", command.reason);
        if !errors.iter().any(|value| value == &rejection_error) {
            errors.push(rejection_error);
        }
        let mut deduplicated = Vec::new();
        for error in errors {
            if !deduplicated.iter().any(|value| value == &error) {
                deduplicated.push(error);
            }
        }
        let validation_errors = serde_json::json!(deduplicated);
        let update = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests \
                     SET status = 'rejected', rejected_by_principal = {}, rejection_reason = {}, \
                         validation_errors = {}, updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                ),
                vec![
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.reason.clone().into(),
                    Value::Json(Some(Box::new(validation_errors.clone()))),
                    command.request_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::PublishRequestNotFound);
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'request_rejected', {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                slug.into(),
                command.request_id.into(),
                Value::Json(Some(Box::new(command.actor_principal))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "rejected",
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                    "errors": validation_errors,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Moves an approved publish request back to the publisher and records the
    /// review decision atomically.
    pub async fn request_publish_request_changes(
        &self,
        command: ModulePublishRequestChangesCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, status, CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "approved" {
            return Err(ModuleGovernanceError::PublishRequestCannotRequestChanges(
                status,
            ));
        }
        let publisher: Option<serde_json::Value> = request
            .try_get::<Option<String>>("", "publisher_principal")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests \
                     SET status = 'changes_requested', changes_requested_by_principal = {}, \
                         changes_requested_reason = {}, changes_requested_reason_code = {}, \
                         changes_requested_at = {now}, updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                ),
                vec![
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.reason.clone().into(),
                    command.reason_code.clone().into(),
                    command.request_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::PublishRequestNotFound);
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'changes_requested', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                slug.into(),
                command.request_id.into(),
                Value::Json(Some(Box::new(command.actor_principal))),
                Value::Json(publisher.map(Box::new)),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "changes_requested",
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Places an eligible publish request on hold while retaining the exact
    /// predecessor state required for a later resume.
    pub async fn hold_publish_request(
        &self,
        command: ModulePublishRequestHoldCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, status, CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let previous_status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if !matches!(
            previous_status.as_str(),
            "submitted" | "approved" | "changes_requested"
        ) {
            return Err(ModuleGovernanceError::PublishRequestCannotBeHeld(
                previous_status,
            ));
        }
        let publisher: Option<serde_json::Value> = request
            .try_get::<Option<String>>("", "publisher_principal")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests \
                     SET status = 'on_hold', held_by_principal = {}, held_reason = {}, \
                         held_reason_code = {}, held_at = {now}, held_from_status = {}, \
                         updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                ),
                vec![
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.reason.clone().into(),
                    command.reason_code.clone().into(),
                    previous_status.clone().into(),
                    command.request_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::PublishRequestNotFound);
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'request_held', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                slug.into(),
                command.request_id.into(),
                Value::Json(Some(Box::new(command.actor_principal))),
                Value::Json(publisher.map(Box::new)),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "on_hold",
                    "held_from_status": previous_status,
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    pub async fn resume_publish_request(
        &self,
        command: ModulePublishRequestResumeCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx.query_one(Statement::from_sql_and_values(backend, format!("SELECT slug, version, status, held_from_status, CAST(publisher_principal AS TEXT) AS publisher_principal FROM registry_publish_requests WHERE id = {}", mark(1)), vec![command.request_id.clone().into()])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "on_hold" {
            return Err(ModuleGovernanceError::PublishRequestCannotBeResumed(status));
        }
        let resumed_status: String = request
            .try_get::<Option<String>>("", "held_from_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| {
                matches!(
                    value.as_str(),
                    "submitted" | "approved" | "changes_requested"
                )
            })
            .ok_or(ModuleGovernanceError::PublishRequestInvalidHeldFromStatus)?;
        let publisher: Option<serde_json::Value> = request
            .try_get::<Option<String>>("", "publisher_principal")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx.execute(Statement::from_sql_and_values(backend, format!("UPDATE registry_publish_requests SET status = {}, updated_at = {now} WHERE id = {}", mark(1), mark(2)), vec![resumed_status.clone().into(), command.request_id.clone().into()])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(ModuleGovernanceError::PublishRequestNotFound);
        }
        tx.execute(Statement::from_sql_and_values(backend, format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'request_resumed', {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)), vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.into(), command.request_id.into(), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(publisher.map(Box::new)), Value::Json(Some(Box::new(serde_json::json!({"version": version, "status": resumed_status.clone(), "resumed_from_hold": true, "resumed_to_status": resumed_status, "reason": command.reason, "reason_code": command.reason_code}))))])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Persists a manual validation-stage transition or a fresh queued attempt
    /// with its stage and follow-up audit facts in one transaction.
    pub async fn report_validation_stage(
        &self,
        command: ModuleValidationStageReportCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, status FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if !matches!(request_status.as_str(), "approved" | "published") {
            return Err(
                ModuleGovernanceError::PublishRequestCannotReportValidationStage(request_status),
            );
        }

        let actor_label = validation_stage_actor_label(&command.actor_principal)?;
        let (stage_id, attempt_number, queue_reason, event_type) = if command.requeue {
            let next_attempt =
                tx.query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number \
                         FROM registry_validation_stages WHERE request_id = {} AND stage_key = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        command.stage_key.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store("missing validation attempt aggregate".to_string())
                })?
                .try_get::<i64>("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))? as i32
                    + 1;
            let stage_id = format!("rvs_{}", Uuid::new_v4().simple());
            let queue_reason = "manual_requeue".to_string();
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_validation_stages \
                     (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, \
                      attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, \
                      claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {}, 'queued', {}, {}, {}, {}, NULL, NULL, NULL, NULL, NULL, \
                             NULL, NULL, NULL, {now}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9),
                ),
                vec![
                    stage_id.clone().into(), command.request_id.clone().into(), slug.clone().into(),
                    version.clone().into(), command.stage_key.clone().into(), actor_label.into(),
                    queue_reason.clone().into(), next_attempt.into(), command.detail.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            (
                stage_id,
                next_attempt,
                queue_reason,
                "validation_stage_queued",
            )
        } else {
            let stage = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id, status, attempt_number, queue_reason \
                         FROM registry_validation_stages WHERE request_id = {} AND stage_key = {} \
                         ORDER BY attempt_number DESC, created_at DESC LIMIT 1",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        command.stage_key.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                .ok_or(ModuleGovernanceError::ValidationStageNotFound)?;
            let stage_id: String = stage
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let current_status: String = stage
                .try_get("", "status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            validation_stage_transition_allowed(
                &current_status,
                &command.status,
                &command.stage_key,
            )?;
            let attempt_number: i32 = stage
                .try_get("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let queue_reason: String = stage
                .try_get("", "queue_reason")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_stages SET status = {}, detail = {}, \
                     last_error = CASE WHEN {} = 'failed' THEN {} ELSE NULL END, \
                     started_at = COALESCE(started_at, {now}), \
                     finished_at = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN {now} ELSE NULL END, \
                     claim_id = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE claim_id END, \
                     claimed_by = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE claimed_by END, \
                     claim_expires_at = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE claim_expires_at END, \
                     last_heartbeat_at = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE last_heartbeat_at END, \
                     runner_kind = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE runner_kind END, \
                     updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                    mark(9),
                    mark(10),
                    mark(11),
                ),
                vec![
                    command.status.clone().into(), command.detail.clone().into(),
                    command.status.clone().into(), command.detail.clone().into(),
                    command.status.clone().into(), command.status.clone().into(),
                    command.status.clone().into(), command.status.clone().into(),
                    command.status.clone().into(), command.status.clone().into(),
                    stage_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let event_type = match command.status.as_str() {
                "running" => "validation_stage_running",
                "passed" => "validation_stage_passed",
                "failed" => "validation_stage_failed",
                "blocked" => "validation_stage_blocked",
                _ => unreachable!("validated non-requeue status"),
            };
            (stage_id, attempt_number, queue_reason, event_type)
        };

        let mut stage_details = serde_json::json!({
            "stage_id": stage_id,
            "stage_key": command.stage_key,
            "status": command.status,
            "detail": command.detail,
            "attempt_number": attempt_number,
            "queue_reason": queue_reason,
            "request_status": request_status,
            "version": version,
        });
        if let Some(reason_code) = &command.reason_code {
            stage_details["reason_code"] = serde_json::Value::String(reason_code.clone());
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                slug.clone().into(),
                command.request_id.clone().into(),
                event_type.into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(stage_details))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let gate = match command.status.as_str() {
            "queued" => Some(("follow_up_gate_queued", "pending")),
            "passed" => Some(("follow_up_gate_passed", "passed")),
            "failed" => Some(("follow_up_gate_failed", "failed")),
            _ => None,
        };
        if let Some((event_type, gate_status)) = gate {
            let mut gate_details = serde_json::json!({
                "stage_key": command.stage_key,
                "status": gate_status,
                "detail": command.detail,
            });
            if let Some(reason_code) = &command.reason_code {
                gate_details["reason_code"] = serde_json::Value::String(reason_code.clone());
            }
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                ),
                vec![
                    format!("rge_{}", Uuid::new_v4().simple()).into(),
                    slug.into(),
                    command.request_id.into(),
                    event_type.into(),
                    Value::Json(Some(Box::new(command.actor_principal))),
                    Value::Json(Some(Box::new(gate_details))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Enqueues at most one active automated validation job and records its
    /// request/job facts atomically. The worker is intentionally outside this
    /// transaction and may begin only after the host observes this result.
    pub async fn enqueue_validation_job(
        &self,
        command: ModuleValidationJobEnqueueCommand,
    ) -> Result<ModuleValidationJobEnqueueResult, ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id, slug, version, status FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let request_id: String = request
            .try_get("", "id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if matches!(
            status.as_str(),
            "draft" | "changes_requested" | "on_hold" | "approved" | "published"
        ) || (status == "rejected" && !command.allow_rejected_retry)
        {
            return Err(ModuleGovernanceError::PublishRequestCannotQueueValidation(
                status,
            ));
        }
        let actor = validation_stage_actor_label(&command.actor_principal)?;
        let active_job = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT id FROM registry_validation_jobs WHERE request_id = {} AND status IN ('queued', 'running') ORDER BY created_at DESC LIMIT 1", mark(1)),
            vec![request_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if let Some(job) = active_job {
            let job_id: String = job
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(ModuleValidationJobEnqueueResult {
                request_id,
                request_status: status,
                queued: false,
                validation_job_id: Some(job_id),
            });
        }
        let requeued = status == "rejected";
        let queue_reason = if status == "validating" {
            "validation_resumed"
        } else if requeued {
            "requeued_after_validation_failed"
        } else {
            "initial_validation"
        };
        if status != "validating" {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_publish_requests SET status = 'validating', validation_errors = {}, rejected_by_principal = NULL, rejection_reason = NULL, validated_at = NULL, updated_at = {now} WHERE id = {}", mark(1), mark(2)),
                vec![Value::Json(Some(Box::new(serde_json::json!([])))), request_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        let attempt = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number FROM registry_validation_jobs WHERE request_id = {}", mark(1)),
            vec![request_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or_else(|| ModuleGovernanceError::Store("missing validation job attempt aggregate".to_string()))?.try_get::<i64>("", "attempt_number").map_err(|e| ModuleGovernanceError::Store(e.to_string()))? as i32 + 1;
        let job_id = format!("rvj_{}", Uuid::new_v4().simple());
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_validation_jobs (id, request_id, slug, version, status, triggered_by, queue_reason, attempt_number, started_at, finished_at, last_error, created_at, updated_at) VALUES ({}, {}, {}, {}, 'queued', {}, {}, {}, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7)),
            vec![job_id.clone().into(), request_id.clone().into(), slug.clone().into(), version.clone().into(), actor.clone().into(), queue_reason.into(), attempt.into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let actor_json = command.actor_principal.clone();
        let events = [
            (
                if requeued {
                    "validation_requeued"
                } else if status == "validating" {
                    "validation_resumed"
                } else {
                    "validation_queued"
                },
                serde_json::json!({"job_id":job_id,"attempt_number":attempt,"queue_reason":queue_reason,"version":version,"status":"validating","requeued":requeued,"follow_up_gates":["compile_smoke","targeted_tests","security_policy_review"]}),
            ),
            (
                "validation_job_queued",
                serde_json::json!({"job_id":job_id,"attempt_number":attempt,"queue_reason":queue_reason,"request_status":"validating","version":version}),
            ),
        ];
        for (event_type, details) in events {
            tx.execute(Statement::from_sql_and_values(backend,
                format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(actor_json.clone()))), Value::Json(Some(Box::new(details)))],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(ModuleValidationJobEnqueueResult {
            request_id,
            request_status: "validating".to_string(),
            queued: true,
            validation_job_id: Some(job_id),
        })
    }

    /// Claims a queued validation job through a conditional update and emits
    /// the started fact in the same transaction. A non-queued job is observed
    /// but never re-executed by this worker invocation.
    pub async fn claim_validation_job(
        &self,
        command: ModuleValidationJobClaimCommand,
    ) -> Result<Option<ModuleValidationJobClaimResult>, ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let Some(job) = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT j.status, j.request_id, j.attempt_number, j.queue_reason, r.slug, r.version, r.status AS request_status FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id WHERE j.id = {}", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))? else {
            return Ok(None);
        };
        let status: String = job
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: String = job
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "queued" {
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(Some(ModuleValidationJobClaimResult {
                request_id,
                should_run: false,
            }));
        }
        let updated = tx.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE registry_validation_jobs SET status = 'running', started_at = {now}, finished_at = NULL, last_error = NULL, updated_at = {now} WHERE id = {} AND status = 'queued'", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if updated.rows_affected() != 1 {
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(Some(ModuleValidationJobClaimResult {
                request_id,
                should_run: false,
            }));
        }
        let slug: String = job
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = job
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = job
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let attempt_number: i32 = job
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let queue_reason: String = job
            .try_get("", "queue_reason")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.execute(Statement::from_sql_and_values(backend,
            format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'validation_job_started', {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5)),
            vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.into(), request_id.clone().into(), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(serde_json::json!({"job_id":command.validation_job_id,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":request_status,"version":version}))))],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(Some(ModuleValidationJobClaimResult {
            request_id,
            should_run: true,
        }))
    }

    /// Applies an automated validation result atomically. A host worker may
    /// inspect and execute an artifact bundle, but it cannot independently
    /// complete the job, mutate the request, or create follow-up stages.
    pub async fn apply_validation_job_result(
        &self,
        command: ModuleValidationJobResultCommand,
    ) -> Result<String, ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let job = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT j.status AS job_status, j.request_id, j.attempt_number, j.queue_reason, r.slug, r.version, r.status AS request_status FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id WHERE j.id = {}", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::ValidationJobNotFound)?;
        let job_status: String = job
            .try_get("", "job_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: String = job
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = job
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let slug: String = job
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = job
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let attempt_number: i32 = job
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let queue_reason: String = job
            .try_get("", "queue_reason")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let (terminal_job_status, terminal_event_type, terminal_request_status) =
            match command.outcome {
                ModuleValidationJobResultOutcome::Passed => {
                    ("succeeded", "validation_job_succeeded", "approved")
                }
                ModuleValidationJobResultOutcome::Failed => {
                    ("failed", "validation_job_failed", "rejected")
                }
            };
        if job_status != "running" {
            if job_status == terminal_job_status && request_status == terminal_request_status {
                tx.commit()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                return Ok(request_id);
            }
            return Err(ModuleGovernanceError::ValidationJobNotRunning(job_status));
        }
        if request_status != "validating" {
            return Err(ModuleGovernanceError::ValidationJobRequestStateMismatch(
                request_status,
            ));
        }

        let warnings = dedupe_validation_messages(command.warnings);
        let errors = dedupe_validation_messages(command.errors);
        let last_error = errors.first().cloned();
        let actor = validation_stage_actor_label(&command.actor_principal)?;
        let request_updated = match command.outcome {
            ModuleValidationJobResultOutcome::Passed => tx.execute(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_publish_requests SET status = 'approved', validation_warnings = {}, validation_errors = {}, rejected_by_principal = NULL, rejection_reason = NULL, validated_at = {now}, approved_by_principal = {}, approved_at = {now}, updated_at = {now} WHERE id = {} AND status = 'validating'", mark(1), mark(2), mark(3), mark(4)),
                vec![Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))).into(), Value::Json(Some(Box::new(serde_json::json!([])))).into(), Value::Json(Some(Box::new(command.actor_principal.clone()))).into(), request_id.clone().into()],
            )),
            ModuleValidationJobResultOutcome::Failed => tx.execute(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_publish_requests SET status = 'rejected', validation_warnings = {}, validation_errors = {}, rejected_by_principal = {}, rejection_reason = {}, validated_at = {now}, approved_by_principal = NULL, approved_at = NULL, published_at = NULL, updated_at = {now} WHERE id = {} AND status = 'validating'", mark(1), mark(2), mark(3), mark(4), mark(5)),
                vec![Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))).into(), Value::Json(Some(Box::new(serde_json::json!(errors.clone())))).into(), Value::Json(Some(Box::new(command.actor_principal.clone()))).into(), last_error.clone().into(), request_id.clone().into()],
            )),
        }.await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if request_updated.rows_affected() != 1 {
            return Err(ModuleGovernanceError::ValidationJobRequestStateMismatch(
                "concurrently changed".to_string(),
            ));
        }
        let job_updated = tx.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE registry_validation_jobs SET status = {}, finished_at = {now}, last_error = {}, updated_at = {now} WHERE id = {} AND status = 'running'", mark(1), mark(2), mark(3)),
            vec![terminal_job_status.into(), last_error.clone().into(), command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if job_updated.rows_affected() != 1 {
            return Err(ModuleGovernanceError::ValidationJobNotRunning(
                "concurrently changed".to_string(),
            ));
        }

        let mut stage_details = Vec::new();
        if command.outcome == ModuleValidationJobResultOutcome::Passed {
            for stage_key in REMOTE_VALIDATION_FOLLOW_UP_STAGES {
                let active_stage = tx.query_one(Statement::from_sql_and_values(
                    backend,
                    format!("SELECT id FROM registry_validation_stages WHERE request_id = {} AND stage_key = {} AND status IN ('queued', 'running') LIMIT 1", mark(1), mark(2)),
                    vec![request_id.clone().into(), (*stage_key).into()],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                if active_stage.is_some() {
                    continue;
                }
                let prior_attempt = tx.query_one(Statement::from_sql_and_values(
                    backend,
                    format!("SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number FROM registry_validation_stages WHERE request_id = {} AND stage_key = {}", mark(1), mark(2)),
                    vec![request_id.clone().into(), (*stage_key).into()],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or_else(|| ModuleGovernanceError::Store("missing validation stage attempt aggregate".to_string()))?;
                let attempt_number = prior_attempt
                    .try_get::<i64>("", "attempt_number")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    as i32
                    + 1;
                let stage_id = format!("rvs_{}", Uuid::new_v4().simple());
                let detail = follow_up_validation_stage_detail(stage_key);
                tx.execute(Statement::from_sql_and_values(
                    backend,
                    format!("INSERT INTO registry_validation_stages (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, 'queued', {}, 'validation_passed', {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8)),
                    vec![stage_id.clone().into(), request_id.clone().into(), slug.clone().into(), version.clone().into(), (*stage_key).into(), actor.clone().into(), attempt_number.into(), detail.into()],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                let stage_value = serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":"queued","detail":detail,"attempt_number":attempt_number,"queue_reason":"validation_passed","started_at":serde_json::Value::Null,"finished_at":serde_json::Value::Null});
                stage_details.push(stage_value.clone());
                for (event_type, details) in [
                    (
                        "validation_stage_queued",
                        serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":"queued","detail":detail,"attempt_number":attempt_number,"queue_reason":"validation_passed","request_status":"approved","version":version,"started_at":serde_json::Value::Null,"finished_at":serde_json::Value::Null}),
                    ),
                    (
                        "follow_up_gate_queued",
                        serde_json::json!({"stage_key":stage_key,"status":"pending","detail":detail}),
                    ),
                ] {
                    tx.execute(Statement::from_sql_and_values(backend,
                        format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                        vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(details)))],
                    )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                }
            }
        }
        let result_event = match command.outcome {
            ModuleValidationJobResultOutcome::Passed => (
                "validation_passed",
                serde_json::json!({"version":version,"status":"approved","warnings":warnings,"automated_checks":command.automated_checks,"follow_up_gates":follow_up_validation_gate_details(),"validation_stages":stage_details}),
            ),
            ModuleValidationJobResultOutcome::Failed => (
                "validation_failed",
                serde_json::json!({"version":version,"status":"rejected","reason":last_error,"warnings":warnings,"errors":errors,"automated_checks":command.automated_checks}),
            ),
        };
        for (event_type, details) in [
            result_event,
            (
                terminal_event_type,
                serde_json::json!({"job_id":command.validation_job_id,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":terminal_request_status,"version":version,"error":last_error}),
            ),
        ] {
            tx.execute(Statement::from_sql_and_values(backend,
                format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(details)))],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(request_id)
    }

    /// Records an in-flight worker retry observation as an owner-owned audit
    /// fact. It does not change the job or request state.
    pub async fn record_validation_job_retry(
        &self,
        command: ModuleValidationJobRetryCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let job = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT j.status, j.attempt_number, r.id AS request_id, r.slug, r.version, r.status AS request_status FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id WHERE j.id = {}", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::ValidationJobNotFound)?;
        let status: String = job
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "running" {
            return Err(ModuleGovernanceError::ValidationJobNotRunning(status));
        }
        let request_id: String = job
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let slug: String = job
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = job
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = job
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let job_attempt: i32 = job
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let event_type = if command.retry_after_seconds.is_some() {
            "validation_retry_scheduled"
        } else {
            "validation_retry_exhausted"
        };
        let mut details = serde_json::json!({"job_id":command.validation_job_id,"job_attempt":job_attempt,"version":version,"status":request_status,"attempt":command.attempt,"error":command.error});
        if let Some(retry_after_seconds) = command.retry_after_seconds {
            details["next_attempt"] = serde_json::json!(command.attempt + 1);
            details["retry_after_seconds"] = serde_json::json!(retry_after_seconds);
        } else {
            details["max_attempts"] = serde_json::json!(command.attempt);
        }
        tx.execute(Statement::from_sql_and_values(backend,
            format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
            vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.into(), request_id.into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(details)))],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Renews a remote validation lease through a conditional update. The
    /// claim id, runner id, running state, remote ownership, and unexpired
    /// lease are one compare-and-swap predicate.
    pub async fn heartbeat_remote_validation_stage(
        &self,
        command: ModuleRemoteValidationHeartbeatCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let now = chrono::Utc::now();
        let expires_at = now
            + chrono::Duration::milliseconds(
                command.lease_ttl_ms.max(1).min(i64::MAX as u64) as i64
            );
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let updated = self
            .db
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_stages SET last_heartbeat_at = {}, \
                     claim_expires_at = {}, updated_at = {} WHERE claim_id = {} \
                     AND claimed_by = {} AND runner_kind = 'remote' AND status = 'running' \
                     AND claim_expires_at >= {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                ),
                vec![
                    now.into(),
                    expires_at.into(),
                    now.into(),
                    command.claim_id.clone().into(),
                    command.runner_id.clone().into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if updated.rows_affected() == 1 {
            return Ok(());
        }
        let stage = self
            .db
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT status, claimed_by, runner_kind FROM registry_validation_stages \
                     WHERE claim_id = {}",
                    mark(1)
                ),
                vec![command.claim_id.into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::RemoteValidationLeaseNotFound)?;
        let claimed_by: Option<String> = stage
            .try_get("", "claimed_by")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let runner_kind: Option<String> = stage
            .try_get("", "runner_kind")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if claimed_by.as_deref() != Some(command.runner_id.as_str())
            || runner_kind.as_deref() != Some("remote")
        {
            return Err(ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch);
        }
        let status: String = stage
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "running" {
            return Err(ModuleGovernanceError::RemoteValidationLeaseNotRunning(
                status,
            ));
        }
        Err(ModuleGovernanceError::RemoteValidationLeaseExpired)
    }

    /// Claims the first eligible validation stage with a compare-and-swap and
    /// records the claim fact in the same transaction. Candidate discovery is
    /// advisory; the conditional update is the serialization point.
    pub async fn claim_remote_validation_stage(
        &self,
        command: ModuleRemoteValidationClaimCommand,
    ) -> Result<Option<ModuleRemoteValidationClaim>, ModuleGovernanceError> {
        let supported_stages = command.normalized_supported_stages()?;
        if supported_stages.is_empty() {
            return Ok(None);
        }
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = chrono::Utc::now();
        let stage_marks = (1..=supported_stages.len())
            .map(&mark)
            .collect::<Vec<_>>()
            .join(", ");
        let mut candidate_values: Vec<Value> =
            supported_stages.iter().cloned().map(Value::from).collect();
        candidate_values.push(now.into());
        candidate_values.push(now.into());
        let candidates = self
            .db
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_validation_stages WHERE stage_key IN ({stage_marks}) \
                     AND ((status = 'queued' AND (claim_expires_at IS NULL OR claim_expires_at <= {})) \
                     OR (status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {})) \
                     ORDER BY created_at ASC LIMIT {}",
                    mark(supported_stages.len() + 1),
                    mark(supported_stages.len() + 2),
                    MAX_REMOTE_VALIDATION_CLAIM_CANDIDATES,
                ),
                candidate_values,
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;

        for candidate in candidates {
            let stage_id: String = candidate
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let tx = self
                .db
                .begin()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let tx_backend = tx.get_database_backend();
            let tx_mark = |n| {
                if tx_backend == sea_orm::DbBackend::Postgres {
                    format!("${n}")
                } else {
                    format!("?{n}")
                }
            };
            let tx_now = if tx_backend == sea_orm::DbBackend::Postgres {
                "NOW()"
            } else {
                "datetime('now')"
            };
            let Some(stage) = tx
                .query_one(Statement::from_sql_and_values(
                    tx_backend,
                    format!(
                        "SELECT s.stage_key, s.status, s.claim_id, s.claimed_by, s.attempt_number, \
                         s.queue_reason, r.id AS request_id, r.slug, r.version, r.status AS request_status, \
                         r.crate_name, r.artifact_storage_key, r.artifact_checksum_sha256 \
                         FROM registry_validation_stages s JOIN registry_publish_requests r ON r.id = s.request_id \
                         WHERE s.id = {}",
                        tx_mark(1)
                    ),
                    vec![stage_id.clone().into()],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            else {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            };
            let request_status: String = stage
                .try_get("", "request_status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let artifact_storage_key: Option<String> = stage
                .try_get("", "artifact_storage_key")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let artifact_checksum_sha256: Option<String> = stage
                .try_get("", "artifact_checksum_sha256")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let Some(artifact_checksum_sha256) = artifact_checksum_sha256 else {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            };
            if !matches!(request_status.as_str(), "approved" | "published")
                || artifact_storage_key.is_none()
            {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            }
            let stage_key: String = stage
                .try_get("", "stage_key")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let status: String = stage
                .try_get("", "status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let previous_claim_id: Option<String> = stage
                .try_get("", "claim_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let previous_runner_id: Option<String> = stage
                .try_get("", "claimed_by")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_id: String = stage
                .try_get("", "request_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let slug: String = stage
                .try_get("", "slug")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let version: String = stage
                .try_get("", "version")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let crate_name: String = stage
                .try_get("", "crate_name")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let attempt_number: i32 = stage
                .try_get("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let queue_reason: String = stage
                .try_get("", "queue_reason")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let claim_id = format!("rvc_{}", Uuid::new_v4().simple());
            let reclaimed = status == "running";
            let detail = if reclaimed {
                format!(
                    "Remote runner '{}' reclaimed expired validation stage '{}' from runner '{}'.",
                    command.runner_id.trim(),
                    stage_key,
                    previous_runner_id.as_deref().unwrap_or("unknown")
                )
            } else {
                format!(
                    "Remote runner '{}' claimed validation stage '{}'.",
                    command.runner_id.trim(),
                    stage_key
                )
            };
            let claim_expires_at = chrono::Utc::now()
                + chrono::Duration::milliseconds(
                    command.lease_ttl_ms.max(1).min(i64::MAX as u64) as i64
                );
            let updated = tx
                .execute(Statement::from_sql_and_values(
                    tx_backend,
                    format!(
                        "UPDATE registry_validation_stages SET status = 'running', detail = {}, \
                         started_at = COALESCE(started_at, {tx_now}), finished_at = NULL, \
                         claim_id = {}, claimed_by = {}, claim_expires_at = {}, \
                         last_heartbeat_at = {tx_now}, runner_kind = 'remote', updated_at = {tx_now} \
                         WHERE id = {} AND ((status = 'queued' AND (claim_expires_at IS NULL OR claim_expires_at <= {tx_now})) \
                         OR (status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {tx_now})) \
                         AND EXISTS (SELECT 1 FROM registry_publish_requests r WHERE r.id = registry_validation_stages.request_id \
                         AND r.status IN ('approved', 'published') AND r.artifact_storage_key IS NOT NULL \
                         AND r.artifact_checksum_sha256 IS NOT NULL)",
                        tx_mark(1),
                        tx_mark(2),
                        tx_mark(3),
                        tx_mark(4),
                        tx_mark(5),
                    ),
                    vec![
                        detail.clone().into(),
                        claim_id.clone().into(),
                        command.runner_id.trim().to_string().into(),
                        claim_expires_at.into(),
                        stage_id.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if updated.rows_affected() != 1 {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            }
            let actor = serde_json::json!({"kind":"remote_runner","id":command.runner_id.trim()});
            let details = serde_json::json!({
                "stage_id": stage_id,
                "stage_key": stage_key,
                "status": "running",
                "detail": detail,
                "attempt_number": attempt_number,
                "queue_reason": queue_reason,
                "request_status": request_status,
                "version": version,
                "claim_id": claim_id,
                "runner_id": command.runner_id.trim(),
                "runner_kind": "remote",
                "execution_mode": "local_workspace",
                "reclaimed_expired_lease": reclaimed,
                "previous_claim_id": previous_claim_id,
                "previous_runner_id": previous_runner_id,
            });
            tx.execute(Statement::from_sql_and_values(
                tx_backend,
                format!(
                    "INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'validation_stage_running', {}, NULL, {}, {tx_now})",
                    tx_mark(1), tx_mark(2), tx_mark(3), tx_mark(4), tx_mark(5),
                ),
                vec![
                    format!("rge_{}", Uuid::new_v4().simple()).into(),
                    slug.clone().into(),
                    request_id.clone().into(),
                    Value::Json(Some(Box::new(actor))),
                    Value::Json(Some(Box::new(details))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(Some(ModuleRemoteValidationClaim {
                claim_id,
                request_id,
                slug,
                version,
                stage_key: stage_key.clone(),
                execution_mode: "local_workspace".to_string(),
                requires_manual_confirmation: stage_key == "security_policy_review",
                allowed_terminal_reason_codes: REGISTRY_VALIDATION_STAGE_REASON_CODES
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                suggested_pass_reason_code: if stage_key == "security_policy_review" {
                    "manual_review_complete".to_string()
                } else {
                    "local_runner_passed".to_string()
                },
                suggested_failure_reason_code: match stage_key.as_str() {
                    "compile_smoke" => "build_failure",
                    "targeted_tests" => "test_failure",
                    "security_policy_review" => "policy_preflight_failed",
                    _ => "manual_override",
                }
                .to_string(),
                suggested_blocked_reason_code: if stage_key == "security_policy_review" {
                    "security_findings".to_string()
                } else {
                    "manual_override".to_string()
                },
                artifact_checksum_sha256,
                crate_name,
            }));
        }
        Ok(None)
    }

    /// Requeues every remote lease that is still expired when its transaction
    /// reaches the compare-and-swap. Blocking the old attempt, creating its
    /// successor, and recording both audit projections are one durable unit.
    pub async fn requeue_expired_remote_validation_claims(
        &self,
    ) -> Result<usize, ModuleGovernanceError> {
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = chrono::Utc::now();
        let candidates = self.db.query_all(Statement::from_sql_and_values(
            backend,
            format!("SELECT id FROM registry_validation_stages WHERE status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {} ORDER BY claim_expires_at ASC", mark(1)),
            vec![now.into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let mut requeued = 0;
        for candidate in candidates {
            let stage_id: String = candidate
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let tx = self
                .db
                .begin()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let backend = tx.get_database_backend();
            let mark = |n| {
                if backend == sea_orm::DbBackend::Postgres {
                    format!("${n}")
                } else {
                    format!("?{n}")
                }
            };
            let now = if backend == sea_orm::DbBackend::Postgres {
                "NOW()"
            } else {
                "datetime('now')"
            };
            let Some(stage) = tx.query_one(Statement::from_sql_and_values(
                backend,
                format!("SELECT s.stage_key, s.attempt_number, s.queue_reason, s.claim_id, s.claimed_by, r.id AS request_id, r.slug, r.version, r.status AS request_status FROM registry_validation_stages s JOIN registry_publish_requests r ON r.id = s.request_id WHERE s.id = {}", mark(1)),
                vec![stage_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))? else {
                tx.rollback().await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            };
            let stage_key: String = stage
                .try_get("", "stage_key")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let attempt_number: i32 = stage
                .try_get("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let queue_reason: String = stage
                .try_get("", "queue_reason")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let claim_id: Option<String> = stage
                .try_get("", "claim_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let claimed_by: Option<String> = stage
                .try_get("", "claimed_by")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_id: String = stage
                .try_get("", "request_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let slug: String = stage
                .try_get("", "slug")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let version: String = stage
                .try_get("", "version")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_status: String = stage
                .try_get("", "request_status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let detail = format!("Remote validation lease expired for runner '{}' (claim '{}'); stage attempt will be requeued.", claimed_by.as_deref().unwrap_or("unknown"), claim_id.as_deref().unwrap_or("unknown"));
            let blocked = tx.execute(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_validation_stages SET status = 'blocked', detail = {}, last_error = NULL, finished_at = {now}, claim_id = NULL, claimed_by = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, runner_kind = NULL, updated_at = {now} WHERE id = {} AND status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {now}", mark(1), mark(2)),
                vec![detail.clone().into(), stage_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if blocked.rows_affected() != 1 {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            }
            let next_attempt = tx.query_one(Statement::from_sql_and_values(
                backend,
                format!("SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number FROM registry_validation_stages WHERE request_id = {} AND stage_key = {}", mark(1), mark(2)),
                vec![request_id.clone().into(), stage_key.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or_else(|| ModuleGovernanceError::Store("missing validation attempt aggregate".to_string()))?.try_get::<i64>("", "attempt_number").map_err(|e| ModuleGovernanceError::Store(e.to_string()))? as i32 + 1;
            let queued_id = format!("rvs_{}", Uuid::new_v4().simple());
            let queued_detail = format!(
                "Remote validation lease expired; retry attempt {} is queued for stage '{}'.",
                next_attempt, stage_key
            );
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!("INSERT INTO registry_validation_stages (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, 'queued', 'system:registry-runner-reaper', 'remote_lease_expired', {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7)),
                vec![queued_id.clone().into(), request_id.clone().into(), slug.clone().into(), version.clone().into(), stage_key.clone().into(), next_attempt.into(), queued_detail.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let actor = serde_json::json!({"kind":"system","id":"registry-runner-reaper"});
            let events = [
                (
                    "validation_stage_blocked",
                    serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":"blocked","detail":detail,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":request_status,"version":version,"reason_code":"other"}),
                ),
                (
                    "validation_stage_queued",
                    serde_json::json!({"stage_id":queued_id,"stage_key":stage_key,"status":"queued","detail":queued_detail,"attempt_number":next_attempt,"queue_reason":"remote_lease_expired","request_status":request_status,"version":version}),
                ),
                (
                    "follow_up_gate_queued",
                    serde_json::json!({"stage_key":stage_key,"status":"pending","detail":queued_detail}),
                ),
            ];
            for (event_type, details) in events {
                tx.execute(Statement::from_sql_and_values(backend,
                    format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                    vec![format!("rge_{}", Uuid::new_v4().simple()).into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(actor.clone()))), Value::Json(Some(Box::new(details)))],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            }
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            requeued += 1;
        }
        Ok(requeued)
    }

    /// Completes a remote lease and emits the terminal stage and follow-up gate
    /// facts in the same transaction. Returns the durable stage ID for a host
    /// adapter that needs to shape a transport response.
    pub async fn complete_remote_validation_stage(
        &self,
        command: ModuleRemoteValidationTerminalCommand,
    ) -> Result<String, ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let row = tx.query_one(Statement::from_sql_and_values(backend, format!(
            "SELECT s.id, s.stage_key, s.status, s.claimed_by, s.runner_kind, s.attempt_number, s.queue_reason, r.id AS request_id, r.slug, r.version, r.status AS request_status \
             FROM registry_validation_stages s JOIN registry_publish_requests r ON r.id = s.request_id WHERE s.claim_id = {}", mark(1)), vec![command.claim_id.clone().into()]))
            .await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::RemoteValidationLeaseNotFound)?;
        let stage_id: String = row
            .try_get("", "id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let stage_key: String = row
            .try_get("", "stage_key")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = row
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let claimed_by: Option<String> = row
            .try_get("", "claimed_by")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let runner_kind: Option<String> = row
            .try_get("", "runner_kind")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if claimed_by.as_deref() != Some(command.runner_id.as_str())
            || runner_kind.as_deref() != Some("remote")
        {
            return Err(ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch);
        }
        if status != "running" {
            return Err(ModuleGovernanceError::RemoteValidationLeaseNotRunning(
                status,
            ));
        }
        let slug: String = row
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: String = row
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = row
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = row
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let attempt_number: i32 = row
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let queue_reason: String = row
            .try_get("", "queue_reason")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let (terminal_status, event_type, gate_event, gate_status, default_reason) =
            match command.outcome {
                ModuleRemoteValidationTerminalOutcome::Passed => (
                    "passed",
                    "validation_stage_passed",
                    "follow_up_gate_passed",
                    "passed",
                    if stage_key == "security_policy_review" {
                        "manual_review_complete"
                    } else {
                        "local_runner_passed"
                    },
                ),
                ModuleRemoteValidationTerminalOutcome::Failed => (
                    "failed",
                    "validation_stage_failed",
                    "follow_up_gate_failed",
                    "failed",
                    match stage_key.as_str() {
                        "compile_smoke" => "build_failure",
                        "targeted_tests" => "test_failure",
                        "security_policy_review" => "policy_preflight_failed",
                        _ => "manual_override",
                    },
                ),
            };
        let reason_code = command
            .reason_code
            .unwrap_or_else(|| default_reason.to_string());
        let detail = command
            .detail
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "Remote validation stage '{}' {} for registry module '{}'.",
                    stage_key, terminal_status, slug
                )
            });
        let update = tx.execute(Statement::from_sql_and_values(backend, format!(
            "UPDATE registry_validation_stages SET status = {}, detail = {}, last_error = {}, started_at = COALESCE(started_at, {now}), finished_at = {now}, claim_id = NULL, claimed_by = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, runner_kind = NULL, updated_at = {now} WHERE id = {} AND claim_id = {} AND claimed_by = {} AND status = 'running' AND claim_expires_at >= {now}", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
            vec![terminal_status.into(), detail.clone().into(), if terminal_status == "failed" { Some(detail.clone()).into() } else { Option::<String>::None.into() }, stage_id.clone().into(), command.claim_id.clone().into(), command.runner_id.clone().into()]))
            .await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            let current = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT status, claimed_by, runner_kind FROM registry_validation_stages \
                         WHERE claim_id = {}",
                        mark(1)
                    ),
                    vec![command.claim_id.clone().into()],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                .ok_or(ModuleGovernanceError::RemoteValidationLeaseNotFound)?;
            let current_claimed_by: Option<String> = current
                .try_get("", "claimed_by")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let current_runner_kind: Option<String> = current
                .try_get("", "runner_kind")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if current_claimed_by.as_deref() != Some(command.runner_id.as_str())
                || current_runner_kind.as_deref() != Some("remote")
            {
                return Err(ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch);
            }
            let current_status: String = current
                .try_get("", "status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if current_status != "running" {
                return Err(ModuleGovernanceError::RemoteValidationLeaseNotRunning(
                    current_status,
                ));
            }
            return Err(ModuleGovernanceError::RemoteValidationLeaseExpired);
        }
        let actor = serde_json::json!({"kind":"remote_runner","id":command.runner_id});
        for (event_type, details) in [
            (
                event_type,
                serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":terminal_status,"detail":detail,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":request_status,"version":version,"reason_code":reason_code}),
            ),
            (
                gate_event,
                serde_json::json!({"stage_key":stage_key,"status":gate_status,"detail":detail,"reason_code":reason_code}),
            ),
        ] {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6),
                ),
                vec![
                    format!("rge_{}", Uuid::new_v4().simple()).into(),
                    slug.clone().into(),
                    request_id.clone().into(),
                    event_type.into(),
                    Value::Json(Some(Box::new(actor.clone()))),
                    Value::Json(Some(Box::new(details))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(stage_id)
    }

    /// Publishes an approved request as one durable governance transition.
    ///
    /// The host performs authorization and assembles any override evidence;
    /// this owner transaction persists the complete release projection, owner
    /// binding, request finalization, and immutable audit facts together.
    pub async fn publish_request(
        &self,
        command: ModulePublishRequestPublicationCommand,
    ) -> Result<(), ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, crate_name, default_locale, ownership, trust_level, license, \
                     entry_type, CAST(marketplace AS TEXT) AS marketplace, \
                     CAST(ui_packages AS TEXT) AS ui_packages, status, artifact_storage_key, \
                     artifact_checksum_sha256, artifact_size \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "approved" {
            return Err(ModuleGovernanceError::PublishRequestCannotBePublished(
                status,
            ));
        }
        let crate_name: String = request
            .try_get("", "crate_name")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let default_locale: String = request
            .try_get("", "default_locale")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let ownership: String = request
            .try_get("", "ownership")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let trust_level: String = request
            .try_get("", "trust_level")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let license: String = request
            .try_get("", "license")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let entry_type: Option<String> = request
            .try_get("", "entry_type")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let marketplace: serde_json::Value = serde_json::from_str(
            &request
                .try_get::<String>("", "marketplace")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let ui_packages: serde_json::Value = serde_json::from_str(
            &request
                .try_get::<String>("", "ui_packages")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let artifact_storage_key = request
            .try_get::<Option<String>>("", "artifact_storage_key")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| !value.trim().is_empty())
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactStorageKey)?;
        let checksum_sha256 = request
            .try_get::<Option<String>>("", "artifact_checksum_sha256")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| !value.trim().is_empty())
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
        let artifact_size = request
            .try_get::<Option<i64>>("", "artifact_size")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| *value >= 0)
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactSize)?;

        let translations = tx
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT locale, name, description FROM registry_publish_request_translations \
                     WHERE request_id = {} ORDER BY locale",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if translations.is_empty() {
            return Err(ModuleGovernanceError::PublishRequestMissingTranslations);
        }

        if let Some(override_evidence) = &command.approval_override {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, NULL, 'publish_approval_override', {}, {}, {}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                ),
                vec![
                    format!("rge_{}", Uuid::new_v4().simple()).into(),
                    slug.clone().into(),
                    command.request_id.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    Value::Json(Some(Box::new(serde_json::json!({
                        "version": version,
                        "reason": override_evidence.reason,
                        "reason_code": override_evidence.reason_code,
                        "validation_stages": override_evidence.validation_stages,
                    })))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        let release_id = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_module_releases WHERE slug = {} AND version = {}",
                    mark(1),
                    mark(2),
                ),
                vec![slug.clone().into(), version.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|row| {
                row.try_get::<String>("", "id")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))
            })
            .transpose()?
            .unwrap_or_else(|| format!("rrel_{}", Uuid::new_v4().simple()));
        let release_exists = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_module_releases WHERE id = {}",
                    mark(1)
                ),
                vec![release_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .is_some();
        if release_exists {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_module_releases SET request_id = {}, crate_name = {}, \
                     default_locale = {}, ownership = {}, trust_level = {}, license = {}, \
                     entry_type = {}, marketplace = {}, ui_packages = {}, status = 'active', \
                     publisher_principal = {}, artifact_storage_key = {}, checksum_sha256 = {}, \
                     artifact_size = {}, yanked_reason = NULL, yanked_by_principal = NULL, \
                     yanked_at = NULL, published_at = {now}, updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                    mark(9),
                    mark(10),
                    mark(11),
                    mark(12),
                    mark(13),
                    mark(14),
                ),
                vec![
                    command.request_id.clone().into(),
                    crate_name.into(),
                    default_locale.into(),
                    ownership.into(),
                    trust_level.into(),
                    license.into(),
                    entry_type.into(),
                    Value::Json(Some(Box::new(marketplace))),
                    Value::Json(Some(Box::new(ui_packages))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    artifact_storage_key.into(),
                    checksum_sha256.clone().into(),
                    artifact_size.into(),
                    release_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        } else {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_releases \
                     (id, request_id, slug, version, crate_name, default_locale, ownership, trust_level, \
                      license, entry_type, marketplace, ui_packages, status, publisher_principal, \
                      artifact_storage_key, checksum_sha256, artifact_size, yanked_reason, \
                      yanked_by_principal, yanked_at, published_at, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'active', {}, {}, {}, {}, \
                             NULL, NULL, NULL, {now}, {now}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8),
                    mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15), mark(16),
                ),
                vec![
                    release_id.clone().into(), command.request_id.clone().into(), slug.clone().into(),
                    version.clone().into(), crate_name.into(), default_locale.into(), ownership.into(),
                    trust_level.into(), license.into(), entry_type.into(),
                    Value::Json(Some(Box::new(marketplace))), Value::Json(Some(Box::new(ui_packages))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    artifact_storage_key.into(), checksum_sha256.clone().into(), artifact_size.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "DELETE FROM registry_module_release_translations WHERE release_id = {}",
                mark(1)
            ),
            vec![release_id.clone().into()],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        for translation in translations {
            let locale: String = translation
                .try_get("", "locale")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let name: String = translation
                .try_get("", "name")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let description: String = translation
                .try_get("", "description")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_release_translations \
                     (release_id, locale, name, description, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {now}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                ),
                vec![
                    release_id.clone().into(),
                    locale.into(),
                    name.into(),
                    description.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        let existing_owner = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal \
                     FROM registry_module_owners WHERE slug = {}",
                    mark(1)
                ),
                vec![slug.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let owner_transition = if let Some(existing_owner) = existing_owner {
            let previous_owner: serde_json::Value = serde_json::from_str(
                &existing_owner
                    .try_get::<String>("", "owner_principal")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
            )
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if previous_owner == command.publisher_principal {
                tx.execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE registry_module_owners SET bound_by_principal = {}, updated_at = {now} \
                         WHERE slug = {}",
                        mark(1), mark(2),
                    ),
                    vec![
                        Value::Json(Some(Box::new(command.actor_principal.clone()))),
                        slug.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                None
            } else {
                if !command.allow_owner_rebind {
                    return Err(ModuleGovernanceError::OwnerAlreadyBound);
                }
                tx.execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE registry_module_owners SET owner_principal = {}, bound_by_principal = {}, \
                         bound_at = {now}, updated_at = {now} WHERE slug = {}",
                        mark(1), mark(2), mark(3),
                    ),
                    vec![
                        Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                        Value::Json(Some(Box::new(command.actor_principal.clone()))),
                        slug.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                Some(("rebind", Some(previous_owner)))
            }
        } else {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_owners \
                     (slug, owner_principal, bound_by_principal, bound_at, updated_at) \
                     VALUES ({}, {}, {}, {now}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    slug.clone().into(),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            Some(("initial", None))
        };
        if let Some((mode, previous_owner)) = owner_transition {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, {}, 'owner_bound', {}, {}, {}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                ),
                vec![
                    format!("rge_{}", Uuid::new_v4().simple()).into(),
                    slug.clone().into(),
                    command.request_id.clone().into(),
                    release_id.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    Value::Json(Some(Box::new(serde_json::json!({
                        "owner_transition": {
                            "previous_owner": previous_owner,
                            "new_owner": command.publisher_principal,
                            "bound_by": command.actor_principal,
                        },
                        "mode": mode,
                    })))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_publish_requests SET status = 'published', approved_by_principal = {}, \
                 approved_at = {now}, published_at = {now}, updated_at = {now} WHERE id = {}",
                mark(1), mark(2),
            ),
            vec![
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                command.request_id.clone().into(),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, {}, 'release_published', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
                mark(7),
            ),
            vec![
                format!("rge_{}", Uuid::new_v4().simple()).into(),
                slug.into(),
                command.request_id.into(),
                release_id.into(),
                Value::Json(Some(Box::new(command.actor_principal))),
                Value::Json(Some(Box::new(command.publisher_principal))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "published",
                    "checksum_sha256": checksum_sha256,
                    "release_status": "active",
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }
}

impl ModuleReleaseYankCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.slug.trim().is_empty()
            || self.version.trim().is_empty()
            || self.reason.trim().is_empty()
            || self.actor_principal.is_null()
        {
            return Err(ModuleGovernanceError::InvalidYankCommand);
        }
        if !REGISTRY_YANK_REASON_CODES.contains(&self.reason_code.as_str()) {
            return Err(ModuleGovernanceError::InvalidYankReasonCode(
                self.reason_code.clone(),
            ));
        }
        Ok(())
    }
}

fn validation_stage_actor_label(
    principal: &serde_json::Value,
) -> Result<String, ModuleGovernanceError> {
    principal
        .get("id")
        .or_else(|| principal.get("subject"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or(ModuleGovernanceError::InvalidValidationStageReportCommand)
}

fn placeholder(backend: sea_orm::DbBackend, position: usize) -> String {
    if backend == sea_orm::DbBackend::Postgres {
        format!("${position}")
    } else {
        format!("?{position}")
    }
}

fn database_now(backend: sea_orm::DbBackend) -> &'static str {
    if backend == sea_orm::DbBackend::Postgres {
        "NOW()"
    } else {
        "datetime('now')"
    }
}

fn store_error(error: impl std::fmt::Display) -> ModuleGovernanceError {
    ModuleGovernanceError::Store(error.to_string())
}

fn dedupe_validation_messages(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if !value.is_empty() && seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn follow_up_validation_stage_detail(stage_key: &str) -> &'static str {
    match stage_key {
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

fn follow_up_validation_gate_details() -> Vec<serde_json::Value> {
    REMOTE_VALIDATION_FOLLOW_UP_STAGES
        .iter()
        .map(|stage_key| {
            serde_json::json!({
                "key": stage_key,
                "status": "pending_follow_up",
                "detail": follow_up_validation_stage_detail(stage_key),
            })
        })
        .collect()
}

fn validation_stage_transition_allowed(
    current: &str,
    next: &str,
    stage_key: &str,
) -> Result<(), ModuleGovernanceError> {
    let allowed = match current {
        "queued" | "running" | "blocked" => {
            matches!(next, "running" | "passed" | "failed" | "blocked")
        }
        "passed" | "failed" => false,
        _ => false,
    };
    if allowed {
        return Ok(());
    }
    Err(ModuleGovernanceError::InvalidValidationStageTransition {
        stage_key: stage_key.to_string(),
        current: current.to_string(),
        next: next.to_string(),
    })
}

#[derive(Debug, Error)]
pub enum ModuleGovernanceError {
    #[error("release yank requires slug, version, reason, and actor principal")]
    InvalidYankCommand,
    #[error("unsupported release yank reason code `{0}`")]
    InvalidYankReasonCode(String),
    #[error("owner transfer requires slug, owner, actor, and reason")]
    InvalidOwnerTransferCommand,
    #[error("owner binding requires slug, owner, and actor")]
    InvalidOwnerBindCommand,
    #[error("publish-request rejection requires request ID, actor, and reason")]
    InvalidPublishRequestRejectCommand,
    #[error("publish-request changes requires request ID, actor, and reason")]
    InvalidPublishRequestChangesCommand,
    #[error("publish-request hold requires request ID, actor, and reason")]
    InvalidPublishRequestHoldCommand,
    #[error("publish-request resume requires request ID, actor, and reason")]
    InvalidPublishRequestResumeCommand,
    #[error("publication requires request ID, actor, and publisher principal")]
    InvalidPublishRequestPublicationCommand,
    #[error("approval override requires a reason and validation-stage evidence")]
    InvalidPublishApprovalOverride,
    #[error(
        "validation-stage report requires request ID, stage key, known status, detail, and actor"
    )]
    InvalidValidationStageReportCommand,
    #[error(
        "validation-stage requeue must use status `queued`, and only requeues may use that status"
    )]
    InvalidValidationStageRequeue,
    #[error("remote validation lease commands require non-empty claim and runner IDs")]
    InvalidRemoteValidationLeaseCommand,
    #[error("validation job enqueue requires a request ID and actor principal")]
    InvalidValidationJobEnqueueCommand,
    #[error("validation job claim requires a job ID and actor principal")]
    InvalidValidationJobClaimCommand,
    #[error(
        "validation job result requires a job ID, actor, coherent outcome, and check evidence"
    )]
    InvalidValidationJobResultCommand,
    #[error("validation job retry requires a job ID, actor, positive attempt, and error detail")]
    InvalidValidationJobRetryCommand,
    #[error("publish-request creation requires complete metadata and an actor principal")]
    InvalidPublishRequestCreateCommand,
    #[error(
        "publish artifact attachment requires durable artifact metadata and an actor principal"
    )]
    InvalidPublishArtifactAttachCommand,
    #[error("unsupported remote validation claim stage `{0}`")]
    InvalidRemoteValidationClaimStage(String),
    #[error("unsupported owner-transfer reason code `{0}`")]
    InvalidOwnerTransferReasonCode(String),
    #[error("unsupported publish-request rejection reason code `{0}`")]
    InvalidPublishRequestRejectReasonCode(String),
    #[error("unsupported publish-request changes reason code `{0}`")]
    InvalidPublishRequestChangesReasonCode(String),
    #[error("unsupported publish-request hold reason code `{0}`")]
    InvalidPublishRequestHoldReasonCode(String),
    #[error("unsupported publish-request resume reason code `{0}`")]
    InvalidPublishRequestResumeReasonCode(String),
    #[error("unsupported publication approval override reason code `{0}`")]
    InvalidPublishApprovalOverrideReasonCode(String),
    #[error("unsupported validation-stage reason code `{0}`")]
    InvalidValidationStageReasonCode(String),
    #[error("published release was not found")]
    ReleaseNotFound,
    #[error("registry owner binding was not found")]
    OwnerBindingNotFound,
    #[error("registry owner is already bound to the requested principal")]
    OwnerUnchanged,
    #[error("registry owner is already bound to a different principal")]
    OwnerAlreadyBound,
    #[error("registry publish request was not found")]
    PublishRequestNotFound,
    #[error("published release `{slug}@{version}` already exists")]
    PublishRequestReleaseAlreadyActive { slug: String, version: String },
    #[error("registry publish request in status `{0}` cannot be rejected")]
    PublishRequestCannotBeRejected(String),
    #[error("registry publish request in status `{0}` cannot request changes")]
    PublishRequestCannotRequestChanges(String),
    #[error("registry publish request in status `{0}` cannot be placed on hold")]
    PublishRequestCannotBeHeld(String),
    #[error("registry publish request in status `{0}` cannot be resumed")]
    PublishRequestCannotBeResumed(String),
    #[error("registry publish request in status `{0}` cannot be published")]
    PublishRequestCannotBePublished(String),
    #[error("registry publish request in status `{0}` cannot accept an artifact")]
    PublishRequestCannotAttachArtifact(String),
    #[error("registry publish request in status `{0}` cannot accept validation-stage updates")]
    PublishRequestCannotReportValidationStage(String),
    #[error("registry publish request in status `{0}` cannot queue automated validation")]
    PublishRequestCannotQueueValidation(String),
    #[error("registry validation job was not found")]
    ValidationJobNotFound,
    #[error("registry validation job is not running (status `{0}`)")]
    ValidationJobNotRunning(String),
    #[error("registry validation job request is not validating (status `{0}`)")]
    ValidationJobRequestStateMismatch(String),
    #[error("registry publish request is missing artifact storage key")]
    PublishRequestMissingArtifactStorageKey,
    #[error("registry publish request is missing artifact checksum")]
    PublishRequestMissingArtifactChecksum,
    #[error("registry publish request is missing a valid artifact size")]
    PublishRequestMissingArtifactSize,
    #[error("registry publish request has no localized metadata")]
    PublishRequestMissingTranslations,
    #[error("registry validation stage was not found")]
    ValidationStageNotFound,
    #[error("remote validation claim was not found")]
    RemoteValidationLeaseNotFound,
    #[error("remote validation claim belongs to another runner or is not remote-owned")]
    RemoteValidationLeaseRunnerMismatch,
    #[error("remote validation claim is not running (status `{0}`)")]
    RemoteValidationLeaseNotRunning(String),
    #[error("remote validation claim has expired")]
    RemoteValidationLeaseExpired,
    #[error(
        "validation stage `{stage_key}` cannot move from `{current}` to `{next}` without requeue"
    )]
    InvalidValidationStageTransition {
        stage_key: String,
        current: String,
        next: String,
    },
    #[error("held publish request has no resumable predecessor status")]
    PublishRequestInvalidHeldFromStatus,
    #[error("module governance store error: {0}")]
    Store(String),
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};

    use super::*;

    #[test]
    fn release_yank_contract_rejects_unrecognized_reason_codes() {
        let command = ModuleReleaseYankCommand {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            reason: "security remediation".to_string(),
            reason_code: "unrecognized".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
        };
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidYankReasonCode(_))
        ));
    }

    #[test]
    fn publication_contract_requires_structured_principals_and_reviewed_override() {
        let command = ModulePublishRequestPublicationCommand {
            request_id: "request-1".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
            publisher_principal: serde_json::json!("publisher"),
            allow_owner_rebind: false,
            approval_override: Some(ModulePublishApprovalOverride {
                reason: "manual review".to_string(),
                reason_code: "manual_review_complete".to_string(),
                validation_stages: serde_json::json!([]),
            }),
        };
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestPublicationCommand)
        ));
    }

    #[test]
    fn validation_job_enqueue_requires_request_and_structured_actor() {
        assert!(matches!(
            ModuleValidationJobEnqueueCommand {
                request_id: " ".to_string(),
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                allow_rejected_retry: false,
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand)
        ));
        assert!(matches!(
            ModuleValidationJobEnqueueCommand {
                request_id: "request-1".to_string(),
                actor_principal: serde_json::json!("operator"),
                allow_rejected_retry: false,
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand)
        ));
    }

    #[test]
    fn validation_job_result_requires_coherent_evidence() {
        let actor_principal = serde_json::json!({ "kind": "service", "id": "worker" });
        assert!(matches!(
            ModuleValidationJobResultCommand {
                validation_job_id: "job-1".to_string(),
                actor_principal: actor_principal.clone(),
                outcome: ModuleValidationJobResultOutcome::Passed,
                warnings: Vec::new(),
                errors: vec!["unexpected error".to_string()],
                automated_checks: serde_json::json!([]),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
        ));
        assert!(matches!(
            ModuleValidationJobResultCommand {
                validation_job_id: "job-1".to_string(),
                actor_principal,
                outcome: ModuleValidationJobResultOutcome::Failed,
                warnings: Vec::new(),
                errors: Vec::new(),
                automated_checks: serde_json::json!([]),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
        ));
    }

    #[test]
    fn remote_lease_contract_rejects_blank_identity_and_unknown_reason_code() {
        assert!(matches!(
            ModuleRemoteValidationHeartbeatCommand {
                claim_id: " ".to_string(),
                runner_id: "runner-1".to_string(),
                lease_ttl_ms: 1,
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand)
        ));
        assert!(matches!(
            ModuleRemoteValidationTerminalCommand {
                claim_id: "claim-1".to_string(),
                runner_id: "runner-1".to_string(),
                outcome: ModuleRemoteValidationTerminalOutcome::Passed,
                detail: None,
                reason_code: Some("unknown".to_string()),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationStageReasonCode(_))
        ));
    }

    #[tokio::test]
    async fn release_yank_persists_release_and_audit_fact_together() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                publisher_principal TEXT NOT NULL, status TEXT NOT NULL, yanked_reason TEXT NULL,\
                yanked_by_principal TEXT NULL, yanked_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_module_releases (\
                id, request_id, slug, version, publisher_principal, status, updated_at\
             ) VALUES (\
                'release-1', 'request-1', 'sample_module', '1.0.0', '{\"subject\":\"publisher\"}', 'active', datetime('now')\
             )",
        ] {
            database
                .execute(Statement::from_string(DbBackend::Sqlite, statement.to_string()))
                .await
                .expect("schema or fixture");
        }
        SeaOrmModuleGovernanceService::new(database.clone())
            .yank_release(ModuleReleaseYankCommand {
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                reason: "critical regression".to_string(),
                reason_code: "critical_regression".to_string(),
                actor_principal: serde_json::json!({ "subject": "operator" }),
            })
            .await
            .expect("yank release");
        let release = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status, yanked_reason FROM registry_module_releases".to_string(),
            ))
            .await
            .expect("release query")
            .expect("release row");
        assert_eq!(
            release.try_get::<String>("", "status").expect("status"),
            "yanked"
        );
        assert_eq!(
            release
                .try_get::<String>("", "yanked_reason")
                .expect("reason"),
            "critical regression"
        );
        let event = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT request_id, release_id, event_type FROM registry_governance_events"
                    .to_string(),
            ))
            .await
            .expect("event query")
            .expect("event row");
        assert_eq!(
            event.try_get::<String>("", "request_id").expect("request"),
            "request-1"
        );
        assert_eq!(
            event.try_get::<String>("", "release_id").expect("release"),
            "release-1"
        );
        assert_eq!(
            event
                .try_get::<String>("", "event_type")
                .expect("event type"),
            "release_yanked"
        );
    }

    #[tokio::test]
    async fn owner_transfer_persists_binding_and_audit_fact_together() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY, owner_principal TEXT NOT NULL, \
                bound_by_principal TEXT NOT NULL, bound_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_module_owners (\
                slug, owner_principal, bound_by_principal, bound_at, updated_at\
             ) VALUES (\
                'sample_module', '{\"kind\":\"user\",\"id\":\"previous\"}', \
                '{\"kind\":\"user\",\"id\":\"operator\"}', datetime('now'), datetime('now')\
             )",
        ] {
            database
                .execute(Statement::from_string(DbBackend::Sqlite, statement.to_string()))
                .await
                .expect("schema or fixture");
        }
        SeaOrmModuleGovernanceService::new(database.clone())
            .transfer_owner(ModuleOwnerTransferCommand {
                slug: "sample_module".to_string(),
                new_owner_principal: serde_json::json!({ "kind": "user", "id": "next" }),
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                reason: "maintenance handoff".to_string(),
                reason_code: "maintenance_handoff".to_string(),
            })
            .await
            .expect("transfer owner");
        let binding = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT owner_principal, bound_by_principal FROM registry_module_owners"
                    .to_string(),
            ))
            .await
            .expect("binding query")
            .expect("binding row");
        let owner: serde_json::Value = serde_json::from_str(
            &binding
                .try_get::<String>("", "owner_principal")
                .expect("owner"),
        )
        .expect("owner JSON");
        assert_eq!(owner["id"], "next");
        let event = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT event_type, request_id, release_id, details FROM registry_governance_events"
                    .to_string(),
            ))
            .await
            .expect("event query")
            .expect("event row");
        assert_eq!(
            event
                .try_get::<String>("", "event_type")
                .expect("event type"),
            "owner_transferred"
        );
        assert_eq!(
            event
                .try_get::<Option<String>>("", "request_id")
                .expect("request id"),
            None
        );
        assert_eq!(
            event
                .try_get::<Option<String>>("", "release_id")
                .expect("release id"),
            None
        );
        let details: serde_json::Value = serde_json::from_str(
            &event
                .try_get::<String>("", "details")
                .expect("event details"),
        )
        .expect("details JSON");
        assert_eq!(details["reason_code"], "maintenance_handoff");
        assert_eq!(
            details["owner_transition"]["previous_owner"]["id"],
            "previous"
        );
        assert_eq!(details["owner_transition"]["new_owner"]["id"], "next");
    }

    #[tokio::test]
    async fn hold_then_resume_preserves_predecessor_state_and_audit_facts() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL,\
                publisher_principal TEXT NULL, held_by_principal TEXT NULL, held_reason TEXT NULL,\
                held_reason_code TEXT NULL, held_at TEXT NULL, held_from_status TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests (\
                id, slug, version, status, publisher_principal, updated_at\
             ) VALUES (\
                'request-1', 'sample_module', '1.0.0', 'approved', \
                '{\"kind\":\"user\",\"id\":\"publisher\"}', datetime('now')\
             )",
        ] {
            database
                .execute(Statement::from_string(DbBackend::Sqlite, statement.to_string()))
                .await
                .expect("schema or fixture");
        }
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        service
            .hold_publish_request(ModulePublishRequestHoldCommand {
                request_id: "request-1".to_string(),
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                reason: "release window".to_string(),
                reason_code: "release_window".to_string(),
            })
            .await
            .expect("hold request");
        service
            .resume_publish_request(ModulePublishRequestResumeCommand {
                request_id: "request-1".to_string(),
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                reason: "window closed".to_string(),
                reason_code: "review_complete".to_string(),
            })
            .await
            .expect("resume request");
        let request = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status, held_from_status FROM registry_publish_requests".to_string(),
            ))
            .await
            .expect("request query")
            .expect("request row");
        assert_eq!(
            request.try_get::<String>("", "status").expect("status"),
            "approved"
        );
        assert_eq!(
            request
                .try_get::<String>("", "held_from_status")
                .expect("held predecessor"),
            "approved"
        );
        let events = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT event_type FROM registry_governance_events ORDER BY created_at, id"
                    .to_string(),
            ))
            .await
            .expect("event query");
        assert_eq!(events.len(), 2);
        let event_types = events
            .iter()
            .map(|event| {
                event
                    .try_get::<String>("", "event_type")
                    .expect("event type")
            })
            .collect::<Vec<_>>();
        assert!(event_types.iter().any(|event| event == "request_held"));
        assert!(event_types.iter().any(|event| event == "request_resumed"));
    }

    #[tokio::test]
    async fn publication_persists_release_binding_request_and_audit_facts_together() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, crate_name TEXT NOT NULL,\
                default_locale TEXT NOT NULL, ownership TEXT NOT NULL, trust_level TEXT NOT NULL,\
                license TEXT NOT NULL, entry_type TEXT NULL, marketplace TEXT NOT NULL, ui_packages TEXT NOT NULL,\
                status TEXT NOT NULL, artifact_storage_key TEXT NULL, artifact_checksum_sha256 TEXT NULL,\
                artifact_size INTEGER NULL, approved_by_principal TEXT NULL, approved_at TEXT NULL,\
                published_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_request_translations (\
                request_id TEXT NOT NULL, locale TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,\
                PRIMARY KEY (request_id, locale)\
             )",
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                crate_name TEXT NOT NULL, default_locale TEXT NOT NULL, ownership TEXT NOT NULL,\
                trust_level TEXT NOT NULL, license TEXT NOT NULL, entry_type TEXT NULL, marketplace TEXT NOT NULL,\
                ui_packages TEXT NOT NULL, status TEXT NOT NULL, publisher_principal TEXT NOT NULL,\
                artifact_storage_key TEXT NULL, checksum_sha256 TEXT NULL, artifact_size INTEGER NULL,\
                yanked_reason TEXT NULL, yanked_by_principal TEXT NULL, yanked_at TEXT NULL,\
                published_at TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,\
                UNIQUE (slug, version)\
             )",
            "CREATE TABLE registry_module_release_translations (\
                release_id TEXT NOT NULL, locale TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,\
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY (release_id, locale)\
             )",
            "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY, owner_principal TEXT NOT NULL, bound_by_principal TEXT NOT NULL,\
                bound_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests (\
                id, slug, version, crate_name, default_locale, ownership, trust_level, license, entry_type,\
                marketplace, ui_packages, status, artifact_storage_key, artifact_checksum_sha256, artifact_size, updated_at\
             ) VALUES (\
                'request-1', 'sample_module', '1.0.0', 'sample_crate', 'en', 'platform', 'verified', 'MIT',\
                NULL, '{}', '[]', 'approved', 'registry/request-1', 'abc123', 42, datetime('now')\
             )",
            "INSERT INTO registry_publish_request_translations (request_id, locale, name, description) VALUES (\
                'request-1', 'en', 'Sample module', 'Sample module description'\
             )",
        ] {
            database
                .execute(Statement::from_string(DbBackend::Sqlite, statement.to_string()))
                .await
                .expect("schema or fixture");
        }

        SeaOrmModuleGovernanceService::new(database.clone())
            .publish_request(ModulePublishRequestPublicationCommand {
                request_id: "request-1".to_string(),
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                publisher_principal: serde_json::json!({ "kind": "user", "id": "publisher" }),
                allow_owner_rebind: false,
                approval_override: None,
            })
            .await
            .expect("publish request");

        let request = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status FROM registry_publish_requests WHERE id = 'request-1'".to_string(),
            ))
            .await
            .expect("request query")
            .expect("request row");
        assert_eq!(
            request
                .try_get::<String>("", "status")
                .expect("request status"),
            "published"
        );
        let release = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT id, status, checksum_sha256 FROM registry_module_releases".to_string(),
            ))
            .await
            .expect("release query")
            .expect("release row");
        assert_eq!(
            release
                .try_get::<String>("", "status")
                .expect("release status"),
            "active"
        );
        assert_eq!(
            release
                .try_get::<String>("", "checksum_sha256")
                .expect("release checksum"),
            "abc123"
        );
        let release_id: String = release.try_get("", "id").expect("release id");
        let translations = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT locale, name FROM registry_module_release_translations".to_string(),
            ))
            .await
            .expect("release translations");
        assert_eq!(translations.len(), 1);
        assert_eq!(
            translations[0]
                .try_get::<String>("", "locale")
                .expect("translation locale"),
            "en"
        );
        let owner = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT owner_principal FROM registry_module_owners WHERE slug = 'sample_module'"
                    .to_string(),
            ))
            .await
            .expect("owner query")
            .expect("owner row");
        let owner: serde_json::Value = serde_json::from_str(
            &owner
                .try_get::<String>("", "owner_principal")
                .expect("owner principal"),
        )
        .expect("owner JSON");
        assert_eq!(owner["id"], "publisher");
        let events = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT event_type, release_id FROM registry_governance_events ORDER BY created_at, id"
                    .to_string(),
            ))
            .await
            .expect("events query");
        assert_eq!(events.len(), 2);
        let event_types = events
            .iter()
            .map(|event| {
                event
                    .try_get::<String>("", "event_type")
                    .expect("event type")
            })
            .collect::<Vec<_>>();
        assert!(event_types
            .iter()
            .any(|event_type| event_type == "owner_bound"));
        let publication = events
            .iter()
            .find(|event| {
                event
                    .try_get::<String>("", "event_type")
                    .expect("event type")
                    == "release_published"
            })
            .expect("publication event");
        assert_eq!(
            publication
                .try_get::<String>("", "release_id")
                .expect("publication release ID"),
            release_id
        );
    }
}
