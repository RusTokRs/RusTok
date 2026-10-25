use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{ModuleArtifactDescriptor, OciArtifactReference};

const MAX_TRUST_EVIDENCE_REFERENCE_BYTES: usize = 512;

/// Stable SLSA builder identity for an OCI package assembled from an immutable
/// owner-receipted Alloy workspace. This is deliberately distinct from the
/// isolated WASM build-worker identity: no Alloy release may claim to have
/// passed through that build service.
pub const ALLOY_WORKSPACE_PUBLICATION_BUILDER_ID: &str =
    "https://rustok.dev/registry-validation-worker";
/// Stable SLSA build type for a canonical Rhai workspace package.
pub const ALLOY_WORKSPACE_PUBLICATION_BUILD_TYPE: &str =
    "https://rustok.dev/build/rhai-workspace/v1";
/// Policy-visible source class for owner-receipted Alloy workspaces. The
/// per-release source identity lives in the signed `rustok` provenance facts.
pub const ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI: &str =
    "https://rustok.dev/alloy/reviewed-workspace";
/// Stable source ref paired with `ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI`.
pub const ALLOY_WORKSPACE_PUBLICATION_SOURCE_REF: &str = "owner-receipted";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustEvidenceKind {
    Signature,
    Provenance,
    Sbom,
}

impl TrustEvidenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Signature => "signature",
            Self::Provenance => "provenance",
            Self::Sbom => "sbom",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustEvidenceReference {
    pub kind: TrustEvidenceKind,
    pub reference: String,
    pub digest: String,
}

impl TrustEvidenceReference {
    pub fn validate(&self) -> bool {
        !self.reference.is_empty()
            && self.reference.len() <= MAX_TRUST_EVIDENCE_REFERENCE_BYTES
            && !self
                .reference
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
            && self.digest.strip_prefix("sha256:").is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
    }
}

/// Versioned policy input passed from the control-plane owner to an isolated
/// verification worker. It contains no registry or trust-root credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustVerificationRequest {
    pub reference: OciArtifactReference,
    pub descriptor: ModuleArtifactDescriptor,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    /// Present only for an Alloy-authored workspace whose source facts were
    /// captured by the registry owner. The isolated verifier requires the
    /// signed SLSA statement to retain this exact binding in addition to its
    /// configured source-class policy.
    pub expected_alloy_workspace_provenance: Option<TrustAlloyWorkspaceProvenance>,
}

/// Exact owner-receipted provenance facts that an Alloy workspace SLSA
/// statement must carry. This is not a user-authored manifest and has no
/// meaning for generic OCI admission paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustAlloyWorkspaceProvenance {
    pub request_id: String,
    pub alloy_tenant_id: Uuid,
    pub alloy_script_id: Uuid,
    pub source_revision: u32,
    pub source_digest: String,
    pub review_digest: String,
    pub descriptor_digest: String,
    pub workspace_entrypoint: String,
}

impl TrustAlloyWorkspaceProvenance {
    pub fn validate(&self) -> bool {
        !self.request_id.trim().is_empty()
            && self.request_id.len() <= 256
            && !self.request_id.chars().any(char::is_control)
            && !self.alloy_tenant_id.is_nil()
            && !self.alloy_script_id.is_nil()
            && self.source_revision > 0
            && is_sha256_digest(&self.source_digest)
            && is_sha256_digest(&self.review_digest)
            && is_sha256_digest(&self.descriptor_digest)
            && !self.workspace_entrypoint.trim().is_empty()
            && self.workspace_entrypoint.len() <= 160
            && !self.workspace_entrypoint.chars().any(char::is_control)
    }
}

pub(crate) fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// Immutable policy revisions selected by the control plane for one admission.
/// The verifier must return a decision produced against exactly these revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustPolicyRevision {
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    /// Revision of the concrete capability grants selected for this
    /// installation. It is distinct from the policy used to evaluate them.
    pub capability_grant_revision: u64,
}

/// Policy revisions against which already-admitted evidence is judged for
/// staleness and refreshed. The concrete capability grant revision is
/// deliberately absent: reverification re-evaluates trust evidence under
/// current policy and never re-selects an installation's grants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionReverificationPolicyRevision {
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
}

/// Redacted worker decision. Evidence references address immutable attestations
/// or bundles; admission records never persist verifier command output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustVerificationDecision {
    pub signer_identity: String,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub signature_verified: bool,
    pub provenance_verified: bool,
    pub sbom_verified: bool,
    pub license_policy_verified: bool,
    pub vulnerability_policy_verified: bool,
    pub evidence: Vec<TrustEvidenceReference>,
}

impl TrustVerificationDecision {
    pub fn admitted(&self) -> bool {
        let evidence_kinds = self
            .evidence
            .iter()
            .map(|evidence| evidence.kind)
            .collect::<std::collections::BTreeSet<_>>();
        self.signature_verified
            && self.provenance_verified
            && self.sbom_verified
            && self.license_policy_verified
            && self.vulnerability_policy_verified
            && self.evidence.iter().all(TrustEvidenceReference::validate)
            && evidence_kinds.len() == self.evidence.len()
            && [
                TrustEvidenceKind::Signature,
                TrustEvidenceKind::Provenance,
                TrustEvidenceKind::Sbom,
            ]
            .into_iter()
            .all(|kind| evidence_kinds.contains(&kind))
    }

    /// True when this decision both admits the artifact and was produced
    /// against exactly the policy revisions selected in `request`. Admission
    /// paths must use this check: `admitted` alone accepts a decision made
    /// under any (possibly stale) policy revision.
    pub fn admitted_for(&self, request: &TrustVerificationRequest) -> bool {
        self.trust_policy_revision == request.trust_policy_revision
            && self.capability_policy_revision == request.capability_policy_revision
            && self.admitted()
    }
}

/// Failure of a [`TrustVerifier`] port. A verifier that could evaluate the
/// request reports its verdict inside [`TrustVerificationDecision`]; this
/// error is reserved for requests the verifier could not evaluate at all.
/// `InvalidRequest` is terminal for the submitted evidence, while
/// `Unavailable` covers verifier, transport, and evidence-fetch outages.
#[derive(Debug, Error)]
pub enum TrustVerificationError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Unavailable(String),
}

/// Owner port implemented by the isolated verification worker adapter.
#[async_trait]
pub trait TrustVerifier: Send + Sync {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, TrustVerificationError>;
}

#[cfg(test)]
mod tests {
    use super::{TrustEvidenceKind, TrustEvidenceReference, TrustVerificationDecision};

    fn admitted_decision() -> TrustVerificationDecision {
        TrustVerificationDecision {
            signer_identity: "build-service:production".to_string(),
            trust_policy_revision: 7,
            capability_policy_revision: 9,
            signature_verified: true,
            provenance_verified: true,
            sbom_verified: true,
            license_policy_verified: true,
            vulnerability_policy_verified: true,
            evidence: [
                TrustEvidenceKind::Signature,
                TrustEvidenceKind::Provenance,
                TrustEvidenceKind::Sbom,
            ]
            .into_iter()
            .map(|kind| TrustEvidenceReference {
                kind,
                reference: format!(
                    "oci://registry.example/module@sha256:{}#{kind:?}",
                    "a".repeat(64)
                ),
                digest: format!("sha256:{}", "a".repeat(64)),
            })
            .collect(),
        }
    }

    #[test]
    fn admitted_for_rejects_decisions_under_other_policy_revisions() {
        use super::TrustVerificationRequest;
        use crate::{
            ArtifactModuleKind, ArtifactPayloadKind, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            ModuleArtifactDescriptor, OciArtifactReference,
        };

        let request = TrustVerificationRequest {
            reference: OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/trust-fixture".to_string(),
                digest: format!("sha256:{}", "b".repeat(64)),
            },
            descriptor: ModuleArtifactDescriptor {
                schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
                slug: "trust_fixture".to_string(),
                version: "1.0.0".to_string(),
                payload_kind: ArtifactPayloadKind::Rhai,
                module_kind: ArtifactModuleKind::Optional,
                runtime_abi: "rustok:module/runtime@1".to_string(),
                platform_compatibility: "^0.1".to_string(),
                required_features: vec![],
                artifact_digest: format!("sha256:{}", "a".repeat(64)),
                entrypoint: "main".to_string(),
                capabilities: vec![],
                bindings: vec![],
                dependencies: vec![],
                permissions: vec![],
                schema_documents: vec![],
                settings_schema_digest: None,
                data_schema_digest: None,
                localization_catalogs: vec![],
                ui_contributions: vec![],
                persistence_contract: None,
            },
            trust_policy_revision: 7,
            capability_policy_revision: 9,
            expected_alloy_workspace_provenance: None,
        };

        let decision = admitted_decision();
        assert!(decision.admitted_for(&request));

        let mut stale_trust = decision.clone();
        stale_trust.trust_policy_revision = 6;
        assert!(stale_trust.admitted());
        assert!(!stale_trust.admitted_for(&request));

        let mut stale_capability = decision;
        stale_capability.capability_policy_revision = 10;
        assert!(!stale_capability.admitted_for(&request));
    }

    #[test]
    fn admission_requires_independent_license_and_vulnerability_policy_results() {
        let mut decision = admitted_decision();
        assert!(decision.admitted());

        decision.license_policy_verified = false;
        assert!(!decision.admitted());

        decision.license_policy_verified = true;
        decision.vulnerability_policy_verified = false;
        assert!(!decision.admitted());
    }
}
