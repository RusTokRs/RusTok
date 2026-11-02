use std::time::Duration;

use rustok_modules::OciArtifactReference;
use uuid::Uuid;

use crate::publisher::{build_cyclonedx_sbom, build_slsa_provenance};
use crate::{
    StaticDistributionJobConfig, StaticDistributionPublisherConfig,
    StaticDistributionPublisherRequest, StaticDistributionWorker,
};

const VALID_HEX_64: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn valid_digest() -> String {
    format!("sha256:{VALID_HEX_64}")
}

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest_bytes = include_bytes!("../Cargo.toml");
    let manifest_text =
        std::str::from_utf8(manifest_bytes).expect("Cargo.toml must be valid UTF-8");

    assert!(manifest_text.contains(r#"name = "rustok-static-distribution-worker""#));
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

fn sample_job_config() -> StaticDistributionJobConfig {
    let root = std::env::current_dir().expect("current dir");
    StaticDistributionJobConfig {
        contract: "rustok.static_distribution.job_config".to_string(),
        cas_root: root.join("cas"),
        cargo_path: root.join("cargo"),
        cargo_digest: valid_digest(),
        rustc_path: root.join("rustc"),
        rustc_digest: valid_digest(),
        cargo_home: root.join("cargo-home"),
        publisher_path: root.join("publisher"),
        publisher_digest: valid_digest(),
        publisher_config_path: root.join("publisher.json"),
        publisher_config_digest: valid_digest(),
        toolchain_digest: valid_digest(),
        build_target: "x86_64-unknown-linux-gnu".to_string(),
        max_archive_bytes: 50 * 1024 * 1024,
        max_source_extracted_bytes: 100 * 1024 * 1024,
        max_total_extracted_bytes: 200 * 1024 * 1024,
        max_archive_entries: 10_000,
        command_timeout_seconds: 3600,
    }
}

#[test]
fn job_config_validation_invariants() {
    let mut config = sample_job_config();
    assert!(config.validate_fields().is_ok());

    config.contract = "invalid.contract".to_string();
    assert!(config.validate_fields().is_err());

    config = sample_job_config();
    config.cargo_digest = "short_digest".to_string();
    assert!(config.validate_fields().is_err());

    config = sample_job_config();
    config.build_target = "invalid target with spaces".to_string();
    assert!(config.validate_fields().is_err());

    config = sample_job_config();
    config.command_timeout_seconds = 0;
    assert!(config.validate_fields().is_err());

    config = sample_job_config();
    config.command_timeout_seconds = 3 * 60 * 60; // exceeds max 2 hours
    assert!(config.validate_fields().is_err());
}

fn sample_publisher_config() -> StaticDistributionPublisherConfig {
    let root = std::env::current_dir().expect("current dir");
    StaticDistributionPublisherConfig {
        contract: "rustok.static_distribution.publisher_config".to_string(),
        registry: "registry.example.com".to_string(),
        repository: "modules/dist".to_string(),
        artifact_file_name: "rustok-server".to_string(),
        credential_broker_path: root.join("broker"),
        credential_broker_digest: valid_digest(),
        cosign_path: root.join("cosign"),
        cosign_digest: valid_digest(),
        cosign_key_reference: "awskms:///arn:aws:kms:us-east-1:123456789012:key/sample".to_string(),
        max_artifact_bytes: 500 * 1024 * 1024,
        max_evidence_bytes: 10 * 1024 * 1024,
        publication_timeout_seconds: 600,
    }
}

#[test]
fn publisher_config_validation_invariants() {
    let mut config = sample_publisher_config();
    assert!(config.validate_fields().is_ok());

    config.contract = "wrong.contract".to_string();
    assert!(config.validate_fields().is_err());

    config = sample_publisher_config();
    config.artifact_file_name = "../invalid_path".to_string();
    assert!(config.validate_fields().is_err());

    config = sample_publisher_config();
    config.cosign_digest = "invalid".to_string();
    assert!(config.validate_fields().is_err());

    config = sample_publisher_config();
    config.publication_timeout_seconds = 0;
    assert!(config.validate_fields().is_err());
}

#[test]
fn static_distribution_worker_debug_formatting() {
    let root = std::env::current_dir().expect("current dir");
    let worker = StaticDistributionWorker::new_for_test(
        root.join("launcher"),
        valid_digest(),
        root.join("config.json"),
        valid_digest(),
        root.join("work"),
        valid_digest(),
        "x86_64-unknown-linux-gnu".to_string(),
        Duration::from_secs(300),
    );
    let debug = format!("{worker:?}");
    assert!(debug.contains("StaticDistributionWorker"));
    assert!(debug.contains("launcher_path"));
    assert!(debug.contains("build_target"));
}

#[test]
fn cyclonedx_sbom_generation_from_cargo_lock() {
    let request = StaticDistributionPublisherRequest {
        contract: "rustok.static_distribution.publisher_request".to_string(),
        distribution_build_id: Uuid::new_v4(),
        claim_id: Uuid::new_v4(),
        attempt_number: 1,
        job_request_digest: valid_digest(),
        generated_output_digest: valid_digest(),
        composition_digest: valid_digest(),
        toolchain_digest: valid_digest(),
        build_target: "x86_64-unknown-linux-gnu".to_string(),
        resolved_lock_digest: valid_digest(),
        test_evidence_digest: valid_digest(),
    };

    let sample_lock = r#"
version = 3

[[package]]
name = "serde"
version = "1.0.210"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "c8e35924721c155954ae1297274da379d237c0eb095ad6561f3a733974f7f849"

[[package]]
name = "tokio"
version = "1.40.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "52e991066e2ca60c7712a14980a63445b257b3c07929247f57e0ff8160164302"
"#;

    let sbom_bytes = build_cyclonedx_sbom(&request, sample_lock.as_bytes(), 1024 * 1024)
        .expect("CycloneDX SBOM generation should succeed");

    let sbom: serde_json::Value =
        serde_json::from_slice(&sbom_bytes).expect("SBOM output must be valid JSON");

    assert_eq!(sbom["bomFormat"], "CycloneDX");
    assert_eq!(sbom["specVersion"], "1.6");
    let components = sbom["components"]
        .as_array()
        .expect("components should be an array");
    assert_eq!(components.len(), 2);

    // Verify components are sorted by bom-ref
    let ref0 = components[0]["bom-ref"].as_str().unwrap();
    let ref1 = components[1]["bom-ref"].as_str().unwrap();
    assert!(ref0 <= ref1);

    // Verify invalid TOML returns error
    let invalid_lock = b"not a valid toml file [[package";
    assert!(build_cyclonedx_sbom(&request, invalid_lock, 1024 * 1024).is_err());
}

#[test]
fn slsa_provenance_generation() {
    let request = StaticDistributionPublisherRequest {
        contract: "rustok.static_distribution.publisher_request".to_string(),
        distribution_build_id: Uuid::new_v4(),
        claim_id: Uuid::new_v4(),
        attempt_number: 1,
        job_request_digest: valid_digest(),
        generated_output_digest: valid_digest(),
        composition_digest: valid_digest(),
        toolchain_digest: valid_digest(),
        build_target: "x86_64-unknown-linux-gnu".to_string(),
        resolved_lock_digest: valid_digest(),
        test_evidence_digest: valid_digest(),
    };

    let artifact = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "modules/dist".to_string(),
        digest: valid_digest(),
    };

    let provenance_bytes = build_slsa_provenance(&request, &artifact, &valid_digest())
        .expect("SLSA provenance generation should succeed");

    let statement: serde_json::Value =
        serde_json::from_slice(&provenance_bytes).expect("Provenance must be valid JSON");

    assert_eq!(statement["_type"], "https://in-toto.io/Statement/v1");
    assert_eq!(statement["predicateType"], "https://slsa.dev/provenance/v1");
    assert_eq!(
        statement["subject"][0]["digest"]["sha256"],
        VALID_HEX_64
    );
    assert_eq!(
        statement["predicate"]["buildDefinition"]["buildType"],
        "https://rustok.dev/build-types/static-distribution"
    );
    assert_eq!(
        statement["predicate"]["runDetails"]["builder"]["id"],
        "https://rustok.dev/builders/static-distribution"
    );
}
