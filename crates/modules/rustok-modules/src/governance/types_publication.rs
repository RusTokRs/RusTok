//! Governance publication artifact origin, evidence commands, and follow-up stages.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;
use super::constants::*;
use crate::build::ModuleBuildPublicationReceipt;
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};

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

