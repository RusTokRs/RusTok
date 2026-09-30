//! Governance commands, outcomes, publication sources, and artifact types.

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

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

/// The origin is immutable release provenance, not a caller-selected trust
/// level. An unclassified durable request is intentionally not promotable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]

pub enum ModulePublicationArtifactOrigin {
    PlatformBuilt,
    ExternalPrebuilt,
    AlloyAuthored,
}

impl ModulePublicationArtifactOrigin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PlatformBuilt => "platform_built",
            Self::ExternalPrebuilt => "external_prebuilt",
            Self::AlloyAuthored => "alloy_authored",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "platform_built" => Some(Self::PlatformBuilt),
            "external_prebuilt" => Some(Self::ExternalPrebuilt),
            "alloy_authored" => Some(Self::AlloyAuthored),
            _ => None,
        }
    }
}

pub(crate) fn publication_follow_up_stages(
    origin: ModulePublicationArtifactOrigin,
) -> &'static [PublicationFollowUpStage] {
    match origin {
        ModulePublicationArtifactOrigin::PlatformBuilt => PLATFORM_BUILT_FOLLOW_UP_STAGES,
        ModulePublicationArtifactOrigin::ExternalPrebuilt => EXTERNAL_PREBUILT_FOLLOW_UP_STAGES,
        ModulePublicationArtifactOrigin::AlloyAuthored => ALLOY_AUTHORED_FOLLOW_UP_STAGES,
    }
}

pub(crate) fn publication_follow_up_stage(
    origin: ModulePublicationArtifactOrigin,
    stage_key: &str,
) -> Option<PublicationFollowUpStage> {
    publication_follow_up_stages(origin)
        .iter()
        .copied()
        .find(|stage| stage.key == stage_key)
}

/// Immutable source-evidence classification for an external prebuilt
/// artifact. `Unavailable` is explicit durable evidence, not a default.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModuleExternalSourceEvidence {
    Reproducible { reference: String, digest: String },
    Unavailable { reason_code: String },
}

/// Owner-authenticated immutable external-artifact staging. The command is
/// platform-scoped because it mutates the global registry aggregate, while the
/// owner records the approved provenance policy and quarantine review.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleExternalPrebuiltStageCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub artifact_digest: String,
    pub source_evidence: ModuleExternalSourceEvidence,
    pub provenance_reference: String,
    pub provenance_digest: String,
    pub provenance_policy_revision: String,
    pub quarantine_review_reference: String,
    pub quarantine_policy_revision: String,
    pub quarantine_approved_by_principal: serde_json::Value,
    pub actor_principal: serde_json::Value,
    /// An authenticated platform privilege required for the external-artifact
    /// quarantine decision.
    pub actor_can_manage_modules: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleExternalPrebuiltStageResult {
    pub staging_id: String,
    pub created: bool,
    /// Current publish-request aggregate revision after this command.
    pub request_revision: i64,
}

/// Owner-authenticated immutable staging of a reviewed Alloy source revision.
/// Alloy supplies source and review facts; the registry owner persists the
/// publication gate and remains the sole marketplace state writer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleAlloyAuthoredStageCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    /// Authenticated host fact for `modules:manage`. The owner combines it
    /// with the current request and owner binding while holding their locks.
    pub actor_can_manage_modules: bool,
    pub alloy_tenant_id: Uuid,
    pub alloy_script_id: Uuid,
    pub artifact_digest: String,
    pub source_digest: String,
    pub source_revision: u32,
    /// Descriptor finalized from the exact reviewed canonical Rhai workspace.
    /// The owner stores this immutable declaration with the stage receipt and
    /// later requires platform admission to carry the same descriptor.
    pub descriptor: crate::ModuleArtifactDescriptor,
    /// Immutable marketplace release from which this Alloy draft was imported.
    /// The optional lineage is owned and verified by the module publication
    /// transaction; Alloy never writes a marketplace release directly.
    pub parent_release: Option<crate::ArtifactReleaseRef>,
    pub review_reference: String,
    pub review_digest: String,
    pub review_policy_revision: String,
    pub reviewed_by_principal: serde_json::Value,
    pub sandbox_execution_id: Uuid,
    pub sandbox_test_path: String,
    /// Digest of the canonical capability-free smoke scenario. A future
    /// Rust/WASM evolution candidate must compare against this exact immutable
    /// scenario rather than an ad-hoc local test input.
    pub sandbox_scenario_digest: String,
    pub sandbox_executor: String,
    pub sandbox_runtime_abi: String,
    pub sandbox_policy_digest: String,
    pub sandbox_capability_grants: u32,
    pub actor_principal: serde_json::Value,
}

/// Returns the domain-separated digest of the fixed Alloy publication smoke
/// scenario. The scenario deliberately has one exact test path, a null input,
/// a successful boolean result, and no capability grants, so it can become the
/// first immutable parity case for a later Rust/WASM rewrite.
pub fn alloy_publication_smoke_scenario_digest() -> String {
    #[derive(Serialize)]
    struct PublicationSmokeScenario<'a> {
        scenario_id: &'a str,
        test_path: &'a str,
        input: (),
        expected_return: bool,
        capability_grants: u32,
    }

    let scenario = PublicationSmokeScenario {
        scenario_id: "alloy.publication-smoke",
        test_path: ALLOY_PUBLICATION_SMOKE_TEST_PATH,
        input: (),
        expected_return: true,
        capability_grants: 0,
    };
    let bytes = serde_json::to_vec(&scenario).expect("publication smoke scenario must serialize");
    let mut hasher = Sha256::new();
    hasher.update(ALLOY_PUBLICATION_SMOKE_SCENARIO_DOMAIN);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAlloyAuthoredStageResult {
    pub staging_id: String,
    pub created: bool,
    /// Current publish-request aggregate revision after this command.
    pub request_revision: i64,
}

/// Metadata of bytes that a host has hashed before asking the owner for an
/// upload slot and then attaching the artifact. The owner derives the storage
/// key from the checksum; a host can never select an object location.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishArtifactAttachCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub actor_principal: serde_json::Value,
    /// An authenticated host fact. It narrows the owner authorization decision
    /// but never replaces owner binding or requester identity checks.
    pub actor_can_manage_modules: bool,
    pub checksum_sha256: String,
    pub artifact_size: i64,
    pub content_type: String,
}

/// Owner-issued, content-addressed upload destination for one exact artifact
/// checksum. Replays never overwrite bytes: an already attached slot may be
/// reused only when all immutable artifact metadata matches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGovernancePublishArtifactUploadSlot {
    pub request_id: String,
    pub artifact_storage_key: String,
    pub artifact_already_attached: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModulePublishArtifactAttachResult {
    pub request_id: String,
    pub artifact_storage_key: String,
    pub previous_storage_key: Option<String>,
    pub reuploaded_after_changes_requested: bool,
}

/// Distinct authorities whose evidence can be attached to a staged release.
/// These facts deliberately do not imply one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModulePublicationEvidenceAuthority {
    AuthorSignature,
    BuildServiceAttestation,
    MarketplaceApproval,
    PlatformAdmission,
}

impl ModulePublicationEvidenceAuthority {
    pub(crate) fn as_str(self) -> &'static str {
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
pub(crate) struct ModulePublicationEvidenceCommand {
    pub request_id: String,
    /// Revision of the publish-request aggregate observed before recording a
    /// new immutable fact. An exact evidence replay is accepted independently
    /// of this value and returns the currently locked aggregate revision.
    pub expected_revision: i64,
    pub authority: ModulePublicationEvidenceAuthority,
    pub subject_digest_sha256: String,
    pub evidence_reference: String,
    pub issuer_identity: String,
    pub policy_revision: String,
    /// The immutable digest of an author signature. Only author-signature
    /// evidence carries this fact; other authorities have distinct evidence
    /// contracts and retain `None`.
    pub signature_digest_sha256: Option<String>,
    pub actor_principal: serde_json::Value,
}

/// Authenticated recording of an author signature for the current staged
/// publish artifact. The owner derives the signed payload digest from the
/// locked request; a transport cannot choose a different evidence subject.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleAuthorSignatureEvidenceCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    /// An authenticated host fact for the platform-wide `modules:manage`
    /// permission. The owner combines it with the current request and owner
    /// binding while holding the aggregate lock; it never trusts a transport
    /// preflight as authorization for the durable write.
    pub actor_can_manage_modules: bool,
    pub evidence_reference: String,
    pub signature_digest_sha256: String,
    pub signer_identity: String,
    pub policy_revision: String,
    pub actor_principal: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublicationEvidenceResult {
    pub evidence_id: String,
    pub recorded: bool,
    /// Current publish-request aggregate revision. A newly recorded fact
    /// advances it once; an exact idempotent replay returns it unchanged.
    pub request_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublishedArtifactContract {
    pub release_id: String,
    pub artifact: crate::ModuleMarketplaceArtifactRelease,
    pub descriptor: crate::ModuleArtifactDescriptor,
    /// Owner-persisted source lineage for this exact published release.
    /// It is separate from marketplace metadata so imports and later forks can
    /// retain their immutable predecessor without trusting a catalog DTO.
    pub lineage: crate::ArtifactSourceLineage,
    /// Exact payload representation admitted before this release became
    /// available. Consumers must not infer a Rhai representation from the
    /// payload kind alone.
    pub payload_media_type: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
}

/// Immutable Rhai workspace materialized by the registry owner from a
/// canonical active-release projection and digest-pinned artifact CAS. This
/// is intentionally not a marketplace DTO: it carries the executable source
/// only after the owner has validated its release, media type, bytes, and
/// lineage identity together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModulePublishedRhaiWorkspace {
    pub release: crate::ArtifactRelease,
    pub workspace: rustok_sandbox::RhaiWorkspace,
}

/// Host-authenticated promotion of a verified build-worker receipt into the
/// publication ledger. Unlike generic evidence, this preserves the exact OCI
/// subject and signature-manifest identity produced by the build service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleBuildServiceAttestationCommand {
    pub request_id: String,
    pub expected_revision: i64,
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
    pub expected_revision: i64,
    pub registry_id: String,
    pub reference: OciArtifactReference,
    pub descriptor: crate::ModuleArtifactDescriptor,
    pub evidence: ArtifactVerificationEvidence,
    pub actor_principal: serde_json::Value,
}

/// Owner-authenticated selection of one durable completed platform build for a
/// registry request. The command carries the complete tenant-scoped command
/// evidence; the build request/result are always reloaded under tenant RLS by
/// the owner service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublishPlatformBuildStageCommand {
    pub request_id: String,
    pub expected_revision: i64,
    pub context: ModuleCommandContext,
    pub build_request_id: Uuid,
    pub actor_principal: serde_json::Value,
    /// An authenticated host fact. The owner combines it with the durable
    /// request and owner-binding identities before staging the build.
    pub actor_can_manage_modules: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePublishPlatformBuildStageResult {
    pub staging_id: String,
    pub created: bool,
    /// Current aggregate revision after this staging command. An exact
    /// idempotent replay returns the current locked revision without another
    /// aggregate transition.
    pub request_revision: i64,
}

/// Owner-reloaded immutable platform build selected for publication evidence.
/// A producer receives this only after the current publish request, staging
/// row, completed build result, and all digest-pinned receipt identities have
/// been revalidated together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePlatformPublicationSource {
    pub request_id: String,
    /// Revision captured with the source facts. The evidence producer must
    /// present it when it records the build-service attestation.
    pub request_revision: i64,
    pub tenant_id: Uuid,
    pub build_request_id: Uuid,
    pub slug: String,
    pub version: String,
    pub component_digest: String,
    pub receipt: ModuleBuildPublicationReceipt,
}

/// Owner-reloaded immutable Alloy receipt selected for canonical Rhai OCI
/// publication. The registry-validation worker consumes this projection rather
/// than rereading mutable Alloy storage or accepting source identity from the
/// uploaded workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAlloyPublicationSource {
    pub request_id: String,
    /// Revision captured with the exact source receipt. The platform admission
    /// must present this value so a stale publication cannot overwrite a later
    /// owner transition.
    pub request_revision: i64,
    pub slug: String,
    pub version: String,
    pub license: String,
    pub alloy_tenant_id: Uuid,
    pub alloy_script_id: Uuid,
    pub source_revision: u32,
    pub source_digest: String,
    pub review_digest: String,
    pub descriptor: crate::ModuleArtifactDescriptor,
    pub descriptor_digest: String,
}

impl ModuleAlloyPublicationSource {
    pub fn trust_provenance(&self) -> crate::TrustAlloyWorkspaceProvenance {
        crate::TrustAlloyWorkspaceProvenance {
            request_id: self.request_id.clone(),
            alloy_tenant_id: self.alloy_tenant_id,
            alloy_script_id: self.alloy_script_id,
            source_revision: self.source_revision,
            source_digest: self.source_digest.clone(),
            review_digest: self.review_digest.clone(),
            descriptor_digest: self.descriptor_digest.clone(),
            workspace_entrypoint: self.descriptor.entrypoint.clone(),
        }
    }
}

