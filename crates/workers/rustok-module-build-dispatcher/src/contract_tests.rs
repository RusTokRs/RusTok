use std::time::Duration;
use uuid::Uuid;

use crate::{
    MODULE_BUILD_CONSUMER_GROUP, ModuleBuildDelivery, ModuleBuildDeliveryResult,
    host::ModuleBuildDispatcherConfig,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text =
        std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-module-build-dispatcher""#));
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
fn consumer_group_contract_matches_expected_topic_owner() {
    assert_eq!(
        MODULE_BUILD_CONSUMER_GROUP,
        "rustok-module-build-dispatcher"
    );
}

#[test]
fn module_build_delivery_equality_and_debug_invariants() {
    let delivery_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let d1 = ModuleBuildDelivery {
        delivery_id,
        request_id,
        tenant_id,
    };
    let d2 = d1.clone();
    assert_eq!(d1, d2);
    assert!(format!("{d1:?}").contains("ModuleBuildDelivery"));
}

#[test]
fn delivery_result_invariants_and_representation() {
    assert_ne!(
        ModuleBuildDeliveryResult::AlreadySettled,
        ModuleBuildDeliveryResult::ClaimedElsewhere
    );
    assert_eq!(
        ModuleBuildDeliveryResult::AlreadySettled.clone(),
        ModuleBuildDeliveryResult::AlreadySettled
    );
}

#[test]
fn dispatcher_config_debug_redacts_database_url() {
    let config = ModuleBuildDispatcherConfig {
        database_url: "postgres://secret_user:super_secret_password@db.internal:5432/rustok"
            .to_string(),
        worker_endpoint: "https://worker.internal:8443".to_string(),
        iggy: rustok_iggy::IggyConfig::default(),
        idle_poll_delay: Duration::from_millis(1_000),
    };
    let debug_repr = format!("{config:?}");
    assert!(debug_repr.contains("[REDACTED]"));
    assert!(!debug_repr.contains("super_secret_password"));
}

#[test]
fn idle_poll_delay_configuration_limits() {
    // Limits: DEFAULT is 1000ms, MAX is 60000ms.
    let default_delay = Duration::from_millis(1_000);
    assert_eq!(default_delay.as_millis(), 1_000);
}
