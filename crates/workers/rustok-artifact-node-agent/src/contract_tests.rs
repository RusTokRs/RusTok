use std::time::Duration;
use rustok_artifact_node_transport::ModuleArtifactNodeAgentIdentity;
use rustok_storage::StorageConfig;
use tonic::transport::Endpoint;
use uuid::Uuid;

use crate::{
    ArtifactNodeAgentConfig, DeploymentSlot, FencedWorkerGenerationCoordinator,
    HttpSsrSwitchingCoordinator, SlotState, SlotSupervisor, WatchdogConfig,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text = std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-artifact-node-agent""#));
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
fn config_debug_redacts_storage_credentials() {
    let node_id = Uuid::new_v4();
    let config = ArtifactNodeAgentConfig {
        controller_endpoint: Endpoint::from_static("https://controller.internal:8443"),
        sandbox_endpoint: Endpoint::from_static("https://sandbox.internal:8443"),
        identity: ModuleArtifactNodeAgentIdentity::new(node_id, "agent-1").expect("valid identity"),
        storage: StorageConfig::default(),
        poll_interval: Duration::from_millis(1_000),
        heartbeat_interval: Duration::from_millis(15_000),
    };
    let debug_repr = format!("{config:?}");
    assert!(debug_repr.contains("ArtifactNodeAgentConfig"));
    assert!(debug_repr.contains("[REDACTED]"));
}

#[test]
fn slot_supervisor_invariants() {
    let supervisor = SlotSupervisor::new("sha256:initial".to_string(), 8081, 8082);
    assert_eq!(supervisor.active_slot(), DeploymentSlot::SlotA);
    assert_eq!(supervisor.standby_slot(), DeploymentSlot::SlotB);
    assert!(matches!(supervisor.active_state(), SlotState::Serving { port: 8081, .. }));
}

#[test]
fn http_ssr_coordinator_pre_switch_safety_invariants() {
    let mut coordinator = HttpSsrSwitchingCoordinator::new("sha256:initial".to_string(), 8081, 8082);
    assert_eq!(coordinator.proxy_target_port(), 8081);
    assert_eq!(coordinator.recovery_attempts_consumed(), 0);

    coordinator.pre_stage_candidate("sha256:candidate".to_string()).unwrap();
    let receipt = coordinator.record_pre_switch_failure("sha256:candidate").unwrap();
    assert_eq!(receipt.recovery_attempts_consumed, 0);
    assert!(receipt.predecessor_capacity_retained);
}

#[test]
fn fenced_generation_coordinator_invariants() {
    let mut coordinator = FencedWorkerGenerationCoordinator::new("test_worker", 1);
    assert_eq!(coordinator.active_generation(), 1);
    assert!(coordinator.claims_permitted());
    assert_eq!(coordinator.recovery_attempts_consumed(), 0);

    coordinator.prepare_candidate(2).unwrap();
    let fence_receipt = coordinator.fence_active_generation().unwrap();
    assert_eq!(fence_receipt.fenced_generation, 1);
    assert!(!fence_receipt.claims_permitted);
}

#[test]
fn watchdog_config_defaults_are_sound() {
    let config = WatchdogConfig::default();
    assert_eq!(config.observation_window, Duration::from_secs(60));
    assert_eq!(config.probe_interval, Duration::from_secs(5));
    assert_eq!(config.probe_timeout, Duration::from_secs(2));
    assert_eq!(config.max_consecutive_failures, 3);
}
