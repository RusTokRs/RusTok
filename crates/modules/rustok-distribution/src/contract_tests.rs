use uuid::Uuid;

use crate::{
    CompositionIdentity, CompositionModule, CompositionModuleKind,
    GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH, GENERATED_DISTRIBUTION_MANIFEST_PATH,
    GENERATED_DISTRIBUTION_REGISTRY_PATH, GeneratedStaticDistributionManifest,
    GeneratedStaticDistributionSource, StaticDistributionGenerationError, build_registry,
    composition_identity,
};

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Public Surface"));
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}

#[test]
fn test_composition_identity_contract() {
    let identity: CompositionIdentity = composition_identity();
    assert!(!identity.revision.is_empty());
    assert_eq!(identity.hash.len(), 64);
    assert!(
        identity
            .hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert!(!identity.modules.is_empty());

    // Verify serialization roundtrip
    let json = serde_json::to_string(&identity).expect("composition identity must serialize");
    assert!(json.contains("\"revision\":"));
    assert!(json.contains("\"hash\":"));
    assert!(json.contains("\"modules\":"));
}

#[test]
fn test_composition_module_traits() {
    let module = CompositionModule {
        slug: "auth".to_string(),
        version: "0.1.0".to_string(),
        kind: CompositionModuleKind::Core,
        dependencies: vec!["tenant".to_string()],
    };

    let cloned = module.clone();
    assert_eq!(module, cloned);
    assert_eq!(format!("{module:?}"), format!("{cloned:?}"));

    let json = serde_json::to_string(&module).expect("composition module must serialize");
    assert!(json.contains("\"slug\":\"auth\""));
    assert!(json.contains("\"kind\":\"core\""));

    let optional_module = CompositionModule {
        slug: "cart".to_string(),
        version: "0.1.0".to_string(),
        kind: CompositionModuleKind::Optional,
        dependencies: vec![],
    };
    let opt_json = serde_json::to_string(&optional_module).expect("optional module must serialize");
    assert!(opt_json.contains("\"kind\":\"optional\""));
}

#[test]
fn test_generated_distribution_constants() {
    assert_eq!(
        GENERATED_DISTRIBUTION_MANIFEST_PATH,
        ".rustok/generated/static-distribution.json"
    );
    assert_eq!(
        GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH,
        "crates/modules/rustok-distribution/Cargo.toml"
    );
    assert_eq!(
        GENERATED_DISTRIBUTION_REGISTRY_PATH,
        "crates/modules/rustok-distribution/src/generated_promotions.rs"
    );
}

#[test]
fn test_static_distribution_generation_error_formatting() {
    let err1 = StaticDistributionGenerationError::InvalidWorkItem("bad input".to_string());
    assert_eq!(
        err1.to_string(),
        "static distribution work item is invalid: bad input"
    );

    let err2 = StaticDistributionGenerationError::Serialization("serde failed".to_string());
    assert_eq!(
        err2.to_string(),
        "static distribution generated manifest could not be serialized: serde failed"
    );
}

#[test]
fn test_generated_static_distribution_manifest_serialization() {
    let manifest = GeneratedStaticDistributionManifest {
        distribution_build_id: Uuid::nil(),
        claim_id: Uuid::nil(),
        attempt_number: 1,
        runner_id: "test-runner".to_string(),
        composition_revision: 42,
        composition_digest: "sha256:abc".to_string(),
        platform_source_reference: "main".to_string(),
        platform_source_digest: "sha256:def".to_string(),
        toolchain_digest: "sha256:123".to_string(),
        build_target: "x86_64-unknown-linux-gnu".to_string(),
        manifest_path: GENERATED_DISTRIBUTION_MANIFEST_PATH.to_string(),
        cargo_manifest_path: GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH.to_string(),
        registry_source_path: GENERATED_DISTRIBUTION_REGISTRY_PATH.to_string(),
        sources: vec![GeneratedStaticDistributionSource {
            ordinal: 0,
            promotion_id: Uuid::nil(),
            promotion_revision: 1,
            release_id: "rel-1".to_string(),
            module_slug: "test-mod".to_string(),
            module_version: "0.1.0".to_string(),
            cargo_package: "rustok-test-mod".to_string(),
            entry_type: "TestModModule".to_string(),
            dependency_alias: "rustok_static_promotion_000".to_string(),
            source_reference: "tag/v0.1.0".to_string(),
            source_digest: "sha256:src".to_string(),
            dependency_lock_digest: "sha256:lock".to_string(),
            executor_mode: rustok_modules::ModuleStaticDistributionExecutorMode::StaticNative,
            materialization_path: ".rustok/static-sources/000".to_string(),
        }],
        output_digest: "sha256:out".to_string(),
    };

    let cloned = manifest.clone();
    assert_eq!(manifest, cloned);

    let json = serde_json::to_string(&manifest).expect("manifest must serialize");
    let deserialized: GeneratedStaticDistributionManifest =
        serde_json::from_str(&json).expect("manifest must deserialize");
    assert_eq!(manifest, deserialized);
}

#[test]
fn test_build_registry_contains_core_modules() {
    let registry = build_registry();
    let modules = registry.list();
    let slugs: Vec<&str> = modules.iter().map(|m| m.slug()).collect();

    // Mandatory core modules
    assert!(slugs.contains(&"modules"));
    assert!(slugs.contains(&"auth"));
    assert!(slugs.contains(&"cache"));
    assert!(slugs.contains(&"channel"));
    assert!(slugs.contains(&"email"));
    assert!(slugs.contains(&"index"));
    assert!(slugs.contains(&"search"));
    assert!(slugs.contains(&"outbox"));
    assert!(slugs.contains(&"events"));
    assert!(slugs.contains(&"tenant"));
    assert!(slugs.contains(&"rbac"));
    assert!(slugs.contains(&"social_graph"));
}
