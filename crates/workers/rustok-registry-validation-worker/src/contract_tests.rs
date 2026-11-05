use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_build_publication::{
    RegistryCredentialBroker, RegistryCredentialError, RegistryCredentialLease,
};
use rustok_modules::{
    ModulePublicationArtifactOrigin, OciArtifactPublicationTarget, OciArtifactReference,
};
use serde_json::json;

use crate::{
    CredentialedOciRegistryProvider, RegistryValidationPublicationPolicy, failed_validation_checks,
    successful_validation_checks,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text =
        std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-registry-validation-worker""#));
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
    assert!(api_text.contains("## Operational Framing & Execution Lifecycle"));
    assert!(api_text.contains("## Minimum Contract Set"));
    assert!(api_text.contains("### Input DTOs/Commands"));
    assert!(api_text.contains("### Domain Invariants"));
    assert!(api_text.contains("### Events / Outbox Side Effects"));
    assert!(api_text.contains("### Errors / Failure Codes"));
    assert!(api_text.contains("## Verification & Test Coverage"));
}

fn sample_policy() -> RegistryValidationPublicationPolicy {
    RegistryValidationPublicationPolicy {
        registry_id: "default-registry".to_string(),
        trust_policy_revision: 1,
        capability_policy_revision: 2,
        build_service_issuer_identity: "build-service:prod".to_string(),
        build_service_policy_revision: "v1".to_string(),
    }
}

#[test]
fn publication_policy_command_validation() {
    let policy = sample_policy();
    let actor = json!({"kind":"service","id":"worker-1"});

    let cmd = policy
        .command("req-1".to_string(), actor.clone())
        .expect("valid command");
    assert_eq!(cmd.request_id, "req-1");
    assert_eq!(cmd.registry_id, "default-registry");
    assert_eq!(cmd.trust_policy_revision, 1);
    assert_eq!(cmd.capability_policy_revision, 2);

    let artifact = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "modules/rhai".to_string(),
        digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
    };
    let alloy_cmd = policy
        .alloy_command("req-2".to_string(), artifact, actor)
        .expect("valid alloy command");
    assert_eq!(alloy_cmd.request_id, "req-2");
    assert_eq!(alloy_cmd.registry_id, "default-registry");

    let mut invalid_policy = sample_policy();
    invalid_policy.registry_id = "".to_string();
    assert!(
        invalid_policy
            .command("req-3".to_string(), json!({"kind":"service"}))
            .is_err()
    );
}

#[test]
fn validation_checks_structure() {
    let platform_checks =
        successful_validation_checks(ModulePublicationArtifactOrigin::PlatformBuilt);
    assert_eq!(platform_checks.len(), 2);
    assert_eq!(platform_checks[0].key, "artifact_contract");
    assert_eq!(platform_checks[0].status, "passed");
    assert_eq!(platform_checks[1].key, "platform_publication_evidence");
    assert_eq!(platform_checks[1].status, "passed");

    let alloy_checks = successful_validation_checks(ModulePublicationArtifactOrigin::AlloyAuthored);
    assert_eq!(alloy_checks.len(), 3);
    assert_eq!(alloy_checks[0].key, "artifact_contract");
    assert_eq!(alloy_checks[1].key, "alloy_oci_publication");
    assert_eq!(alloy_checks[2].key, "platform_admission");

    let prebuilt_checks =
        successful_validation_checks(ModulePublicationArtifactOrigin::ExternalPrebuilt);
    assert_eq!(prebuilt_checks.len(), 1);

    let failed_checks =
        failed_validation_checks(ModulePublicationArtifactOrigin::AlloyAuthored, false);
    assert_eq!(failed_checks[0].status, "failed");
    assert_eq!(failed_checks[1].status, "not_run");
    assert_eq!(failed_checks[2].status, "not_run");

    let partial_failed =
        failed_validation_checks(ModulePublicationArtifactOrigin::PlatformBuilt, true);
    assert_eq!(partial_failed[0].status, "passed");
    assert_eq!(partial_failed[1].status, "failed");
}

struct MockCredentialBroker;

#[async_trait]
impl RegistryCredentialBroker for MockCredentialBroker {
    fn is_ready(&self) -> bool {
        true
    }

    async fn acquire(
        &self,
        _target: &OciArtifactPublicationTarget,
        _minimum_ttl: Duration,
    ) -> Result<RegistryCredentialLease, RegistryCredentialError> {
        Err(RegistryCredentialError::Unavailable("mock".to_string()))
    }
}

#[test]
fn debug_formatting_implementations() {
    let policy = sample_policy();
    let debug_policy = format!("{policy:?}");
    assert!(debug_policy.contains("RegistryValidationPublicationPolicy"));
    assert!(debug_policy.contains("default-registry"));

    let broker = Arc::new(MockCredentialBroker);
    let provider = CredentialedOciRegistryProvider::new(broker).expect("ready provider");
    let debug_provider = format!("{provider:?}");
    assert!(debug_provider.contains("CredentialedOciRegistryProvider"));
}
