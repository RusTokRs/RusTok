use rustok_artifact_node_transport::StaticModuleArtifactNodeAgentAuthenticator;

use crate::ArtifactNodeControllerConfig;

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text = std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-artifact-node-controller""#));
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
    assert!(api_text.contains("## Configuration & Environment"));
    assert!(api_text.contains("## Minimum Contract Set"));
    assert!(api_text.contains("### Input DTOs/Commands"));
    assert!(api_text.contains("### Domain Invariants"));
    assert!(api_text.contains("### Events / Outbox Side Effects"));
    assert!(api_text.contains("### Errors / Failure Codes"));
    assert!(api_text.contains("## Verification & Test Coverage"));
}

#[test]
fn controller_config_debug_redacts_database_url() {
    let fp = rustok_worker_transport::PeerCertificateFingerprint::parse(
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .unwrap();
    let identity = rustok_artifact_node_transport::ModuleArtifactNodeAgentIdentity::new(
        uuid::Uuid::new_v4(),
        "agent-1",
    )
    .unwrap();
    let authenticator =
        StaticModuleArtifactNodeAgentAuthenticator::new(vec![(fp, identity)]).unwrap();

    let config = ArtifactNodeControllerConfig {
        database_url: "postgres://secret_admin:very_secret_pass@10.0.0.1:5432/rustok".to_string(),
        agent_authenticator: authenticator,
    };

    let debug_repr = format!("{config:?}");
    assert!(debug_repr.contains("ArtifactNodeControllerConfig"));
    assert!(debug_repr.contains("[REDACTED]"));
    assert!(!debug_repr.contains("very_secret_pass"));
}
