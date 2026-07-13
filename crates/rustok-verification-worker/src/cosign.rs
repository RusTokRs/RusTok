use std::time::Duration;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use rustok_modules::{TrustVerificationDecision, TrustVerificationRequest, TrustVerifier};
use serde_json::Value;
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

    async fn run(&self, arguments: Vec<String>) -> Result<Vec<u8>, String> {
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
        Ok(output.stdout)
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
        let mut signature = vec![
            "verify".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];
        signature.extend(flags.clone());
        signature.push(reference.clone());
        self.run(signature).await?;
        let mut attestation_outputs = Vec::new();
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
            attestation_outputs.push(self.run(attestation).await?);
        }
        validate_slsa(&attestation_outputs[0], request, &self.policy)?;
        validate_cyclonedx(&attestation_outputs[1], request, &self.policy)?;
        Ok(())
    }
}

fn attestation_predicate(output: &[u8]) -> Result<Value, String> {
    let values: Value = serde_json::from_slice(output)
        .map_err(|_| "cosign attestation output is not JSON".to_string())?;
    let payload = values
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("payload").or_else(|| item.get("Payload")))
        .and_then(Value::as_str)
        .ok_or_else(|| "cosign attestation payload is missing".to_string())?;
    let envelope: Value = serde_json::from_slice(
        &STANDARD
            .decode(payload)
            .map_err(|_| "cosign attestation payload is not base64".to_string())?,
    )
    .map_err(|_| "in-toto statement is not JSON".to_string())?;
    envelope
        .get("predicate")
        .cloned()
        .ok_or_else(|| "in-toto attestation predicate is missing".to_string())
}

fn validate_slsa(output: &[u8], request: &TrustVerificationRequest, policy: &VerificationPolicy) -> Result<(), String> {
    let predicate = attestation_predicate(output)?;
    let builder = predicate.pointer("/runDetails/builder/id").and_then(Value::as_str).unwrap_or_default();
    let build_type = predicate.pointer("/buildDefinition/buildType").and_then(Value::as_str).unwrap_or_default();
    let source = predicate.pointer("/buildDefinition/externalParameters/source/uri").and_then(Value::as_str).unwrap_or_default();
    if !policy.allowed_builders.contains(&builder.to_string()) || !policy.allowed_build_types.contains(&build_type.to_string()) || !policy.allowed_source_repositories.iter().any(|allowed| source.starts_with(allowed)) {
        return Err("SLSA provenance does not satisfy builder, build-type, or source policy".into());
    }
    let statement = serde_json::to_string(&predicate).map_err(|_| "could not inspect SLSA provenance".to_string())?;
    if !statement.contains(&request.descriptor.artifact_digest) {
        return Err("SLSA provenance does not reference the admitted payload digest".into());
    }
    Ok(())
}

fn validate_cyclonedx(output: &[u8], request: &TrustVerificationRequest, policy: &VerificationPolicy) -> Result<(), String> {
    let predicate = attestation_predicate(output)?;
    let bom = predicate.get("bom").unwrap_or(&predicate);
    if bom.get("bomFormat").and_then(Value::as_str) != Some("CycloneDX") {
        return Err("SBOM attestation is not CycloneDX JSON".into());
    }
    let text = serde_json::to_string(bom).map_err(|_| "could not inspect CycloneDX SBOM".to_string())?;
    if !text.contains(&request.descriptor.artifact_digest) || !policy.allowed_licenses.iter().any(|license| text.contains(license)) {
        return Err("CycloneDX SBOM does not satisfy subject or license policy".into());
    }
    Ok(())
}

#[async_trait]
impl TrustVerifier for CosignTrustVerifier {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, String> {
        for identity in &self.policy.allowed_signer_identities {
            for issuer in &self.policy.allowed_oidc_issuers {
                if self
                    .verify_for_identity(&request, identity, issuer)
                    .await
                    .is_ok()
                {
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
