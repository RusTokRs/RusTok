//! Owner contracts for registry governance transitions.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::build::{
    ModuleBuildOutcome, ModuleBuildPublicationReceipt, ModuleBuildSignatureAuthority,
    SeaOrmModuleBuildService,
};
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};

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

/// Explicit reasons why an external prebuilt artifact cannot provide a
/// reproducible source identity. Absence is a reviewable trust fact, never an
/// implicit downgrade to a platform build.
pub const REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES: &[&str] = &[
    "source_unavailable",
    "reproducibility_not_supported",
    "license_restriction",
    "other",
];

const REMOTE_VALIDATION_FOLLOW_UP_STAGES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];
const MAX_REMOTE_VALIDATION_CLAIM_CANDIDATES: u64 = 128;
const MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES: usize = 512;
const MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES: usize = 256;
const MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES: usize = 128;
const MAX_PLATFORM_ADMISSION_MEDIA_TYPE_BYTES: usize = 256;
const MARKETPLACE_APPROVAL_POLICY_REVISION: &str = "registry-governance-v1";
/// A worker that has not materialized a terminal result within this interval
/// is considered lost. A later authorized enqueue marks that attempt failed
/// and creates a new durable attempt in the same owner transaction.
const VALIDATION_JOB_STALE_AFTER_SECONDS: u64 = 15 * 60;
const VALIDATION_WORK_ITEM_INVALID_ERROR: &str =
    "Validation job delivery facts are incomplete or malformed.";

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
    /// Stable external command identity for exactly-once final publication.
    pub idempotency_key: Uuid,
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
    pub slug: String,
    pub version: String,
    pub crate_name: String,
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
    pub artifact_origin: ModulePublicationArtifactOrigin,
    pub marketplace: serde_json::Value,
    pub ui_packages: serde_json::Value,
    pub name: String,
    pub description: String,
    pub actor_principal: serde_json::Value,
}

/// The origin is immutable release provenance, not a caller-selected trust
/// level. An unclassified durable request is intentionally not promotable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulePublicationArtifactOrigin {
    PlatformBuilt,
    ExternalPrebuilt,
}

impl ModulePublicationArtifactOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::PlatformBuilt => "platform_built",
            Self::ExternalPrebuilt => "external_prebuilt",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "platform_built" => Some(Self::PlatformBuilt),
            "external_prebuilt" => Some(Self::ExternalPrebuilt),
            _ => None,
        }
    }
}

/// Immutable source-evidence classification for an external prebuilt
/// artifact. `Unavailable` is explicit durable evidence, not a default.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModuleExternalSourceEvidence {
    Reproducible { reference: String, digest: String },
    Unavailable { reason_code: String },
}

/// Owner-authenticated immutable external-artifact staging. The platform
/// records the approved provenance policy and stricter quarantine review
/// before the request can enter ordinary final publication.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleExternalPrebuiltStageCommand {
    pub request_id: String,
    pub artifact_digest: String,
    pub source_evidence: ModuleExternalSourceEvidence,
    pub provenance_reference: String,
    pub provenance_digest: String,
    pub provenance_policy_revision: String,
    pub quarantine_review_reference: String,
    pub quarantine_policy_revision: String,
    pub quarantine_approved_by_principal: serde_json::Value,
    pub idempotency_key: Uuid,
    pub actor_principal: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleExternalPrebuiltStageResult {
    pub staging_id: String,
    pub created: bool,
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

/// Distinct authorities whose evidence can be attached to a staged release.
/// These facts deliberately do not imply one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulePublicationEvidenceAuthority {
    AuthorSignature,
    BuildServiceAttestation,
    MarketplaceApproval,
    PlatformAdmission,
}

impl ModulePublicationEvidenceAuthority {
    fn as_str(self) -> &'static str {
        match self {
            Self::AuthorSignature => "author_signature",
            Self::BuildServiceAttestation => "build_service_attestation",
            Self::MarketplaceApproval => "marketplace_approval",
            Self::PlatformAdmission => "platform_admission",
        }
    }
}

/// Host-authorized immutable evidence for one exact staged artifact subject.
/// The reference identifies an externally stored signature, attestation, or
/// decision record; untrusted document contents never enter the ledger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublicationEvidenceCommand {
    pub request_id: String,
    pub authority: ModulePublicationEvidenceAuthority,
    pub subject_digest_sha256: String,
    pub evidence_reference: String,
    pub issuer_identity: String,
    pub policy_revision: String,
    pub actor_principal: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublicationEvidenceResult {
    pub evidence_id: String,
    pub recorded: bool,
}

/// Host-authenticated promotion of a verified build-worker receipt into the
/// publication ledger. Unlike generic evidence, this preserves the exact OCI
/// subject and signature-manifest identity produced by the build service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleBuildServiceAttestationCommand {
    pub request_id: String,
    pub receipt: ModuleBuildPublicationReceipt,
    pub issuer_identity: String,
    pub policy_revision: String,
    pub actor_principal: serde_json::Value,
}

/// Host-authenticated registration of one platform trust decision. The
/// decision evidence remains redacted and immutable; this command only records
/// its digest-bound admission fact in the marketplace governance ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePlatformAdmissionCommand {
    pub request_id: String,
    pub reference: OciArtifactReference,
    pub evidence: ArtifactVerificationEvidence,
    pub actor_principal: serde_json::Value,
}

/// Owner-authenticated selection of one durable completed platform build for a
/// registry request. The command carries only immutable identifiers; the build
/// request/result are always reloaded under tenant RLS by the owner service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublishPlatformBuildStageCommand {
    pub request_id: String,
    pub tenant_id: Uuid,
    pub build_request_id: Uuid,
    pub idempotency_key: Uuid,
    pub actor_principal: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublishPlatformBuildStageResult {
    pub staging_id: String,
    pub created: bool,
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
            || self.idempotency_key.is_nil()
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
        let slug = self.slug.trim();
        if slug.is_empty()
            || !slug.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
            || Version::parse(self.version.trim()).is_err()
            || self.crate_name.trim().is_empty()
            || rustok_api::normalize_locale_tag(&self.default_locale).is_none()
            || self.ownership.trim().is_empty()
            || self.trust_level.trim().is_empty()
            || self.license.trim().is_empty()
            || self.name.trim().is_empty()
            || self.description.trim().len() < 20
            || !self.marketplace.is_object()
            || !self.ui_packages.is_object()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand);
        }
        Ok(())
    }

    /// Returns owner-derived, content-free publication warnings. Transport
    /// adapters must not invent governance policy or persist caller-provided
    /// warning text.
    pub fn validation_warnings(&self) -> Result<Vec<String>, ModuleGovernanceError> {
        self.validate()?;
        let ui_packages = self
            .ui_packages
            .as_object()
            .expect("validated publish request UI packages must be an object");
        let has_admin = ui_packages
            .get("admin")
            .is_some_and(|value| !value.is_null());
        let has_storefront = ui_packages
            .get("storefront")
            .is_some_and(|value| !value.is_null());

        let mut warnings = Vec::new();
        if !has_admin && !has_storefront {
            warnings.push(
                "No publishable admin/storefront UI packages declared; only backend contract would be published."
                    .to_string(),
            );
        }
        if !self.ownership.eq_ignore_ascii_case("first_party") {
            warnings.push(
                "Third-party publishing requires the configured governance and evidence gates before release."
                    .to_string(),
            );
        }
        Ok(warnings)
    }
}

impl ModuleExternalPrebuiltStageCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || receipt_digest_sha256(&self.artifact_digest).is_err()
            || receipt_digest_sha256(&self.provenance_digest).is_err()
            || self.idempotency_key.is_nil()
            || !self.quarantine_approved_by_principal.is_object()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
        }
        for value in [
            &self.provenance_reference,
            &self.quarantine_review_reference,
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
                || value.contains(char::is_whitespace)
            {
                return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
            }
        }
        for value in [
            &self.provenance_policy_revision,
            &self.quarantine_policy_revision,
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES
                || value.contains(char::is_whitespace)
            {
                return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
            }
        }
        match &self.source_evidence {
            ModuleExternalSourceEvidence::Reproducible { reference, digest } => {
                if reference.trim().is_empty()
                    || reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
                    || reference.contains(char::is_whitespace)
                    || receipt_digest_sha256(digest).is_err()
                {
                    return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
                }
            }
            ModuleExternalSourceEvidence::Unavailable { reason_code }
                if !REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES
                    .contains(&reason_code.as_str()) =>
            {
                return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
            }
            ModuleExternalSourceEvidence::Unavailable { .. } => {}
        }
        Ok(())
    }
}

impl ModulePublishArtifactAttachCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.artifact_storage_key.trim().is_empty()
            || !is_sha256_hex(&self.checksum_sha256)
            || self.artifact_size < 0
            || self.content_type.trim().is_empty()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPublishArtifactAttachCommand);
        }
        Ok(())
    }
}

impl ModulePublicationEvidenceCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if matches!(
            self.authority,
            ModulePublicationEvidenceAuthority::MarketplaceApproval
                | ModulePublicationEvidenceAuthority::BuildServiceAttestation
                | ModulePublicationEvidenceAuthority::PlatformAdmission
        ) {
            return Err(ModuleGovernanceError::PublicationEvidenceAuthorityReserved);
        }
        validate_publication_evidence_fields(
            &self.request_id,
            &self.subject_digest_sha256,
            &self.evidence_reference,
            &self.issuer_identity,
            &self.policy_revision,
            &self.actor_principal,
        )
    }
}

impl ModuleBuildServiceAttestationCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.receipt.signature_authority != ModuleBuildSignatureAuthority::BuildService {
            return Err(ModuleGovernanceError::InvalidBuildServiceAttestationCommand);
        }
        let references = [
            &self.receipt.artifact,
            &self.receipt.sbom_referrer,
            &self.receipt.provenance_referrer,
            &self.receipt.signature_manifest,
        ];
        if references
            .iter()
            .any(|reference| reference.validate().is_err())
            || references.iter().skip(1).any(|reference| {
                reference.registry != self.receipt.artifact.registry
                    || reference.repository != self.receipt.artifact.repository
            })
        {
            return Err(ModuleGovernanceError::InvalidBuildServiceAttestationCommand);
        }
        validate_publication_evidence_fields(
            &self.request_id,
            receipt_subject_digest_sha256(&self.receipt)?,
            &format!("oci://{}", self.receipt.signature_manifest.canonical()),
            &self.issuer_identity,
            &self.policy_revision,
            &self.actor_principal,
        )
    }

    fn publication_evidence(
        &self,
    ) -> Result<ModulePublicationEvidenceCommand, ModuleGovernanceError> {
        Ok(ModulePublicationEvidenceCommand {
            request_id: self.request_id.clone(),
            authority: ModulePublicationEvidenceAuthority::BuildServiceAttestation,
            subject_digest_sha256: receipt_subject_digest_sha256(&self.receipt)?.to_string(),
            evidence_reference: format!("oci://{}", self.receipt.signature_manifest.canonical()),
            issuer_identity: self.issuer_identity.clone(),
            policy_revision: self.policy_revision.clone(),
            actor_principal: self.actor_principal.clone(),
        })
    }
}

impl ModulePlatformAdmissionCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        self.reference
            .validate()
            .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?;
        if self.request_id.trim().is_empty()
            || self.evidence.manifest_digest != self.reference.digest
            || !prefixed_sha256_digest(&self.evidence.payload_digest)
            || self.evidence.media_type.trim().is_empty()
            || self.evidence.media_type.len() > MAX_PLATFORM_ADMISSION_MEDIA_TYPE_BYTES
            || self.evidence.signer_identity.trim().is_empty()
            || self.evidence.signer_identity.len() > MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES
            || !self.evidence.signature_verified
            || !self.evidence.provenance_verified
            || !self.evidence.sbom_verified
            || self.evidence.evidence_references.is_empty()
            || self.evidence.evidence_references.iter().any(|reference| {
                reference.trim().is_empty()
                    || reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
            })
        {
            return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
        }
        validate_publication_evidence_fields(
            &self.request_id,
            receipt_digest_sha256(&self.reference.digest)
                .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?,
            &platform_admission_evidence_reference(&self.reference, &self.evidence),
            "rustok-platform-admission",
            &platform_admission_policy_revision(&self.evidence),
            &self.actor_principal,
        )
    }

    fn publication_evidence(
        &self,
        artifact_origin: ModulePublicationArtifactOrigin,
    ) -> Result<ModulePublicationEvidenceCommand, ModuleGovernanceError> {
        let subject_digest_sha256 = match artifact_origin {
            ModulePublicationArtifactOrigin::PlatformBuilt => {
                receipt_digest_sha256(&self.reference.digest)
            }
            ModulePublicationArtifactOrigin::ExternalPrebuilt => {
                receipt_digest_sha256(&self.evidence.payload_digest)
            }
        }
        .map_err(|_| ModuleGovernanceError::InvalidPlatformAdmissionCommand)?;
        Ok(ModulePublicationEvidenceCommand {
            request_id: self.request_id.clone(),
            authority: ModulePublicationEvidenceAuthority::PlatformAdmission,
            subject_digest_sha256: subject_digest_sha256.to_string(),
            evidence_reference: platform_admission_evidence_reference(
                &self.reference,
                &self.evidence,
            ),
            issuer_identity: "rustok-platform-admission".to_string(),
            policy_revision: platform_admission_policy_revision(&self.evidence),
            actor_principal: self.actor_principal.clone(),
        })
    }
}

impl ModulePublishPlatformBuildStageCommand {
    pub fn validate(&self) -> Result<(), ModuleGovernanceError> {
        if self.request_id.trim().is_empty()
            || self.tenant_id.is_nil()
            || self.build_request_id.is_nil()
            || self.idempotency_key.is_nil()
            || !self.actor_principal.is_object()
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
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
        let warnings = command.validation_warnings()?;
        let default_locale = rustok_api::normalize_locale_tag(&command.default_locale)
            .ok_or(ModuleGovernanceError::InvalidPublishRequestCreateCommand)?;
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
        let warnings = dedupe_validation_messages(warnings);
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_requests (id, slug, version, crate_name, default_locale, ownership, trust_level, license, entry_type, artifact_origin, marketplace, ui_packages, status, requested_by_principal, publisher_principal, approved_by_principal, rejected_by_principal, rejection_reason, changes_requested_by_principal, changes_requested_reason, changes_requested_reason_code, changes_requested_at, held_by_principal, held_reason, held_reason_code, held_at, held_from_status, validation_warnings, validation_errors, artifact_storage_key, artifact_checksum_sha256, artifact_size, artifact_content_type, submitted_at, validated_at, approved_at, published_at, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'draft', {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15), mark(16)),
            vec![request_id.clone().into(), command.slug.clone().into(), command.version.clone().into(), command.crate_name.into(), default_locale.clone().into(), command.ownership.into(), command.trust_level.into(), command.license.into(), command.entry_type.into(), command.artifact_origin.as_str().into(), Value::Json(Some(Box::new(command.marketplace))), Value::Json(Some(Box::new(command.ui_packages))), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!([]))))],
        )).await.map_err(store_error)?;
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_publish_request_translations (request_id, locale, name, description, created_at, updated_at) VALUES ({}, {}, {}, {}, {now}, {now})", mark(1), mark(2), mark(3), mark(4)),
            vec![request_id.clone().into(), default_locale.into(), command.name.trim().to_string().into(), command.description.trim().to_string().into()],
        )).await.map_err(store_error)?;
        let details = serde_json::json!({
            "version": command.version,
            "status": "draft",
            "artifact_origin": command.artifact_origin.as_str(),
            "warnings": warnings,
        });
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

    /// Stages one immutable completed platform build for a submitted registry
    /// artifact. The owner reloads the durable build pair under tenant RLS and
    /// binds its source, payload, and OCI receipt identities to this request.
    pub async fn stage_platform_build(
        &self,
        command: ModulePublishPlatformBuildStageCommand,
    ) -> Result<ModulePublishPlatformBuildStageResult, ModuleGovernanceError> {
        command.validate()?;
        let completed = SeaOrmModuleBuildService::new(self.db.clone())
            .load_completed(command.tenant_id, command.build_request_id)
            .await
            .map_err(|_| ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        let component_digest = completed
            .result
            .component_digest
            .as_deref()
            .ok_or(ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        let receipt = completed
            .result
            .publication
            .as_ref()
            .ok_or(ModuleGovernanceError::InvalidPlatformBuildStageCommand)?;
        if !matches!(&completed.result.outcome, ModuleBuildOutcome::Succeeded)
            || completed.request.expected_module_slug.trim().is_empty()
            || completed.request.expected_version.trim().is_empty()
            || receipt_digest_sha256(component_digest).is_err()
            || receipt.artifact.digest != component_digest
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
        }

        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request_lock = if backend == sea_orm::DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, status, artifact_origin, artifact_checksum_sha256 \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let version: String = request.try_get("", "version").map_err(store_error)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        let checksum: Option<String> = request
            .try_get("", "artifact_checksum_sha256")
            .map_err(store_error)?;
        if artifact_origin != ModulePublicationArtifactOrigin::PlatformBuilt.as_str()
            || !matches!(status.as_str(), "submitted" | "validating" | "approved")
            || slug != completed.request.expected_module_slug
            || version != completed.request.expected_version
            || checksum.as_deref() != receipt_digest_sha256(component_digest).ok()
        {
            return Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand);
        }

        let staging_id = format!("rpbs_{}", Uuid::new_v4().simple());
        let inserted = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publish_build_staging \
                     (id, request_id, tenant_id, build_request_id, source_digest, component_digest, \
                      artifact_manifest_digest, sbom_manifest_digest, provenance_manifest_digest, \
                      signature_manifest_digest, staged_by_principal, idempotency_key, staged_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                     ON CONFLICT (request_id, idempotency_key) DO NOTHING",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8),
                    mark(9), mark(10), mark(11), mark(12),
                ),
                vec![
                    staging_id.clone().into(),
                    command.request_id.clone().into(),
                    registry_uuid_value(command.tenant_id, backend),
                    registry_uuid_value(command.build_request_id, backend),
                    completed.request.source.digest.clone().into(),
                    component_digest.to_string().into(),
                    receipt.artifact.digest.clone().into(),
                    receipt.sbom_referrer.digest.clone().into(),
                    receipt.provenance_referrer.digest.clone().into(),
                    receipt.signature_manifest.digest.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    registry_uuid_value(command.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        if inserted.rows_affected() == 0 {
            let existing = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id, CAST(tenant_id AS TEXT) AS tenant_id, \
                         CAST(build_request_id AS TEXT) AS build_request_id, \
                         source_digest, component_digest, \
                         CAST(staged_by_principal AS TEXT) AS staged_by_principal \
                         FROM registry_publish_build_staging \
                         WHERE request_id = {} AND idempotency_key = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        registry_uuid_value(command.idempotency_key, backend),
                    ],
                ))
                .await
                .map_err(store_error)?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store("build-stage conflict lost its row".to_string())
                })?;
            let existing_id: String = existing.try_get("", "id").map_err(store_error)?;
            let existing_tenant_id: String =
                existing.try_get("", "tenant_id").map_err(store_error)?;
            let existing_build_request_id: String = existing
                .try_get("", "build_request_id")
                .map_err(store_error)?;
            let existing_source_digest: String =
                existing.try_get("", "source_digest").map_err(store_error)?;
            let existing_component_digest: String = existing
                .try_get("", "component_digest")
                .map_err(store_error)?;
            let existing_actor: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "staged_by_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            if existing_tenant_id != command.tenant_id.to_string()
                || existing_build_request_id != command.build_request_id.to_string()
                || existing_source_digest != completed.request.source.digest
                || existing_component_digest != component_digest
                || existing_actor != command.actor_principal
            {
                return Err(ModuleGovernanceError::PlatformBuildStageIdempotencyConflict);
            }
            tx.rollback().await.map_err(store_error)?;
            return Ok(ModulePublishPlatformBuildStageResult {
                staging_id: existing_id,
                created: false,
            });
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'platform_build_staged', {}, NULL, {}, {now})",
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
                    "build_request_id": command.build_request_id,
                    "source_digest": completed.request.source.digest,
                    "component_digest": component_digest,
                    "artifact_manifest_digest": receipt.artifact.digest,
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(ModulePublishPlatformBuildStageResult {
            staging_id,
            created: true,
        })
    }

    /// Stages an externally built payload only after the owner has recorded
    /// its provenance-policy decision, source-evidence classification, and a
    /// separate quarantine review. This is intentionally distinct from the
    /// platform build path: it never manufactures a build-worker attestation.
    pub async fn stage_external_prebuilt(
        &self,
        command: ModuleExternalPrebuiltStageCommand,
    ) -> Result<ModuleExternalPrebuiltStageResult, ModuleGovernanceError> {
        command.validate()?;
        let (source_evidence_kind, source_reference, source_digest, source_absence_reason) =
            match &command.source_evidence {
                ModuleExternalSourceEvidence::Reproducible { reference, digest } => (
                    "reproducible",
                    Some(reference.clone()),
                    Some(digest.clone()),
                    None,
                ),
                ModuleExternalSourceEvidence::Unavailable { reason_code } => {
                    ("unavailable", None, None, Some(reason_code.clone()))
                }
            };
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request_lock = if backend == sea_orm::DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, status, artifact_origin, artifact_checksum_sha256 \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        let checksum: Option<String> = request
            .try_get("", "artifact_checksum_sha256")
            .map_err(store_error)?;
        if artifact_origin != ModulePublicationArtifactOrigin::ExternalPrebuilt.as_str()
            || !matches!(status.as_str(), "submitted" | "validating" | "approved")
            || checksum.as_deref() != receipt_digest_sha256(&command.artifact_digest).ok()
        {
            return Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand);
        }

        let staging_id = format!("rpes_{}", Uuid::new_v4().simple());
        let inserted = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publish_external_staging \
                     (id, request_id, artifact_digest, source_evidence_kind, source_reference, \
                      source_digest, source_absence_reason, provenance_reference, provenance_digest, \
                      provenance_policy_revision, quarantine_review_reference, quarantine_policy_revision, \
                      quarantine_approved_by_principal, staged_by_principal, idempotency_key, staged_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                     ON CONFLICT (request_id, idempotency_key) DO NOTHING",
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
                    mark(15),
                ),
                vec![
                    staging_id.clone().into(),
                    command.request_id.clone().into(),
                    command.artifact_digest.clone().into(),
                    source_evidence_kind.into(),
                    source_reference.clone().into(),
                    source_digest.clone().into(),
                    source_absence_reason.clone().into(),
                    command.provenance_reference.clone().into(),
                    command.provenance_digest.clone().into(),
                    command.provenance_policy_revision.clone().into(),
                    command.quarantine_review_reference.clone().into(),
                    command.quarantine_policy_revision.clone().into(),
                    Value::Json(Some(Box::new(
                        command.quarantine_approved_by_principal.clone(),
                    ))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    registry_uuid_value(command.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        if inserted.rows_affected() == 0 {
            let existing = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id, artifact_digest, source_evidence_kind, source_reference, source_digest, \
                         source_absence_reason, provenance_reference, provenance_digest, \
                         provenance_policy_revision, quarantine_review_reference, \
                         quarantine_policy_revision, \
                         CAST(quarantine_approved_by_principal AS TEXT) AS quarantine_approved_by_principal, \
                         CAST(staged_by_principal AS TEXT) AS staged_by_principal \
                         FROM registry_publish_external_staging \
                         WHERE request_id = {} AND idempotency_key = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        registry_uuid_value(command.idempotency_key, backend),
                    ],
                ))
                .await
                .map_err(store_error)?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store("external-stage conflict lost its row".to_string())
                })?;
            let existing_id: String = existing.try_get("", "id").map_err(store_error)?;
            let existing_artifact_digest: String = existing
                .try_get("", "artifact_digest")
                .map_err(store_error)?;
            let existing_source_evidence_kind: String = existing
                .try_get("", "source_evidence_kind")
                .map_err(store_error)?;
            let existing_source_reference: Option<String> = existing
                .try_get("", "source_reference")
                .map_err(store_error)?;
            let existing_source_digest: Option<String> =
                existing.try_get("", "source_digest").map_err(store_error)?;
            let existing_source_absence_reason: Option<String> = existing
                .try_get("", "source_absence_reason")
                .map_err(store_error)?;
            let existing_provenance_reference: String = existing
                .try_get("", "provenance_reference")
                .map_err(store_error)?;
            let existing_provenance_digest: String = existing
                .try_get("", "provenance_digest")
                .map_err(store_error)?;
            let existing_policy_revision: String = existing
                .try_get("", "provenance_policy_revision")
                .map_err(store_error)?;
            let existing_quarantine_reference: String = existing
                .try_get("", "quarantine_review_reference")
                .map_err(store_error)?;
            let existing_quarantine_policy_revision: String = existing
                .try_get("", "quarantine_policy_revision")
                .map_err(store_error)?;
            let existing_quarantine_approver: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "quarantine_approved_by_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let existing_actor: serde_json::Value = serde_json::from_str(
                &existing
                    .try_get::<String>("", "staged_by_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            if existing_artifact_digest != command.artifact_digest
                || existing_source_evidence_kind != source_evidence_kind
                || existing_source_reference != source_reference
                || existing_source_digest != source_digest
                || existing_source_absence_reason != source_absence_reason
                || existing_provenance_reference != command.provenance_reference
                || existing_provenance_digest != command.provenance_digest
                || existing_policy_revision != command.provenance_policy_revision
                || existing_quarantine_reference != command.quarantine_review_reference
                || existing_quarantine_policy_revision != command.quarantine_policy_revision
                || existing_quarantine_approver != command.quarantine_approved_by_principal
                || existing_actor != command.actor_principal
            {
                return Err(ModuleGovernanceError::ExternalPrebuiltStageIdempotencyConflict);
            }
            tx.rollback().await.map_err(store_error)?;
            return Ok(ModuleExternalPrebuiltStageResult {
                staging_id: existing_id,
                created: false,
            });
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'external_prebuilt_staged', {}, NULL, {}, {now})",
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
                    "artifact_digest": command.artifact_digest,
                    "source_evidence_kind": source_evidence_kind,
                    "source_absence_reason": source_absence_reason,
                    "provenance_digest": command.provenance_digest,
                    "provenance_policy_revision": command.provenance_policy_revision,
                    "quarantine_policy_revision": command.quarantine_policy_revision,
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(ModuleExternalPrebuiltStageResult {
            staging_id,
            created: true,
        })
    }

    /// Records one authority-scoped immutable publication fact. This ledger is
    /// intentionally append-only: a later source or artifact can acquire new
    /// evidence, but it never rewrites an earlier author, build, marketplace,
    /// or platform-admission decision.
    pub async fn record_publication_evidence(
        &self,
        command: ModulePublicationEvidenceCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        command.validate()?;
        self.record_publication_evidence_inner(command).await
    }

    /// Records a build-service attestation only after the receipt's OCI
    /// identities and declared signature authority have been validated.
    pub async fn record_build_service_attestation(
        &self,
        command: ModuleBuildServiceAttestationCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        command.validate()?;
        self.record_publication_evidence_inner(command.publication_evidence()?)
            .await
    }

    /// Records an admitted platform trust decision only after its exact OCI
    /// manifest, verified payload, policy revisions, and mandatory verification
    /// outcomes are bound to one immutable evidence fingerprint.
    pub async fn record_platform_admission(
        &self,
        command: ModulePlatformAdmissionCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        command.validate()?;
        let backend = self.db.get_database_backend();
        let artifact_origin = self
            .db
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT artifact_origin FROM registry_publish_requests WHERE id = {}",
                    placeholder(backend, 1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?
            .try_get::<String>("", "artifact_origin")
            .map_err(store_error)?;
        let artifact_origin = ModulePublicationArtifactOrigin::parse(&artifact_origin)
            .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
        self.record_publication_evidence_inner(command.publication_evidence(artifact_origin)?)
            .await
    }

    async fn record_publication_evidence_inner(
        &self,
        command: ModulePublicationEvidenceCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, status FROM registry_publish_requests WHERE id = {}",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        if status == "rejected" {
            return Err(
                ModuleGovernanceError::PublishRequestCannotRecordPublicationEvidence(status),
            );
        }

        let authority = command.authority.as_str();
        let evidence_digest_sha256 = publication_evidence_digest_sha256(&command);
        let evidence_id = format!("rpe_{}", Uuid::new_v4().simple());
        let inserted = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publication_evidence \
                 (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                  issuer_identity, policy_revision, evidence_digest_sha256, \
                  recorded_by_principal, created_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                 ON CONFLICT (request_id, evidence_digest_sha256) DO NOTHING",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                    mark(9),
                ),
                vec![
                    evidence_id.clone().into(),
                    command.request_id.clone().into(),
                    authority.into(),
                    command.subject_digest_sha256.clone().into(),
                    command.evidence_reference.into(),
                    command.issuer_identity.into(),
                    command.policy_revision.clone().into(),
                    evidence_digest_sha256.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(store_error)?;
        if inserted.rows_affected() == 0 {
            let existing = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id FROM registry_publication_evidence \
                         WHERE request_id = {} AND evidence_digest_sha256 = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        evidence_digest_sha256.into(),
                    ],
                ))
                .await
                .map_err(store_error)?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store(
                        "publication evidence conflict did not expose its existing record"
                            .to_string(),
                    )
                })?;
            let evidence_id: String = existing.try_get("", "id").map_err(store_error)?;
            tx.rollback().await.map_err(store_error)?;
            return Ok(ModulePublicationEvidenceResult {
                evidence_id,
                recorded: false,
            });
        }
        tx.execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'publication_evidence_recorded', {}, NULL, {}, {now})",
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
                    "evidence_id": evidence_id.clone(),
                    "authority": authority,
                    "subject_digest_sha256": command.subject_digest_sha256,
                    "policy_revision": command.policy_revision,
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(ModulePublicationEvidenceResult {
            evidence_id,
            recorded: true,
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
        let actor_json = command.actor_principal.clone();
        let stale_predicate = if backend == sea_orm::DbBackend::Postgres {
            format!(
                "started_at <= NOW() - INTERVAL '{} seconds'",
                VALIDATION_JOB_STALE_AFTER_SECONDS
            )
        } else {
            format!(
                "datetime(started_at) <= datetime('now', '-{} seconds')",
                VALIDATION_JOB_STALE_AFTER_SECONDS
            )
        };
        let stale_job = tx.query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id FROM registry_validation_jobs WHERE request_id = {} AND status = 'running' AND {stale_predicate} ORDER BY started_at ASC LIMIT 1",
                mark(1)
            ),
            vec![request_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let recovered_stale_job = if let Some(job) = stale_job {
            let stale_job_id: String = job
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let updated = tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_jobs SET status = 'failed', finished_at = {now}, last_error = 'validation_worker_lease_expired', updated_at = {now} WHERE id = {} AND status = 'running' AND {stale_predicate}",
                    mark(1)
                ),
                vec![stale_job_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if updated.rows_affected() == 1 {
                let details = serde_json::json!({
                    "job_id": stale_job_id,
                    "reason_code": "validation_worker_lease_expired",
                    "lease_timeout_seconds": VALIDATION_JOB_STALE_AFTER_SECONDS,
                });
                tx.execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'validation_job_recovered', {}, NULL, {}, {now})",
                        mark(1), mark(2), mark(3), mark(4), mark(5)
                    ),
                    vec![
                        format!("rge_{}", Uuid::new_v4().simple()).into(),
                        slug.clone().into(),
                        request_id.clone().into(),
                        Value::Json(Some(Box::new(actor_json.clone()))),
                        Value::Json(Some(Box::new(details))),
                    ],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                true
            } else {
                false
            }
        } else {
            false
        };
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
        let queue_reason = if recovered_stale_job {
            "requeued_after_validation_lease_expired"
        } else if status == "validating" {
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
        let events = [
            (
                if recovered_stale_job {
                    "validation_recovered"
                } else if requeued {
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
            format!("SELECT j.status, j.request_id, j.attempt_number, j.queue_reason, r.slug, r.version, r.crate_name, r.ownership, r.trust_level, r.license, r.entry_type, r.marketplace, r.ui_packages, r.validation_warnings, r.status AS request_status, r.artifact_storage_key, r.artifact_checksum_sha256, r.artifact_size, r.artifact_content_type, t.name AS module_name, t.description AS module_description FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id LEFT JOIN registry_publish_request_translations t ON t.request_id = r.id AND t.locale = r.default_locale WHERE j.id = {}", mark(1)),
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
                work_item: None,
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
        let work_item_result =
            (|| -> Result<ModuleValidationJobWorkItem, ModuleGovernanceError> {
                let artifact_storage_key = job
                    .try_get::<Option<String>>("", "artifact_storage_key")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactStorageKey)?;
                let artifact_checksum_sha256 = job
                    .try_get::<Option<String>>("", "artifact_checksum_sha256")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
                if !is_sha256_hex(&artifact_checksum_sha256) {
                    return Err(ModuleGovernanceError::PublishRequestInvalidArtifactChecksum);
                }
                let artifact_size = job
                    .try_get::<Option<i64>>("", "artifact_size")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| *value >= 0)
                    .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactSize)?;
                let artifact_content_type = job
                    .try_get::<Option<String>>("", "artifact_content_type")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::InvalidPublishArtifactAttachCommand)?;
                let crate_name: String = job
                    .try_get("", "crate_name")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                Ok(ModuleValidationJobWorkItem {
                    validation_job_id: command.validation_job_id.clone(),
                    request_id: request_id.clone(),
                    slug: slug.clone(),
                    version: version.clone(),
                    crate_name,
                    artifact_storage_key,
                    artifact_checksum_sha256,
                    artifact_size: artifact_size as u64,
                    artifact_content_type,
                    existing_warnings: validation_warnings_from_row(&job)?,
                    contract: module_publish_validation_contract_from_row(&job)?,
                })
            })();
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
                work_item: None,
            }));
        }
        let work_item = match work_item_result {
            Ok(work_item) => work_item,
            Err(_) => {
                terminalize_invalid_validation_work_item(
                    &tx,
                    backend,
                    &command,
                    &request_id,
                    &slug,
                    &version,
                    attempt_number,
                    &queue_reason,
                )
                .await?;
                tx.commit()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                return Ok(Some(ModuleValidationJobClaimResult {
                    request_id,
                    should_run: false,
                    work_item: None,
                }));
            }
        };
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
            work_item: Some(work_item),
        }))
    }

    /// Claims one oldest queued validation job for an independent worker.
    ///
    /// The initial lookup is intentionally only a candidate selection; the
    /// existing conditional claim remains the authority. A concurrent worker
    /// may win the candidate, in which case this method retries once before
    /// returning no work. The worker never needs an HTTP server callback or a
    /// synthetic tenant-scoped event to find durable queued work.
    pub async fn claim_next_validation_job(
        &self,
        actor_principal: serde_json::Value,
    ) -> Result<Option<ModuleValidationJobClaimResult>, ModuleGovernanceError> {
        let command = ModuleValidationJobClaimCommand {
            validation_job_id: "candidate".to_string(),
            actor_principal: actor_principal.clone(),
        };
        command.validate()?;
        for _ in 0..2 {
            let backend = self.db.get_database_backend();
            let candidate = self
                .db
                .query_one(Statement::from_sql_and_values(
                    backend,
                    "SELECT id FROM registry_validation_jobs WHERE status = 'queued' \
                     ORDER BY created_at ASC, id ASC LIMIT 1"
                        .to_string(),
                    Vec::new(),
                ))
                .await
                .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
            let Some(candidate) = candidate else {
                return Ok(None);
            };
            let validation_job_id: String = candidate
                .try_get("", "id")
                .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
            let claim = self
                .claim_validation_job(ModuleValidationJobClaimCommand {
                    validation_job_id,
                    actor_principal: actor_principal.clone(),
                })
                .await?;
            if claim.as_ref().is_some_and(|claim| claim.should_run) {
                return Ok(claim);
            }
        }
        Ok(None)
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
        let request_lock = if backend == sea_orm::DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        tx.query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id FROM registry_publish_requests WHERE id = {}{request_lock}",
                mark(1)
            ),
            vec![command.request_id.clone().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let command_approval_override = command
            .approval_override
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(store_error)?;
        let existing_operation = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(actor_principal AS TEXT) AS actor_principal, \
                     CAST(publisher_principal AS TEXT) AS publisher_principal, \
                     CAST(allow_owner_rebind AS TEXT) AS allow_owner_rebind, \
                     CAST(approval_override AS TEXT) AS approval_override, release_id \
                     FROM registry_publication_operations \
                     WHERE request_id = {} AND idempotency_key = {}{request_lock}",
                    mark(1),
                    mark(2),
                ),
                vec![
                    command.request_id.clone().into(),
                    registry_uuid_value(command.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        if let Some(operation) = existing_operation {
            let stored_actor: serde_json::Value = serde_json::from_str(
                &operation
                    .try_get::<String>("", "actor_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let stored_publisher: serde_json::Value = serde_json::from_str(
                &operation
                    .try_get::<String>("", "publisher_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let stored_override = operation
                .try_get::<Option<String>>("", "approval_override")
                .map_err(store_error)?
                .map(|value| serde_json::from_str::<serde_json::Value>(&value))
                .transpose()
                .map_err(store_error)?;
            let stored_allow_owner_rebind = operation
                .try_get::<String>("", "allow_owner_rebind")
                .map_err(store_error)?;
            let expected_allow_owner_rebind = if backend == sea_orm::DbBackend::Postgres {
                if command.allow_owner_rebind {
                    "true"
                } else {
                    "false"
                }
            } else if command.allow_owner_rebind {
                "1"
            } else {
                "0"
            };
            if stored_actor != command.actor_principal
                || stored_publisher != command.publisher_principal
                || stored_allow_owner_rebind != expected_allow_owner_rebind
                || stored_override != command_approval_override
            {
                return Err(ModuleGovernanceError::PublicationIdempotencyConflict);
            }
            let release_id: String = operation.try_get("", "release_id").map_err(store_error)?;
            let release_exists = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id FROM registry_module_releases WHERE id = {} LIMIT 1",
                        mark(1)
                    ),
                    vec![release_id.into()],
                ))
                .await
                .map_err(store_error)?
                .is_some();
            if !release_exists {
                return Err(ModuleGovernanceError::PublishedRequestMissingRelease);
            }
            tx.rollback().await.map_err(store_error)?;
            return Ok(());
        }
        let request = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, crate_name, default_locale, ownership, trust_level, license, \
                     entry_type, CAST(marketplace AS TEXT) AS marketplace, \
                     CAST(ui_packages AS TEXT) AS ui_packages, status, artifact_storage_key, \
                     artifact_checksum_sha256, artifact_size, artifact_origin \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
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
        if status == "published" {
            let release_exists = tx
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id FROM registry_module_releases \
                         WHERE request_id = {} AND slug = {} AND version = {} LIMIT 1",
                        mark(1),
                        mark(2),
                        mark(3),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        slug.clone().into(),
                        version.clone().into(),
                    ],
                ))
                .await
                .map_err(store_error)?
                .is_some();
            if !release_exists {
                return Err(ModuleGovernanceError::PublishedRequestMissingRelease);
            }
            tx.rollback().await.map_err(store_error)?;
            return Err(ModuleGovernanceError::PublishedRequestMissingIdempotencyRecord);
        }
        if status != "approved" {
            return Err(ModuleGovernanceError::PublishRequestCannotBePublished(
                status,
            ));
        }
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        let artifact_origin = ModulePublicationArtifactOrigin::parse(&artifact_origin)
            .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
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
        if !is_sha256_hex(&checksum_sha256) {
            return Err(ModuleGovernanceError::PublishRequestInvalidArtifactChecksum);
        }
        let artifact_size = request
            .try_get::<Option<i64>>("", "artifact_size")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| *value >= 0)
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactSize)?;

        match artifact_origin {
            ModulePublicationArtifactOrigin::PlatformBuilt => {
                let platform_build_manifest = tx
                    .query_one(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT artifact_manifest_digest FROM registry_publish_build_staging AS stage \
                             WHERE stage.request_id = {} \
                               AND stage.component_digest = {} \
                               AND stage.staged_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = stage.request_id \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![
                            command.request_id.clone().into(),
                            format!("sha256:{checksum_sha256}").into(),
                        ],
                    ))
                    .await
                    .map_err(store_error)?
                    .map(|row| row.try_get::<String>("", "artifact_manifest_digest"))
                    .transpose()
                    .map_err(store_error)?;
                let Some(platform_build_manifest) = platform_build_manifest else {
                    return Err(ModuleGovernanceError::PublishRequestMissingPlatformBuildStage);
                };
                let platform_build_manifest = receipt_digest_sha256(&platform_build_manifest)
                    .map_err(|_| ModuleGovernanceError::PublishRequestMissingPlatformBuildStage)?;

                let matched_build_and_platform_evidence = tx
                    .query_one(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publication_evidence AS build \
                             WHERE build.request_id = {} \
                               AND build.authority = 'build_service_attestation' \
                               AND build.subject_digest_sha256 = {} \
                               AND build.created_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = build.request_id \
                               ) \
                               AND EXISTS ( \
                                   SELECT 1 FROM registry_publication_evidence AS platform \
                                   WHERE platform.request_id = build.request_id \
                                     AND platform.authority = 'platform_admission' \
                                     AND platform.subject_digest_sha256 = build.subject_digest_sha256 \
                                     AND platform.created_at >= ( \
                                         SELECT request.submitted_at FROM registry_publish_requests AS request \
                                         WHERE request.id = platform.request_id \
                                     ) \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![
                            command.request_id.clone().into(),
                            platform_build_manifest.to_string().into(),
                        ],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !matched_build_and_platform_evidence {
                    return Err(
                        ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission,
                    );
                }
            }
            ModulePublicationArtifactOrigin::ExternalPrebuilt => {
                let external_prebuilt_staged = tx
                    .query_one(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publish_external_staging AS stage \
                             WHERE stage.request_id = {} \
                               AND stage.artifact_digest = {} \
                               AND stage.staged_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = stage.request_id \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![
                            command.request_id.clone().into(),
                            format!("sha256:{checksum_sha256}").into(),
                        ],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !external_prebuilt_staged {
                    return Err(ModuleGovernanceError::PublishRequestMissingExternalPrebuiltStage);
                }
            }
        }

        let author_signature_recorded = tx
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT 1 FROM registry_publication_evidence AS author \
                     WHERE author.request_id = {} AND author.authority = 'author_signature' \
                     AND author.subject_digest_sha256 = {} \
                     AND author.created_at >= ( \
                         SELECT request.submitted_at FROM registry_publish_requests AS request \
                         WHERE request.id = author.request_id \
                     ) \
                     LIMIT 1",
                    mark(1),
                    mark(2),
                ),
                vec![
                    command.request_id.clone().into(),
                    checksum_sha256.clone().into(),
                ],
            ))
            .await
            .map_err(store_error)?
            .is_some();
        if !author_signature_recorded {
            return Err(ModuleGovernanceError::PublishRequestMissingAuthorSignature);
        }
        match artifact_origin {
            ModulePublicationArtifactOrigin::PlatformBuilt => {}
            ModulePublicationArtifactOrigin::ExternalPrebuilt => {
                let platform_admission_recorded = tx
                    .query_one(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publication_evidence AS platform \
                             WHERE platform.request_id = {} \
                               AND platform.authority = 'platform_admission' \
                               AND platform.subject_digest_sha256 = {} \
                               AND platform.created_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = platform.request_id \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![command.request_id.clone().into(), checksum_sha256.clone().into()],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !platform_admission_recorded {
                    return Err(
                        ModuleGovernanceError::PublishRequestMissingExternalPlatformAdmission,
                    );
                }
            }
        }

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

        let marketplace_approval = ModulePublicationEvidenceCommand {
            request_id: command.request_id.clone(),
            authority: ModulePublicationEvidenceAuthority::MarketplaceApproval,
            subject_digest_sha256: checksum_sha256.clone(),
            evidence_reference: format!(
                "registry://publish-requests/{}/marketplace-approval",
                command.request_id
            ),
            issuer_identity: validation_stage_actor_label(&command.actor_principal)?,
            policy_revision: MARKETPLACE_APPROVAL_POLICY_REVISION.to_string(),
            actor_principal: command.actor_principal.clone(),
        };
        let marketplace_approval_digest = publication_evidence_digest_sha256(&marketplace_approval);
        let marketplace_approval_id = format!("rpe_{}", Uuid::new_v4().simple());
        let marketplace_approval_inserted = tx
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publication_evidence \
                 (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                  issuer_identity, policy_revision, evidence_digest_sha256, \
                  recorded_by_principal, created_at) \
                 VALUES ({}, {}, 'marketplace_approval', {}, {}, {}, {}, {}, {}, {now}) \
                 ON CONFLICT (request_id, evidence_digest_sha256) DO NOTHING",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                ),
                vec![
                    marketplace_approval_id.clone().into(),
                    command.request_id.clone().into(),
                    marketplace_approval.subject_digest_sha256.into(),
                    marketplace_approval.evidence_reference.into(),
                    marketplace_approval.issuer_identity.into(),
                    marketplace_approval.policy_revision.clone().into(),
                    marketplace_approval_digest.into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(store_error)?;
        if marketplace_approval_inserted.rows_affected() == 1 {
            tx.execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, NULL, 'marketplace_approval_recorded', {}, NULL, {}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5),
                ),
                vec![
                    format!("rge_{}", Uuid::new_v4().simple()).into(),
                    slug.clone().into(),
                    command.request_id.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(serde_json::json!({
                        "evidence_id": marketplace_approval_id,
                        "authority": "marketplace_approval",
                        "subject_digest_sha256": checksum_sha256,
                        "policy_revision": MARKETPLACE_APPROVAL_POLICY_REVISION,
                    })))),
                ],
            )).await.map_err(store_error)?;
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
                     entry_type = {}, artifact_origin = {}, marketplace = {}, ui_packages = {}, status = 'active', \
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
                    mark(15),
                ),
                vec![
                    command.request_id.clone().into(),
                    crate_name.into(),
                    default_locale.into(),
                    ownership.into(),
                    trust_level.into(),
                    license.into(),
                    entry_type.into(),
                    artifact_origin.as_str().into(),
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
                      license, entry_type, artifact_origin, marketplace, ui_packages, status, publisher_principal, \
                      artifact_storage_key, checksum_sha256, artifact_size, yanked_reason, \
                      yanked_by_principal, yanked_at, published_at, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'active', {}, {}, {}, {}, \
                             NULL, NULL, NULL, {now}, {now}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8),
                    mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15), mark(16), mark(17),
                ),
                vec![
                    release_id.clone().into(), command.request_id.clone().into(), slug.clone().into(),
                    version.clone().into(), crate_name.into(), default_locale.into(), ownership.into(),
                    trust_level.into(), license.into(), entry_type.into(),
                    artifact_origin.as_str().into(), Value::Json(Some(Box::new(marketplace))), Value::Json(Some(Box::new(ui_packages))),
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
                "INSERT INTO registry_publication_operations \
                 (operation_id, request_id, idempotency_key, actor_principal, publisher_principal, \
                  allow_owner_rebind, approval_override, release_id, committed_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
                mark(7),
                mark(8),
            ),
            vec![
                registry_uuid_value(Uuid::new_v4(), backend),
                command.request_id.clone().into(),
                registry_uuid_value(command.idempotency_key, backend),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                command.allow_owner_rebind.into(),
                Value::Json(command_approval_override.clone().map(Box::new)),
                release_id.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?;

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
                    "artifact_origin": artifact_origin.as_str(),
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

async fn terminalize_invalid_validation_work_item(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    command: &ModuleValidationJobClaimCommand,
    request_id: &str,
    slug: &str,
    version: &str,
    attempt_number: i32,
    queue_reason: &str,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| {
        if backend == DbBackend::Postgres {
            format!("${n}")
        } else {
            format!("?{n}")
        }
    };
    let now = if backend == DbBackend::Postgres {
        "NOW()"
    } else {
        "datetime('now')"
    };
    let errors = serde_json::json!([VALIDATION_WORK_ITEM_INVALID_ERROR]);
    let request_updated = tx
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_publish_requests \
                 SET status = 'rejected', validation_errors = {}, \
                     rejected_by_principal = {}, rejection_reason = {}, \
                     validated_at = {now}, approved_by_principal = NULL, \
                     approved_at = NULL, published_at = NULL, updated_at = {now} \
                 WHERE id = {} AND status = 'validating'",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
            ),
            vec![
                Value::Json(Some(Box::new(errors.clone()))).into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))).into(),
                VALIDATION_WORK_ITEM_INVALID_ERROR.to_string().into(),
                request_id.to_string().into(),
            ],
        ))
        .await
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    if request_updated.rows_affected() != 1 {
        return Err(ModuleGovernanceError::ValidationJobRequestStateMismatch(
            "concurrently changed".to_string(),
        ));
    }
    let job_updated = tx
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_validation_jobs \
                 SET status = 'failed', finished_at = {now}, last_error = {}, \
                     updated_at = {now} \
                 WHERE id = {} AND status = 'running'",
                mark(1),
                mark(2),
            ),
            vec![
                VALIDATION_WORK_ITEM_INVALID_ERROR.to_string().into(),
                command.validation_job_id.clone().into(),
            ],
        ))
        .await
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    if job_updated.rows_affected() != 1 {
        return Err(ModuleGovernanceError::ValidationJobNotRunning(
            "concurrently changed".to_string(),
        ));
    }

    let events = [
        (
            "validation_failed",
            serde_json::json!({
                "version": version,
                "status": "rejected",
                "reason": VALIDATION_WORK_ITEM_INVALID_ERROR,
                "errors": errors,
                "automated_checks": [{"check":"delivery_work_item","status":"failed"}],
            }),
        ),
        (
            "validation_job_failed",
            serde_json::json!({
                "job_id": command.validation_job_id.clone(),
                "attempt_number": attempt_number,
                "queue_reason": queue_reason,
                "request_status": "rejected",
                "version": version,
                "error": VALIDATION_WORK_ITEM_INVALID_ERROR,
            }),
        ),
    ];
    for (event_type, details) in events {
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
                slug.to_string().into(),
                request_id.to_string().into(),
                event_type.into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))).into(),
                Value::Json(Some(Box::new(details))).into(),
            ],
        ))
        .await
        .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    }
    Ok(())
}

fn module_publish_validation_contract_from_row(
    row: &QueryResult,
) -> Result<ModulePublishValidationContract, ModuleGovernanceError> {
    let marketplace: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "marketplace")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
    )
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    let ui_packages: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "ui_packages")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
    )
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    let marketplace_category = marketplace
        .get("category")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let marketplace_tags = marketplace
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let ui_crate_name = |surface: &str| {
        ui_packages
            .get(surface)
            .and_then(|package| package.get("crate_name"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    };
    Ok(ModulePublishValidationContract {
        slug: row
            .try_get("", "slug")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        version: row
            .try_get("", "version")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        crate_name: row
            .try_get("", "crate_name")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        module_name: row
            .try_get("", "module_name")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        module_description: row
            .try_get("", "module_description")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        ownership: row
            .try_get("", "ownership")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        trust_level: row
            .try_get("", "trust_level")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        license: row
            .try_get("", "license")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        entry_type: row
            .try_get::<Option<String>>("", "entry_type")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
        marketplace_category,
        marketplace_tags,
        admin_ui_crate_name: ui_crate_name("admin"),
        storefront_ui_crate_name: ui_crate_name("storefront"),
    })
}

fn validation_warnings_from_row(row: &QueryResult) -> Result<Vec<String>, ModuleGovernanceError> {
    let warnings: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "validation_warnings")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?,
    )
    .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
    let mut warnings = warnings
        .as_array()
        .ok_or_else(|| {
            ModuleGovernanceError::Store("validation warnings must be an array".to_string())
        })?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|warning| !warning.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    warnings.sort();
    warnings.dedup();
    Ok(warnings)
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

fn registry_uuid_value(value: Uuid, backend: sea_orm::DbBackend) -> Value {
    if backend == sea_orm::DbBackend::Postgres {
        Value::Uuid(Some(Box::new(value)))
    } else {
        value.to_string().into()
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

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn receipt_subject_digest_sha256(
    receipt: &ModuleBuildPublicationReceipt,
) -> Result<&str, ModuleGovernanceError> {
    receipt_digest_sha256(&receipt.artifact.digest)
}

fn receipt_digest_sha256(digest: &str) -> Result<&str, ModuleGovernanceError> {
    digest
        .strip_prefix("sha256:")
        .filter(|digest| is_sha256_hex(digest))
        .ok_or(ModuleGovernanceError::InvalidBuildServiceAttestationCommand)
}

fn prefixed_sha256_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(is_sha256_hex)
}

fn platform_admission_policy_revision(evidence: &ArtifactVerificationEvidence) -> String {
    format!(
        "trust:{};capability:{}",
        evidence.trust_policy_revision, evidence.capability_policy_revision
    )
}

fn platform_admission_evidence_reference(
    reference: &OciArtifactReference,
    evidence: &ArtifactVerificationEvidence,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.module.platform-admission.v1\0");
    let mut evidence_references = evidence.evidence_references.clone();
    evidence_references.sort();
    for value in [
        reference.canonical(),
        evidence.payload_digest.clone(),
        evidence.media_type.clone(),
        evidence.signer_identity.clone(),
        platform_admission_policy_revision(evidence),
        evidence.signature_verified.to_string(),
        evidence.provenance_verified.to_string(),
        evidence.sbom_verified.to_string(),
        evidence.verified_at.to_rfc3339(),
    ]
    .into_iter()
    .chain(evidence_references)
    {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    format!(
        "platform-admission://{}/evidence/{}",
        reference.canonical(),
        hex::encode(hasher.finalize())
    )
}

fn validate_publication_evidence_fields(
    request_id: &str,
    subject_digest_sha256: &str,
    evidence_reference: &str,
    issuer_identity: &str,
    policy_revision: &str,
    actor_principal: &serde_json::Value,
) -> Result<(), ModuleGovernanceError> {
    if request_id.trim().is_empty()
        || !is_sha256_hex(subject_digest_sha256)
        || evidence_reference.trim().is_empty()
        || evidence_reference.len() > MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES
        || issuer_identity.trim().is_empty()
        || issuer_identity.len() > MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES
        || policy_revision.trim().is_empty()
        || policy_revision.len() > MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES
        || !actor_principal.is_object()
    {
        return Err(ModuleGovernanceError::InvalidPublicationEvidenceCommand);
    }
    Ok(())
}

fn publication_evidence_digest_sha256(command: &ModulePublicationEvidenceCommand) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.module.publication-evidence.v1\0");
    for value in [
        command.authority.as_str(),
        command.subject_digest_sha256.as_str(),
        command.evidence_reference.as_str(),
        command.issuer_identity.as_str(),
        command.policy_revision.as_str(),
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
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
    #[error(
        "publication evidence requires an exact SHA-256 subject, bounded references, and an actor principal"
    )]
    InvalidPublicationEvidenceCommand,
    #[error(
        "marketplace approval, build-service, and platform evidence require their owner-only operations"
    )]
    PublicationEvidenceAuthorityReserved,
    #[error("build-service attestation requires one valid build-worker publication receipt")]
    InvalidBuildServiceAttestationCommand,
    #[error("platform admission requires one admitted immutable verification decision")]
    InvalidPlatformAdmissionCommand,
    #[error(
        "platform build staging requires one matching completed build result and submitted artifact"
    )]
    InvalidPlatformBuildStageCommand,
    #[error("platform build stage idempotency key was reused for different immutable input")]
    PlatformBuildStageIdempotencyConflict,
    #[error(
        "external prebuilt staging requires an approved provenance policy, quarantine review, and explicit source evidence"
    )]
    InvalidExternalPrebuiltStageCommand,
    #[error("external prebuilt stage idempotency key was reused for different immutable input")]
    ExternalPrebuiltStageIdempotencyConflict,
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
    #[error("published registry request has no matching durable release")]
    PublishedRequestMissingRelease,
    #[error("registry publication idempotency key was reused for a different immutable command")]
    PublicationIdempotencyConflict,
    #[error("published registry request has no matching publication idempotency record")]
    PublishedRequestMissingIdempotencyRecord,
    #[error("registry publish request in status `{0}` cannot accept an artifact")]
    PublishRequestCannotAttachArtifact(String),
    #[error("registry publish request in status `{0}` cannot accept publication evidence")]
    PublishRequestCannotRecordPublicationEvidence(String),
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
    #[error("registry publish request artifact checksum is not canonical SHA-256")]
    PublishRequestInvalidArtifactChecksum,
    #[error("registry publish request is missing a valid artifact size")]
    PublishRequestMissingArtifactSize,
    #[error("registry publish request is missing a current platform build stage")]
    PublishRequestMissingPlatformBuildStage,
    #[error("registry publish request is missing a current external prebuilt stage")]
    PublishRequestMissingExternalPrebuiltStage,
    #[error("registry publish request artifact origin is unclassified")]
    PublishRequestArtifactOriginUnclassified,
    #[error(
        "registry publish request is missing author signature evidence for its staged artifact"
    )]
    PublishRequestMissingAuthorSignature,
    #[error(
        "registry publish request is missing matching build-service and platform-admission evidence"
    )]
    PublishRequestMissingBuildOrPlatformAdmission,
    #[error(
        "registry publish request is missing matching platform-admission evidence for its external prebuilt artifact"
    )]
    PublishRequestMissingExternalPlatformAdmission,
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

    fn publish_request_create_command() -> ModulePublishRequestCreateCommand {
        ModulePublishRequestCreateCommand {
            slug: "sample-module".to_string(),
            version: "1.0.0".to_string(),
            crate_name: "sample-module".to_string(),
            default_locale: "en-US".to_string(),
            ownership: "third_party".to_string(),
            trust_level: "reviewed".to_string(),
            license: "MIT".to_string(),
            entry_type: Some("sandboxed".to_string()),
            artifact_origin: ModulePublicationArtifactOrigin::ExternalPrebuilt,
            marketplace: serde_json::json!({ "category": "utilities", "tags": [] }),
            ui_packages: serde_json::json!({ "admin": null, "storefront": null }),
            name: "Sample module".to_string(),
            description: "A publish request description long enough for policy.".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": "publisher" }),
        }
    }

    #[test]
    fn publish_request_contract_accepts_transport_ui_object_and_derives_warnings() {
        let command = publish_request_create_command();

        assert!(command.validate().is_ok());
        assert_eq!(
            command.validation_warnings().expect("owner warnings"),
            vec![
                "No publishable admin/storefront UI packages declared; only backend contract would be published."
                    .to_string(),
                "Third-party publishing requires the configured governance and evidence gates before release."
                    .to_string(),
            ]
        );
    }

    #[test]
    fn publish_request_contract_rejects_noncanonical_locale_and_ui_array() {
        let mut command = publish_request_create_command();
        command.default_locale = "not a locale".to_string();
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand)
        ));

        let mut command = publish_request_create_command();
        command.ui_packages = serde_json::json!([]);
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand)
        ));
    }

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
            idempotency_key: Uuid::new_v4(),
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
    fn external_evidence_cannot_claim_marketplace_approval_or_noncanonical_subjects() {
        let marketplace = ModulePublicationEvidenceCommand {
            request_id: "request-1".to_string(),
            authority: ModulePublicationEvidenceAuthority::MarketplaceApproval,
            subject_digest_sha256: "a".repeat(64),
            evidence_reference: "registry://publish-requests/request-1/marketplace-approval"
                .to_string(),
            issuer_identity: "operator".to_string(),
            policy_revision: "registry-governance-v1".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
        };
        assert!(matches!(
            marketplace.validate(),
            Err(ModuleGovernanceError::PublicationEvidenceAuthorityReserved)
        ));
        let mut build = marketplace;
        build.authority = ModulePublicationEvidenceAuthority::BuildServiceAttestation;
        assert!(matches!(
            build.validate(),
            Err(ModuleGovernanceError::PublicationEvidenceAuthorityReserved)
        ));
        build.authority = ModulePublicationEvidenceAuthority::PlatformAdmission;
        assert!(matches!(
            build.validate(),
            Err(ModuleGovernanceError::PublicationEvidenceAuthorityReserved)
        ));
        assert!(matches!(
            ModulePublishArtifactAttachCommand {
                request_id: "request-1".to_string(),
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                artifact_storage_key: "registry/request-1".to_string(),
                checksum_sha256: "SHA256:ABC".to_string(),
                artifact_size: 1,
                content_type: "application/octet-stream".to_string(),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidPublishArtifactAttachCommand)
        ));
    }

    #[test]
    fn external_prebuilt_stage_requires_explicit_source_evidence() {
        let command = ModuleExternalPrebuiltStageCommand {
            request_id: "request-1".to_string(),
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            source_evidence: ModuleExternalSourceEvidence::Unavailable {
                reason_code: "source_unavailable".to_string(),
            },
            provenance_reference: "https://evidence.example/provenance.json".to_string(),
            provenance_digest: format!("sha256:{}", "b".repeat(64)),
            provenance_policy_revision: "external-provenance-v1".to_string(),
            quarantine_review_reference: "https://reviews.example/quarantine/1".to_string(),
            quarantine_policy_revision: "external-quarantine-v1".to_string(),
            quarantine_approved_by_principal: serde_json::json!({
                "kind": "reviewer",
                "id": "security-operator",
            }),
            idempotency_key: Uuid::new_v4(),
            actor_principal: serde_json::json!({ "kind": "operator", "id": "publisher" }),
        };
        assert!(command.validate().is_ok());

        let mut unclassified_absence = command;
        unclassified_absence.source_evidence = ModuleExternalSourceEvidence::Unavailable {
            reason_code: "not_recorded".to_string(),
        };
        assert!(matches!(
            unclassified_absence.validate(),
            Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand)
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
                yanked_by_principal TEXT NULL, yanked_at TEXT NULL, artifact_storage_key TEXT NULL,\
                checksum_sha256 TEXT NULL, artifact_size INTEGER NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_module_releases (\
                id, request_id, slug, version, publisher_principal, status, artifact_storage_key,\
                checksum_sha256, artifact_size, updated_at\
             ) VALUES (\
                'release-1', 'request-1', 'sample_module', '1.0.0', '{\"subject\":\"publisher\"}', 'active',\
                'registry/sample-1.0.0', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 42, datetime('now')\
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
                "SELECT status, yanked_reason, artifact_storage_key, checksum_sha256, artifact_size \
                 FROM registry_module_releases"
                    .to_string(),
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
        assert_eq!(
            release
                .try_get::<String>("", "artifact_storage_key")
                .expect("artifact storage key"),
            "registry/sample-1.0.0"
        );
        assert_eq!(
            release
                .try_get::<String>("", "checksum_sha256")
                .expect("artifact checksum"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            release
                .try_get::<i64>("", "artifact_size")
                .expect("artifact size"),
            42
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
                license TEXT NOT NULL, entry_type TEXT NULL, artifact_origin TEXT NOT NULL, marketplace TEXT NOT NULL, ui_packages TEXT NOT NULL,\
                status TEXT NOT NULL, artifact_storage_key TEXT NULL, artifact_checksum_sha256 TEXT NULL,\
                artifact_size INTEGER NULL, approved_by_principal TEXT NULL, approved_at TEXT NULL,\
                submitted_at TEXT NULL, published_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_request_translations (\
                request_id TEXT NOT NULL, locale TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,\
                PRIMARY KEY (request_id, locale)\
             )",
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                crate_name TEXT NOT NULL, default_locale TEXT NOT NULL, ownership TEXT NOT NULL,\
                trust_level TEXT NOT NULL, license TEXT NOT NULL, entry_type TEXT NULL, artifact_origin TEXT NOT NULL, marketplace TEXT NOT NULL,\
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
            "CREATE TABLE registry_publication_operations (\
                operation_id TEXT PRIMARY KEY, request_id TEXT NOT NULL, idempotency_key TEXT NOT NULL,\
                actor_principal TEXT NOT NULL, publisher_principal TEXT NOT NULL, allow_owner_rebind INTEGER NOT NULL,\
                approval_override TEXT NULL, release_id TEXT NOT NULL, committed_at TEXT NOT NULL,\
                UNIQUE (request_id, idempotency_key)\
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
            "CREATE TABLE registry_publication_evidence (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, authority TEXT NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, evidence_reference TEXT NOT NULL,\
                issuer_identity TEXT NOT NULL, policy_revision TEXT NOT NULL,\
                evidence_digest_sha256 TEXT NOT NULL, recorded_by_principal TEXT NOT NULL,\
                created_at TEXT NOT NULL, UNIQUE (request_id, evidence_digest_sha256)\
             )",
            "CREATE TABLE registry_publish_build_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, component_digest TEXT NOT NULL,\
                artifact_manifest_digest TEXT NOT NULL,\
                staged_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests (\
                id, slug, version, crate_name, default_locale, ownership, trust_level, license, entry_type,\
                artifact_origin, marketplace, ui_packages, status, artifact_storage_key, artifact_checksum_sha256, artifact_size, submitted_at, updated_at\
             ) VALUES (\
                'request-1', 'sample_module', '1.0.0', 'sample_crate', 'en', 'platform', 'verified', 'MIT',\
                NULL, 'platform_built', '{}', '[]', 'approved', 'registry/request-1', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 42, datetime('now'), datetime('now')\
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

        let command = ModulePublishRequestPublicationCommand {
            request_id: "request-1".to_string(),
            idempotency_key: Uuid::new_v4(),
            actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
            publisher_principal: serde_json::json!({ "kind": "user", "id": "publisher" }),
            allow_owner_rebind: false,
            approval_override: None,
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication without a current platform build stage must fail");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingPlatformBuildStage
        ));
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_build_staging \
                 (id, request_id, component_digest, artifact_manifest_digest, staged_at) VALUES (\
                 'stage-1', 'request-1', \
                 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 datetime('now'))"
                    .to_string(),
            ))
            .await
            .expect("platform build stage fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication without author evidence must fail");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingAuthorSignature
        ));

        let staged_subject = "a".repeat(64);
        let oci_subject = staged_subject.clone();
        let evidence_fixture = |id: &str, authority: &str, subject: &str, created_at: &str| {
            Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!(
                    "INSERT INTO registry_publication_evidence \
                 (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                  issuer_identity, policy_revision, evidence_digest_sha256, \
                  recorded_by_principal, created_at) \
                 VALUES (?, 'request-1', ?, ?, 'evidence://fixture', 'fixture', \
                         'fixture-v1', ?, '{{}}', {created_at})"
                ),
                vec![
                    id.to_string().into(),
                    authority.to_string().into(),
                    subject.to_string().into(),
                    format!("digest-{id}").into(),
                ],
            )
        };
        database
            .execute(evidence_fixture(
                "rpe_author",
                "author_signature",
                staged_subject.as_str(),
                "datetime('now')",
            ))
            .await
            .expect("author evidence fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication without matching build and platform evidence must fail");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission
        ));
        for (id, authority) in [
            ("rpe_build", "build_service_attestation"),
            ("rpe_platform", "platform_admission"),
        ] {
            database
                .execute(evidence_fixture(
                    id,
                    authority,
                    oci_subject.as_str(),
                    "datetime('now')",
                ))
                .await
                .expect("publication evidence fixture");
        }

        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_requests \
                 SET submitted_at = datetime('now', '+1 second') WHERE id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("reupload stage fixture");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_build_staging \
                 (id, request_id, component_digest, artifact_manifest_digest, staged_at) VALUES (\
                 'stage-2', 'request-1', \
                 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 datetime('now', '+2 seconds'))"
                    .to_string(),
            ))
            .await
            .expect("current platform build stage fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("stale publication evidence must not survive a reupload");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingAuthorSignature
        ));
        database
            .execute(evidence_fixture(
                "rpe_author_current",
                "author_signature",
                staged_subject.as_str(),
                "datetime('now', '+2 seconds')",
            ))
            .await
            .expect("current author evidence fixture");
        for (id, authority) in [
            ("rpe_build_current", "build_service_attestation"),
            ("rpe_platform_current", "platform_admission"),
        ] {
            database
                .execute(evidence_fixture(
                    id,
                    authority,
                    oci_subject.as_str(),
                    "datetime('now', '+2 seconds')",
                ))
                .await
                .expect("current publication evidence fixture");
        }
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_build_staging \
                 SET artifact_manifest_digest = \
                 'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' \
                 WHERE id = 'stage-2'"
                    .to_string(),
            ))
            .await
            .expect("mismatched build manifest fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication evidence must match the staged OCI manifest");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission
        ));
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_build_staging \
                 SET artifact_manifest_digest = \
                 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' \
                 WHERE id = 'stage-2'"
                    .to_string(),
            ))
            .await
            .expect("matching build manifest fixture");

        service
            .publish_request(command.clone())
            .await
            .expect("publish request");
        service
            .publish_request(command.clone())
            .await
            .expect("published request replay");
        let mut conflicting_replay = command;
        conflicting_replay.publisher_principal = serde_json::json!({
            "kind": "user",
            "id": "different-publisher",
        });
        let conflict = service
            .publish_request(conflicting_replay)
            .await
            .expect_err("idempotency key must bind the immutable publication command");
        assert!(matches!(
            conflict,
            ModuleGovernanceError::PublicationIdempotencyConflict
        ));

        let release_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_module_releases".to_string(),
            ))
            .await
            .expect("release count query")
            .expect("release count row");
        assert_eq!(
            release_count
                .try_get::<i64>("", "count")
                .expect("release count"),
            1
        );
        let publication_operation_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_publication_operations".to_string(),
            ))
            .await
            .expect("publication operation count query")
            .expect("publication operation count row");
        assert_eq!(
            publication_operation_count
                .try_get::<i64>("", "count")
                .expect("publication operation count"),
            1
        );

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
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
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
        assert_eq!(events.len(), 3);
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
        assert!(event_types
            .iter()
            .any(|event_type| event_type == "marketplace_approval_recorded"));
        let evidence = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT authority, subject_digest_sha256 FROM registry_publication_evidence"
                    .to_string(),
            ))
            .await
            .expect("marketplace approval evidence query")
            .expect("marketplace approval evidence row");
        assert_eq!(
            evidence
                .try_get::<String>("", "authority")
                .expect("marketplace approval authority"),
            "marketplace_approval"
        );
        assert_eq!(
            evidence
                .try_get::<String>("", "subject_digest_sha256")
                .expect("marketplace approval subject"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[tokio::test]
    async fn publication_evidence_is_authority_scoped_and_idempotent() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, status TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publication_evidence (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, authority TEXT NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, evidence_reference TEXT NOT NULL,\
                issuer_identity TEXT NOT NULL, policy_revision TEXT NOT NULL,\
                evidence_digest_sha256 TEXT NOT NULL, recorded_by_principal TEXT NOT NULL,\
                created_at TEXT NOT NULL, UNIQUE (request_id, evidence_digest_sha256)\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests (id, slug, status) \
             VALUES ('request-1', 'sample_module', 'approved')",
        ] {
            database
                .execute(Statement::from_string(DbBackend::Sqlite, statement.to_string()))
                .await
                .expect("schema or fixture");
        }

        let command = ModulePublicationEvidenceCommand {
            request_id: "request-1".to_string(),
            authority: ModulePublicationEvidenceAuthority::AuthorSignature,
            subject_digest_sha256: "a".repeat(64),
            evidence_reference: "oci://registry.example/modules/sample@sha256:author-signature"
                .to_string(),
            issuer_identity: "author:sample".to_string(),
            policy_revision: "author-policy-v1".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": "author" }),
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let first = service
            .record_publication_evidence(command.clone())
            .await
            .expect("record evidence");
        let repeated = service
            .record_publication_evidence(command)
            .await
            .expect("repeat evidence");
        let reference = |digest: char| crate::installation::OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample".to_string(),
            digest: format!("sha256:{}", digest.to_string().repeat(64)),
        };
        let build_command = ModuleBuildServiceAttestationCommand {
            request_id: "request-1".to_string(),
            receipt: ModuleBuildPublicationReceipt {
                artifact: reference('a'),
                sbom_referrer: reference('b'),
                provenance_referrer: reference('c'),
                signature_manifest: reference('d'),
                signature_authority: ModuleBuildSignatureAuthority::BuildService,
            },
            issuer_identity: "build-service:production".to_string(),
            policy_revision: "build-policy-v1".to_string(),
            actor_principal: serde_json::json!({ "kind": "service", "id": "build-worker" }),
        };
        let build = service
            .record_build_service_attestation(build_command.clone())
            .await
            .expect("record build evidence");
        let platform_admission = ModulePlatformAdmissionCommand {
            request_id: "request-1".to_string(),
            reference: reference('a'),
            evidence: ArtifactVerificationEvidence {
                manifest_digest: format!("sha256:{}", "a".repeat(64)),
                payload_digest: format!("sha256:{}", "e".repeat(64)),
                media_type: "application/wasm".to_string(),
                signer_identity: "build-service:production".to_string(),
                trust_policy_revision: 7,
                capability_policy_revision: 9,
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                evidence_references: vec![
                    "oci://registry.example/modules/sample@sha256:signature".to_string(),
                    "oci://registry.example/modules/sample@sha256:provenance".to_string(),
                ],
                verified_at: chrono::Utc::now(),
            },
            actor_principal: serde_json::json!({ "kind": "service", "id": "verification-worker" }),
        };
        assert_eq!(
            platform_admission
                .publication_evidence(ModulePublicationArtifactOrigin::ExternalPrebuilt)
                .expect("external admission evidence")
                .subject_digest_sha256,
            "e".repeat(64)
        );
        let admission = service
            .record_platform_admission(platform_admission.clone())
            .await
            .expect("record platform admission");
        let repeated_admission = service
            .record_platform_admission(platform_admission)
            .await
            .expect("repeat platform admission");

        assert!(first.recorded);
        assert!(!repeated.recorded);
        assert_eq!(first.evidence_id, repeated.evidence_id);
        assert!(build.recorded);
        assert_ne!(first.evidence_id, build.evidence_id);
        assert!(admission.recorded);
        assert!(!repeated_admission.recorded);
        assert_eq!(admission.evidence_id, repeated_admission.evidence_id);
        assert_ne!(build.evidence_id, admission.evidence_id);
        let evidence = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT authority, subject_digest_sha256 FROM registry_publication_evidence"
                    .to_string(),
            ))
            .await
            .expect("evidence query");
        assert_eq!(evidence.len(), 3);
        let authorities = evidence
            .iter()
            .map(|row| row.try_get::<String>("", "authority").expect("authority"))
            .collect::<Vec<_>>();
        assert!(authorities.contains(&"author_signature".to_string()));
        assert!(authorities.contains(&"build_service_attestation".to_string()));
        assert!(authorities.contains(&"platform_admission".to_string()));
        for row in evidence {
            assert_eq!(
                row.try_get::<String>("", "subject_digest_sha256")
                    .expect("subject digest"),
                "a".repeat(64)
            );
        }
    }
}
