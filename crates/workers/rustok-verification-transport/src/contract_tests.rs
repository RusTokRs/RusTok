use std::sync::Arc;
use std::time::Duration;

use rustok_modules::{
    TrustVerificationDecision, TrustVerificationError, TrustVerificationRequest, TrustVerifier,
};
use rustok_worker_transport::WorkerAdmission;
use tonic::transport::Channel;

use crate::{GrpcTrustVerifier, VerificationGrpcService};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text =
        std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-verification-transport""#));
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
    assert!(api_text.contains("## Wire Protocol & Framing"));
    assert!(api_text.contains("## Minimum Contract Set"));
    assert!(api_text.contains("### Input DTOs/Commands"));
    assert!(api_text.contains("### Domain Invariants"));
    assert!(api_text.contains("### Events / Outbox Side Effects"));
    assert!(api_text.contains("### Errors / Failure Codes"));
    assert!(api_text.contains("## Verification & Test Coverage"));
}

struct DummyVerifier;

#[async_trait::async_trait]
impl TrustVerifier for DummyVerifier {
    async fn verify(
        &self,
        _request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, TrustVerificationError> {
        Err(TrustVerificationError::Unavailable("dummy".to_string()))
    }
}

impl std::fmt::Debug for DummyVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DummyVerifier")
    }
}

#[tokio::test]
async fn grpc_trust_verifier_debug_formatting() {
    let channel = Channel::from_static("http://127.0.0.1:50051").connect_lazy();
    let verifier = GrpcTrustVerifier::from_channel(channel);
    let debug_output = format!("{verifier:?}");
    assert!(debug_output.contains("GrpcTrustVerifier"));
}

#[test]
fn verification_grpc_service_debug_formatting() {
    let verifier = Arc::new(DummyVerifier);
    let admission = WorkerAdmission::new(5, Duration::from_secs(2)).expect("valid admission");
    let service = VerificationGrpcService::new(verifier, admission);
    let debug_output = format!("{service:?}");
    assert!(debug_output.contains("VerificationGrpcService"));
    assert!(debug_output.contains("DummyVerifier"));
}
