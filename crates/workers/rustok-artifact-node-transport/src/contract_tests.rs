use rustok_worker_transport::PeerCertificateFingerprint;
use uuid::Uuid;

use crate::{
    ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE, ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
    ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE, ModuleArtifactNodeAgentIdentity,
    ModuleArtifactNodeReconciliationIdentity, StaticModuleArtifactNodeAgentAuthenticator,
    StaticModuleArtifactNodeReconciliationAuthenticator,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text =
        std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-artifact-node-transport""#));
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

#[test]
fn protocol_constants_invariants() {
    assert_eq!(ARTIFACT_NODE_AGENT_PROTOCOL_REVISION, 1);
    assert_eq!(ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE, 1024 * 1024);
    assert_eq!(ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE, 1024 * 1024);
}

#[test]
fn static_authenticators_debug_formatting() {
    let fp = PeerCertificateFingerprint::parse(
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .unwrap();
    let agent_id = ModuleArtifactNodeAgentIdentity::new(Uuid::new_v4(), "agent-1").unwrap();
    let agent_auth =
        StaticModuleArtifactNodeAgentAuthenticator::new(vec![(fp.clone(), agent_id)]).unwrap();
    let agent_debug = format!("{agent_auth:?}");
    assert!(agent_debug.contains("StaticModuleArtifactNodeAgentAuthenticator"));
    assert!(agent_debug.contains("identities_count: 1"));

    let operator_id =
        ModuleArtifactNodeReconciliationIdentity::new(Uuid::new_v4(), vec![Uuid::new_v4()])
            .unwrap();
    let operator_auth =
        StaticModuleArtifactNodeReconciliationAuthenticator::new(vec![(fp, operator_id)]).unwrap();
    let operator_debug = format!("{operator_auth:?}");
    assert!(operator_debug.contains("StaticModuleArtifactNodeReconciliationAuthenticator"));
    assert!(operator_debug.contains("identities_count: 1"));
}
