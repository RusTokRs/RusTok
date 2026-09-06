use async_trait::async_trait;
use rustok_modules::{TrustVerificationDecision, TrustVerificationRequest, TrustVerifier};
use thiserror::Error;

use crate::VerificationPolicy;

/// Worker orchestration seam. Concrete Cosign/SLSA/CycloneDX adapters are
/// injected by the process host and must not run in `apps/server`.
pub struct VerificationWorker<V> {
    verifier: V,
    policy: VerificationPolicy,
}

impl<V> VerificationWorker<V> {
    pub fn new(verifier: V, policy: VerificationPolicy) -> Self {
        Self { verifier, policy }
    }
}

#[async_trait]
impl<V> TrustVerifier for VerificationWorker<V>
where
    V: TrustVerifier,
{
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, String> {
        if request.trust_policy_revision != self.policy.trust_policy_revision
            || request.capability_policy_revision != self.policy.capability_policy_revision
        {
            return Err(VerificationWorkerError::PolicyRevisionMismatch.to_string());
        }
        let decision = self.verifier.verify(request).await?;
        if !self.policy.signer_is_allowed(&decision.signer_identity) {
            return Err(VerificationWorkerError::SignerDenied.to_string());
        }
        if !decision.admitted() {
            return Err(VerificationWorkerError::RequiredEvidenceMissing.to_string());
        }
        Ok(decision)
    }
}

#[derive(Debug, Error)]
pub enum VerificationWorkerError {
    #[error("verification request policy revisions do not match worker policy")]
    PolicyRevisionMismatch,
    #[error("verification signer identity is not allowed")]
    SignerDenied,
    #[error(
        "verification decision lacks required signature, provenance, SBOM, license, or vulnerability evidence"
    )]
    RequiredEvidenceMissing,
}
