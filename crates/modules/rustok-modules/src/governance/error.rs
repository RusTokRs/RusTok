//! Governance error types and categories.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Transport-neutral classification for the canonical module-governance error
/// contract. Hosts map this category to their own envelopes without recreating
/// the owner lifecycle taxonomy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleGovernanceErrorCategory {
    InvalidInput,
    PermissionDenied,
    NotFound,
    Conflict,
    Internal,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleGovernanceError {
    #[error("registry lifecycle query requires a module slug")]
    InvalidLifecycleQuery,
    #[error("registry owner-binding query requires a module slug")]
    InvalidOwnerBindingQuery,
    #[error("registry publish-artifact download query requires a request ID")]
    InvalidPublishArtifactDownloadQuery,
    #[error("registry publish-request status query requires a request ID")]
    InvalidPublishRequestStatusQuery,
    #[error("registry publish request has an unsupported status '{0}'")]
    InvalidPublishRequestStatus(String),
    #[error("registry publish request identity conflicts with its immutable create command")]
    PublishRequestCreationConflict,
    #[error("registry lifecycle contains unsupported artifact origin `{0}`")]
    InvalidLifecycleArtifactOrigin(String),
    #[error("release yank requires slug, version, reason, and actor principal")]
    InvalidYankCommand,
    #[error("unsupported release yank reason code `{0}`")]
    InvalidYankReasonCode(String),
    #[error("owner transfer requires slug, owner, actor, and reason")]
    InvalidOwnerTransferCommand,
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
    #[error("validation-stage report requires request ID, stage key, known status, and actor")]
    InvalidValidationStageReportCommand,
    #[error(
        "registry validation-job enqueue idempotency key was reused for a different immutable command"
    )]
    ValidationJobEnqueueIdempotencyConflict,
    #[error(
        "validation stage `{stage_key}` is not required for artifact origin `{artifact_origin}`"
    )]
    ValidationStageNotRequiredForArtifactOrigin {
        stage_key: String,
        artifact_origin: String,
    },
    #[error("owner-evidence validation stage `{0}` cannot be changed by a manual report")]
    OwnerEvidenceValidationStageCannotBeReported(String),
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
    #[error("validation job result requires a job ID, actor, coherent outcome, and check evidence")]
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
        "author-signature evidence requires a platform command context and bounded signature facts"
    )]
    InvalidAuthorSignatureEvidenceCommand,
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
    #[error("platform publication-evidence request is invalid")]
    InvalidPlatformPublicationEvidenceRequest,
    #[error("Alloy publication-evidence request is invalid")]
    InvalidAlloyPublicationEvidenceRequest,
    #[error(
        "current platform publication source is unavailable or no longer matches its completed build"
    )]
    PlatformPublicationEvidenceSourceUnavailable,
    #[error(
        "current Alloy publication source is unavailable or no longer matches its immutable stage receipt"
    )]
    AlloyPublicationEvidenceSourceUnavailable,
    #[error(
        "external prebuilt staging requires an approved provenance policy, quarantine review, and explicit source evidence"
    )]
    InvalidExternalPrebuiltStageCommand,
    #[error("external prebuilt stage idempotency key was reused for different immutable input")]
    ExternalPrebuiltStageIdempotencyConflict,
    #[error(
        "Alloy-authored staging requires an immutable source revision and approved review evidence"
    )]
    InvalidAlloyAuthoredStageCommand,
    #[error("Alloy-authored stage idempotency key was reused for different immutable input")]
    AlloyAuthoredStageIdempotencyConflict,
    #[error(
        "actor is not authorized to stage an Alloy-authored artifact for this registry publish request"
    )]
    PublishRequestAlloyAuthoredStagingUnauthorized,
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
    #[error("actor is not authorized to yank this published release")]
    ReleaseYankUnauthorized,
    #[error("registry release-yank idempotency key was reused for a different immutable command")]
    ReleaseYankIdempotencyConflict,
    #[error("published release in status `{0}` cannot be yanked")]
    ReleaseCannotBeYanked(String),
    #[error("registry owner binding was not found")]
    OwnerBindingNotFound,
    #[error("actor is not authorized to transfer this registry owner binding")]
    OwnerTransferUnauthorized,
    #[error("registry owner-transfer idempotency key was reused for a different immutable command")]
    OwnerTransferIdempotencyConflict,
    #[error("registry owner is already bound to the requested principal")]
    OwnerUnchanged,
    #[error("registry owner is already bound to a different principal")]
    OwnerAlreadyBound,
    #[error("registry publish request was not found")]
    PublishRequestNotFound,
    #[error("actor is not authorized to create this registry publish request")]
    PublishRequestCreationUnauthorized,
    #[error("actor is not authorized to stage a platform build for this registry publish request")]
    PublishRequestPlatformBuildStagingUnauthorized,
    #[error(
        "actor is not authorized to record author-signature evidence for this registry publish request"
    )]
    PublishRequestAuthorSignatureUnauthorized,
    #[error(
        "actor is not authorized to stage an external prebuilt artifact for this registry publish request"
    )]
    PublishRequestExternalPrebuiltStagingUnauthorized,
    #[error("published release `{slug}@{version}` already exists")]
    PublishRequestReleaseAlreadyActive { slug: String, version: String },
    #[error("registry publish request revision conflict: expected {expected}, current {current}")]
    PublishRequestRevisionConflict { expected: i64, current: i64 },
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
    #[error(
        "registry publish-request review idempotency key was reused for a different immutable command"
    )]
    PublishRequestReviewIdempotencyConflict,
    #[error(
        "registry validation-stage report idempotency key was reused for a different immutable command"
    )]
    ValidationStageReportIdempotencyConflict,
    #[error("published registry request has no matching publication idempotency record")]
    PublishedRequestMissingIdempotencyRecord,
    #[error("registry publish request in status `{0}` cannot accept an artifact")]
    PublishRequestCannotAttachArtifact(String),
    #[error("actor is not authorized to upload an artifact for this registry publish request")]
    PublishRequestArtifactUploadUnauthorized,
    #[error(
        "registry publish artifact replay does not match the already attached immutable artifact"
    )]
    PublishRequestArtifactReplayConflict,
    #[error(
        "registry publish-artifact idempotency key was reused for a different immutable command"
    )]
    PublishRequestArtifactIdempotencyConflict,
    #[error(
        "registry author-signature idempotency key was reused for a different immutable command"
    )]
    AuthorSignatureEvidenceIdempotencyConflict,
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
    #[error("registry publish request is missing a current reviewed Alloy-authored stage")]
    PublishRequestMissingAlloyAuthoredStage,
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
    #[error(
        "registry publish request is missing matching platform-admission evidence for its Alloy-authored artifact"
    )]
    PublishRequestMissingAlloyPlatformAdmission,
    #[error(
        "registry publish request is missing a complete immutable marketplace artifact contract"
    )]
    PublishRequestMissingCanonicalArtifactContract,
    #[error("registry publish request has no localized metadata")]
    PublishRequestMissingTranslations,
    #[error("registry publish request localized metadata violates the locale contract")]
    PublishRequestInvalidTranslations,
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

impl ModuleGovernanceError {
    /// Classifies an owner failure without exposing persistence or lifecycle
    /// details to transport adapters. Unrecognized future variants fail closed
    /// as internal errors until their public classification is explicit.
    pub const fn category(&self) -> ModuleGovernanceErrorCategory {
        match self {
            Self::InvalidLifecycleQuery
            | Self::InvalidOwnerBindingQuery
            | Self::InvalidPublishArtifactDownloadQuery
            | Self::InvalidPublishRequestStatusQuery
            | Self::InvalidYankCommand
            | Self::InvalidYankReasonCode(_)
            | Self::InvalidOwnerTransferCommand
            | Self::InvalidPublishRequestRejectCommand
            | Self::InvalidPublishRequestChangesCommand
            | Self::InvalidPublishRequestHoldCommand
            | Self::InvalidPublishRequestResumeCommand
            | Self::InvalidPublishRequestPublicationCommand
            | Self::InvalidPublishApprovalOverride
            | Self::InvalidValidationStageReportCommand
            | Self::ValidationStageNotRequiredForArtifactOrigin { .. }
            | Self::OwnerEvidenceValidationStageCannotBeReported(_)
            | Self::InvalidValidationStageRequeue
            | Self::InvalidRemoteValidationLeaseCommand
            | Self::InvalidValidationJobEnqueueCommand
            | Self::InvalidValidationJobClaimCommand
            | Self::InvalidValidationJobResultCommand
            | Self::InvalidValidationJobRetryCommand
            | Self::InvalidPublishRequestCreateCommand
            | Self::InvalidPublishArtifactAttachCommand
            | Self::InvalidPublicationEvidenceCommand
            | Self::InvalidAuthorSignatureEvidenceCommand
            | Self::InvalidPlatformPublicationEvidenceRequest
            | Self::InvalidAlloyPublicationEvidenceRequest
            | Self::InvalidBuildServiceAttestationCommand
            | Self::InvalidPlatformAdmissionCommand
            | Self::InvalidPlatformBuildStageCommand
            | Self::InvalidExternalPrebuiltStageCommand
            | Self::InvalidAlloyAuthoredStageCommand
            | Self::InvalidRemoteValidationClaimStage(_)
            | Self::InvalidOwnerTransferReasonCode(_)
            | Self::InvalidPublishRequestRejectReasonCode(_)
            | Self::InvalidPublishRequestChangesReasonCode(_)
            | Self::InvalidPublishRequestHoldReasonCode(_)
            | Self::InvalidPublishRequestResumeReasonCode(_)
            | Self::InvalidPublishApprovalOverrideReasonCode(_)
            | Self::InvalidValidationStageReasonCode(_) => {
                ModuleGovernanceErrorCategory::InvalidInput
            }
            Self::PublishRequestCreationUnauthorized
            | Self::PublishRequestPlatformBuildStagingUnauthorized
            | Self::PublishRequestAuthorSignatureUnauthorized
            | Self::PublishRequestAlloyAuthoredStagingUnauthorized
            | Self::PublishRequestExternalPrebuiltStagingUnauthorized
            | Self::ReleaseYankUnauthorized
            | Self::OwnerTransferUnauthorized
            | Self::PublishRequestArtifactUploadUnauthorized
            | Self::RemoteValidationLeaseRunnerMismatch => {
                ModuleGovernanceErrorCategory::PermissionDenied
            }
            Self::ReleaseNotFound
            | Self::OwnerBindingNotFound
            | Self::PublishRequestNotFound
            | Self::ValidationJobNotFound
            | Self::ValidationStageNotFound
            | Self::RemoteValidationLeaseNotFound => ModuleGovernanceErrorCategory::NotFound,
            Self::OwnerUnchanged
            | Self::OwnerAlreadyBound
            | Self::OwnerTransferIdempotencyConflict
            | Self::ReleaseYankIdempotencyConflict
            | Self::ReleaseCannotBeYanked(_)
            | Self::PublishRequestCreationConflict
            | Self::PlatformPublicationEvidenceSourceUnavailable
            | Self::AlloyPublicationEvidenceSourceUnavailable
            | Self::PublishRequestReleaseAlreadyActive { .. }
            | Self::PublishRequestRevisionConflict { .. }
            | Self::PublishRequestCannotBeRejected(_)
            | Self::PublishRequestCannotRequestChanges(_)
            | Self::PublishRequestCannotBeHeld(_)
            | Self::PublishRequestCannotBeResumed(_)
            | Self::PublishRequestCannotBePublished(_)
            | Self::PublishedRequestMissingRelease
            | Self::PublishRequestCannotAttachArtifact(_)
            | Self::PublishRequestArtifactReplayConflict
            | Self::PublishRequestArtifactIdempotencyConflict
            | Self::AuthorSignatureEvidenceIdempotencyConflict
            | Self::PublishRequestCannotRecordPublicationEvidence(_)
            | Self::PublishRequestCannotReportValidationStage(_)
            | Self::PublishRequestCannotQueueValidation(_)
            | Self::ValidationJobNotRunning(_)
            | Self::ValidationJobRequestStateMismatch(_)
            | Self::PublishRequestMissingArtifactStorageKey
            | Self::PublishRequestMissingArtifactChecksum
            | Self::PublishRequestInvalidArtifactChecksum
            | Self::PublishRequestMissingArtifactSize
            | Self::PublishRequestMissingPlatformBuildStage
            | Self::PublishRequestMissingExternalPrebuiltStage
            | Self::PublishRequestMissingAlloyAuthoredStage
            | Self::PublishRequestArtifactOriginUnclassified
            | Self::PublishRequestMissingAuthorSignature
            | Self::PublishRequestMissingCanonicalArtifactContract
            | Self::PublishRequestMissingBuildOrPlatformAdmission
            | Self::PublishRequestMissingExternalPlatformAdmission
            | Self::PublishRequestMissingAlloyPlatformAdmission
            | Self::PublishRequestMissingTranslations
            | Self::PublishRequestInvalidTranslations
            | Self::RemoteValidationLeaseNotRunning(_)
            | Self::RemoteValidationLeaseExpired
            | Self::InvalidValidationStageTransition { .. }
            | Self::ValidationJobEnqueueIdempotencyConflict
            | Self::PublishRequestInvalidHeldFromStatus
            | Self::PlatformBuildStageIdempotencyConflict
            | Self::ExternalPrebuiltStageIdempotencyConflict
            | Self::AlloyAuthoredStageIdempotencyConflict
            | Self::PublicationIdempotencyConflict
            | Self::PublishRequestReviewIdempotencyConflict
            | Self::ValidationStageReportIdempotencyConflict
            | Self::PublishedRequestMissingIdempotencyRecord => {
                ModuleGovernanceErrorCategory::Conflict
            }
            Self::Store(_)
            | Self::InvalidLifecycleArtifactOrigin(_)
            | Self::InvalidPublishRequestStatus(_) => ModuleGovernanceErrorCategory::Internal,
        }
    }

    /// Stable owner-issued code for transport envelopes. The detail remains the
    /// error display text where the selected category permits exposing it.
    pub const fn code(&self) -> &'static str {
        match self.category() {
            ModuleGovernanceErrorCategory::InvalidInput => "module_governance_invalid_input",
            ModuleGovernanceErrorCategory::PermissionDenied => {
                "module_governance_permission_denied"
            }
            ModuleGovernanceErrorCategory::NotFound => "module_governance_not_found",
            ModuleGovernanceErrorCategory::Conflict => "module_governance_conflict",
            ModuleGovernanceErrorCategory::Internal => "module_governance_internal",
        }
    }
}
