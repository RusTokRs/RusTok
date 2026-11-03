use std::sync::Arc;

use crate::{
    CapabilityBrokerRouter, CapabilityName, ExecutorRegistry, FixtureCapabilityBroker,
    LocalSandboxHarness, SandboxAdmissionLimits, SandboxCancellation, SandboxLimits, SandboxPolicy,
    SandboxRuntime,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("readme = \"README.md\""),
        "manifest must declare readme"
    );
    assert!(
        manifest.contains("repository.workspace = true"),
        "manifest must inherit repository from workspace"
    );
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "manifest must inherit rust-version from workspace"
    );
    assert!(
        manifest.contains("[package.metadata.docs.rs]"),
        "manifest must configure docs.rs metadata"
    );
}

#[test]
fn crate_api_contract_specification_is_complete() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(
        api_doc.contains("# `rustok-sandbox` Crate API & Architecture Contract"),
        "CRATE_API.md must contain the main title"
    );
    assert!(
        api_doc.contains("## 1. Architectural Purpose & Boundaries"),
        "CRATE_API.md must document purpose and boundaries"
    );
    assert!(
        api_doc.contains("## 2. Feature Flags"),
        "CRATE_API.md must document feature flags"
    );
    assert!(
        api_doc.contains("## 3. Core Types & Invariants"),
        "CRATE_API.md must document core types"
    );
    assert!(
        api_doc.contains("## 4. Capability System & Policy Enforcement"),
        "CRATE_API.md must document capability system"
    );
    assert!(
        api_doc.contains("## 5. Universal Logic Dimensions Verification"),
        "CRATE_API.md must document universal logic dimensions"
    );
}

#[test]
fn debug_formatting_implementations() {
    let registry = ExecutorRegistry::new();
    let debug_registry = format!("{registry:?}");
    assert!(debug_registry.contains("ExecutorRegistry"));

    let broker = Arc::new(CapabilityBrokerRouter::new());
    let debug_router = format!("{broker:?}");
    assert!(debug_router.contains("CapabilityBrokerRouter"));

    let runtime = SandboxRuntime::new(registry.clone(), broker);
    let debug_runtime = format!("{runtime:?}");
    assert!(debug_runtime.contains("SandboxRuntime"));

    let fixtures = FixtureCapabilityBroker::default();
    let debug_fixtures = format!("{fixtures:?}");
    assert!(debug_fixtures.contains("FixtureCapabilityBroker"));

    let harness = LocalSandboxHarness::new(registry);
    let debug_harness = format!("{harness:?}");
    assert!(debug_harness.contains("LocalSandboxHarness"));

    #[cfg(feature = "rhai")]
    {
        let rhai_executor = crate::rhai::RhaiExecutor::new();
        let debug_rhai = format!("{rhai_executor:?}");
        assert!(debug_rhai.contains("RhaiExecutor"));
    }
}

#[test]
fn default_limits_and_admission_invariants() {
    let limits = SandboxLimits::default();
    assert_eq!(limits.wall_clock_ms, 100);
    assert_eq!(limits.instruction_budget, 50_000);
    assert_eq!(limits.max_memory_bytes, 64 * 1024 * 1024);
    assert_eq!(limits.max_output_bytes, 1024 * 1024);
    assert_eq!(limits.max_concurrency, 1);
    assert_eq!(limits.max_capability_calls, 16);
    assert_eq!(limits.max_capability_input_bytes, 64 * 1024);
    assert_eq!(limits.max_capability_calls_per_second, 16);

    let admission = SandboxAdmissionLimits::default();
    assert_eq!(admission.global, 64);
    assert_eq!(admission.per_executor, 32);
    assert_eq!(admission.per_tenant, 16);
    assert_eq!(admission.per_artifact, 8);

    let policy = SandboxPolicy::default();
    assert!(policy.grants.is_empty());
    assert_eq!(policy.limits, limits);
}

#[test]
fn capability_name_validation_invariants() {
    let valid_names = [
        "platform.events",
        "platform.http",
        "platform.data",
        "platform.data.objects",
        "platform.secrets",
        "platform.mcp",
        "tenant:service_v1",
        "custom_tool_42",
    ];

    for name in valid_names {
        assert!(
            CapabilityName::new(name).is_ok(),
            "expected '{name}' to be a valid capability name"
        );
        let cap = CapabilityName::new(name).unwrap();
        assert_eq!(cap.as_str(), name);
        assert_eq!(format!("{cap}"), name);
    }

    let overlong = "a".repeat(97);
    let invalid_names = [
        "",
        "Platform.Events",
        "platform events",
        "platform/http",
        "platform;inject",
        overlong.as_str(),
    ];

    for name in invalid_names {
        assert!(
            CapabilityName::new(name).is_err(),
            "expected '{name}' to be rejected as invalid capability name"
        );
    }
}

#[test]
fn cooperative_cancellation_lifecycle() {
    let cancellation = SandboxCancellation::new();
    assert!(!cancellation.is_cancelled());
    cancellation.cancel();
    assert!(cancellation.is_cancelled());

    let clone = cancellation.clone();
    assert!(clone.is_cancelled());
}
