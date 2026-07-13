use std::time::Duration;

use async_trait::async_trait;
use rustok_modules::{TrustVerificationDecision, TrustVerificationRequest, TrustVerifier};
use tokio::process::Command;

use crate::VerificationPolicy;

/// Concrete worker-only Cosign adapter. It accepts no caller-provided command
/// arguments: image reference, identities and issuers are derived from typed
/// owner input and mounted policy only.
pub struct CosignTrustVerifier {
    program: String,
    policy: VerificationPolicy,
    timeout: Duration,
}

impl CosignTrustVerifier {
    pub fn new(policy: VerificationPolicy) -> Self {
        Self {
            program: std::env::var("RUSTOK_COSIGN_PROGRAM").unwrap_or_else(|_| "cosign".into()),
            policy,
            timeout: Duration::from_secs(30),
        }
    }

    async fn run(&self, arguments: Vec<String>) -> Result<(), String> {
        let output = tokio::time::timeout(
            self.timeout,
            Command::new(&self.program).args(arguments).output(),
        )
        .await
        .map_err(|_| "cosign verification timed out".to_string())?
        .map_err(|error| format!("could not start cosign: {error}"))?;
        if !output.status.success() {
            return Err("cosign verification rejected the artifact".to_string());
        }
        Ok(())
    }

    async fn verify_for_identity(
        &self,
        request: &TrustVerificationRequest,
        identity: &str,
        issuer: &str,
    ) -> Result<(), String> {
        let reference = request.reference.canonical();
        let flags = vec![
            "--certificate-identity".to_string(),
            identity.to_string(),
            "--certificate-oidc-issuer".to_string(),
            issuer.to_string(),
        ];
        let mut signature = vec!["verify".to_string(), "--output".to_string(), "json".to_string()];
        signature.extend(flags.clone());
        signature.push(reference.clone());
        self.run(signature).await?;
        for predicate in ["slsaprovenance", "cyclonedx"] {
            let mut attestation = vec![
                "verify-attestation".to_string(),
                "--type".to_string(),
                predicate.to_string(),
                "--output".to_string(),
                "json".to_string(),
            ];
            attestation.extend(flags.clone());
            attestation.push(reference.clone());
            self.run(attestation).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl TrustVerifier for CosignTrustVerifier {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, String> {
        for identity in &self.policy.allowed_signer_identities {
            for issuer in &self.policy.allowed_oidc_issuers {
                if self.verify_for_identity(&request, identity, issuer).await.is_ok() {
                    return Ok(TrustVerificationDecision {
                        signer_identity: identity.clone(),
                        trust_policy_revision: request.trust_policy_revision,
                        capability_policy_revision: request.capability_policy_revision,
                        signature_verified: true,
                        provenance_verified: true,
                        sbom_verified: true,
                        evidence_references: vec![
                            format!("{}#cosign-signature", request.reference.canonical()),
                            format!("{}#slsa-provenance", request.reference.canonical()),
                            format!("{}#cyclonedx-sbom", request.reference.canonical()),
                        ],
                    });
                }
            }
        }
        Err("no configured Cosign signer identity and OIDC issuer verified the artifact".into())
    }
}
