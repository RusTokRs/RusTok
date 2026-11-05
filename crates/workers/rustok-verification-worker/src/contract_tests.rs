use async_trait::async_trait;
use rustok_modules::{
    ModuleArtifactDescriptor, OciArtifactReference, TrustEvidenceKind, TrustEvidenceReference,
    TrustVerificationDecision, TrustVerificationError, TrustVerificationRequest, TrustVerifier,
};

use crate::{
    CosignTrustVerifier, VerificationPolicy, VerificationRetiringTrustRoot, VerificationTrustRoot,
    VerificationTrustRoots, VerificationWorker,
};

fn sample_descriptor() -> ModuleArtifactDescriptor {
    serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "slug": "sample-mod",
        "version": "1.0.0",
        "payload_kind": "wasm_component",
        "module_kind": "optional",
        "runtime_abi": "wasm32-wasip2",
        "platform_compatibility": "any",
        "entrypoint": "sample.wasm",
        "artifact_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }))
    .expect("sample descriptor JSON should deserialize")
}

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text =
        std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-verification-worker""#));
    assert!(manifest_text.contains(r#"readme = "README.md""#));
    assert!(manifest_text.contains(r#"repository.workspace = true"#));
    assert!(manifest_text.contains(r#"[package.metadata.docs.rs]"#));
}

#[test]
fn crate_api_contract_specification_is_complete() {
    let api_bytes = include_bytes!("../CRATE_API.md");
    let api_text = std::str::from_utf8(api_bytes).expect("CRATE_API.md must be valid UTF-8");

    assert!(api_text.contains("## Role"));
    assert!(api_text.contains("## Public surface"));
    assert!(api_text.contains("## Wire Protocol & Operational Framing"));
    assert!(api_text.contains("## Minimum Contract Set"));
    assert!(api_text.contains("### Input DTOs/Commands"));
    assert!(api_text.contains("### Domain Invariants"));
    assert!(api_text.contains("### Events / Outbox Side Effects"));
    assert!(api_text.contains("### Errors / Failure Codes"));
    assert!(api_text.contains("## Verification & Test Coverage"));
}

fn sample_policy() -> VerificationPolicy {
    VerificationPolicy {
        trust_policy_revision: 2,
        capability_policy_revision: 3,
        trust_root: VerificationTrustRoots {
            active: VerificationTrustRoot::KeylessSigstore {
                allowed_signer_identities: vec!["signer@rustok.dev".into()],
                allowed_oidc_issuers: vec!["https://auth.rustok.dev".into()],
                require_transparency_bundle: true,
            },
            retiring: None,
        },
        allowed_builders: vec!["https://builder.rustok.dev".into()],
        allowed_source_repositories: vec!["https://github.com/rustok/repo".into()],
        allowed_source_refs: vec!["refs/heads/main".into()],
        allowed_build_types: vec!["https://rustok.dev/build/v1".into()],
        allowed_licenses: vec!["MIT".into(), "Apache-2.0".into()],
        allowed_cyclonedx_spec_versions: vec!["1.6".into()],
        maximum_vulnerability_severity: "medium".into(),
    }
}

#[test]
fn policy_validation_catches_invalid_configuration() {
    let mut policy = sample_policy();
    assert!(policy.validate().is_ok());

    policy.maximum_vulnerability_severity = "invalid_severity".into();
    assert!(policy.validate().is_err());

    policy.maximum_vulnerability_severity = "high".into();
    policy.allowed_builders = vec!["".into()];
    assert!(policy.validate().is_err());

    policy.allowed_builders = vec![];
    assert!(policy.validate().is_err());

    policy = sample_policy();
    policy.trust_root.retiring = Some(VerificationRetiringTrustRoot {
        root: policy.trust_root.active.clone(),
        retire_after_unix_seconds: 100,
    });
    assert!(
        policy.validate().is_err(),
        "retiring root must differ from active"
    );
}

struct DummyVerifier;

#[async_trait]
impl TrustVerifier for DummyVerifier {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, TrustVerificationError> {
        Ok(TrustVerificationDecision {
            signer_identity: "signer@rustok.dev".to_string(),
            trust_policy_revision: request.trust_policy_revision,
            capability_policy_revision: request.capability_policy_revision,
            signature_verified: true,
            provenance_verified: true,
            sbom_verified: true,
            license_policy_verified: true,
            vulnerability_policy_verified: true,
            evidence: vec![
                TrustEvidenceReference {
                    kind: TrustEvidenceKind::Signature,
                    reference: "oci://ghcr.io/rustok/sample#cosign-signature".to_string(),
                    digest:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_string(),
                },
                TrustEvidenceReference {
                    kind: TrustEvidenceKind::Provenance,
                    reference: "oci://ghcr.io/rustok/sample#slsa-provenance".to_string(),
                    digest:
                        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                            .to_string(),
                },
                TrustEvidenceReference {
                    kind: TrustEvidenceKind::Sbom,
                    reference: "oci://ghcr.io/rustok/sample#cyclonedx-sbom".to_string(),
                    digest:
                        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                            .to_string(),
                },
            ],
        })
    }
}

impl std::fmt::Debug for DummyVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DummyVerifier")
    }
}

#[tokio::test]
async fn worker_enforces_policy_revisions_and_fails_closed() {
    let policy = sample_policy();
    let worker = VerificationWorker::new(DummyVerifier, policy);

    let debug_output = format!("{worker:?}");
    assert!(debug_output.contains("VerificationWorker"));
    assert!(debug_output.contains("DummyVerifier"));

    let req_mismatched = TrustVerificationRequest {
        reference: OciArtifactReference {
            registry: "ghcr.io".to_string(),
            repository: "rustok/sample".to_string(),
            digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
        },
        descriptor: sample_descriptor(),
        trust_policy_revision: 999,
        capability_policy_revision: 3,
        expected_alloy_workspace_provenance: None,
    };

    let result = worker.verify(req_mismatched).await;
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("policy revisions do not match"));

    let req_valid = TrustVerificationRequest {
        reference: OciArtifactReference {
            registry: "ghcr.io".to_string(),
            repository: "rustok/sample".to_string(),
            digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
        },
        descriptor: sample_descriptor(),
        trust_policy_revision: 2,
        capability_policy_revision: 3,
        expected_alloy_workspace_provenance: None,
    };

    let decision = worker.verify(req_valid).await.expect("valid verification");
    assert_eq!(decision.signer_identity, "signer@rustok.dev");
    assert!(decision.signature_verified);
}

#[test]
fn cosign_verifier_debug_formatting() {
    let policy = sample_policy();
    let verifier = CosignTrustVerifier::new(policy);
    let debug_output = format!("{verifier:?}");
    assert!(debug_output.contains("CosignTrustVerifier"));
    assert!(debug_output.contains("program"));
    assert!(debug_output.contains("timeout"));
}
