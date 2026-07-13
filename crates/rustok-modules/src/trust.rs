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
    #[serde(default)]
    pub evidence_references: Vec<String>,
}

impl TrustVerificationDecision {
    pub fn admitted(&self) -> bool {
        self.signature_verified && self.provenance_verified && self.sbom_verified
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
