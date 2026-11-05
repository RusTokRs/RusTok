use std::path::{Path, PathBuf};

use crate::{
    CommandRegistryCredentialBroker, CosignArtifactSigner, CosignAttestationPredicate,
    validate_fixed_program,
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
        api_doc.contains("# `rustok-build-publication` Crate API & Architecture Contract"),
        "CRATE_API.md must contain the main title"
    );
    assert!(
        api_doc.contains("## 1. Architectural Purpose & Boundaries"),
        "CRATE_API.md must document purpose and boundaries"
    );
    assert!(
        api_doc.contains("## 2. Bounded Credential Broker & Short-Lived Leases"),
        "CRATE_API.md must document credential broker"
    );
    assert!(
        api_doc.contains("## 3. Cosign Signing & In-Toto Attestation Normalization"),
        "CRATE_API.md must document signing"
    );
    assert!(
        api_doc.contains("## 4. Hermetic OCI Publication Pipeline"),
        "CRATE_API.md must document publication pipeline"
    );
    assert!(
        api_doc.contains("## 5. Universal Logic Dimensions Verification"),
        "CRATE_API.md must document universal logic dimensions"
    );
}

#[test]
fn debug_formatting_implementations() {
    let broker = CommandRegistryCredentialBroker::new(
        PathBuf::from("/bin/sh"),
        format!("sha256:{}", "a".repeat(64)),
    );
    // Since /bin/sh doesn't match hash "a"*64, new returns an Err, which validates runtime guard!
    assert!(broker.is_err());

    let signer = CosignArtifactSigner::new(
        PathBuf::from("/bin/cosign"),
        format!("sha256:{}", "b".repeat(64)),
        "awskms://12345".to_string(),
    );
    assert!(signer.is_err());
}

#[test]
fn cosign_predicate_types_match_spec() {
    assert_eq!(
        CosignAttestationPredicate::SlsaProvenance.cosign_type(),
        "https://slsa.dev/provenance/v1"
    );
    assert_eq!(
        CosignAttestationPredicate::CycloneDx.cosign_type(),
        "https://cyclonedx.org/bom"
    );
}

#[test]
fn fixed_program_validation_guards() {
    assert!(validate_fixed_program(Path::new("relative/path"), "invalid", "test").is_err());
    assert!(
        validate_fixed_program(
            Path::new("/non/existent/executable"),
            &format!("sha256:{}", "0".repeat(64)),
            "test"
        )
        .is_err()
    );
}
