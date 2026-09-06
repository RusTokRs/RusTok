use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{ModuleArtifactDescriptor, OciArtifactReference};

const MAX_TRUST_EVIDENCE_REFERENCE_BYTES: usize = 512;

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
}

/// Owner port implemented by the isolated verification worker adapter.
#[async_trait]
pub trait TrustVerifier: Send + Sync {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, String>;
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
