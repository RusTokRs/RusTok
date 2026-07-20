use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{ModuleArtifactDescriptor, OciArtifactReference};

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
    #[serde(default)]
    pub evidence_references: Vec<String>,
}

impl TrustVerificationDecision {
    pub fn admitted(&self) -> bool {
        self.signature_verified
            && self.provenance_verified
            && self.sbom_verified
            && self.license_policy_verified
            && self.vulnerability_policy_verified
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
    use super::TrustVerificationDecision;

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
            evidence_references: vec!["oci://evidence".to_string()],
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
