use std::error::Error;

use crate::{
    ArtifactDescriptorError, BuildEvidenceError, CargoMetadataError, ComponentArtifactError,
    DependencyMaterializationError, PublicationBundleError, SourceMaterializationError,
    SourcePolicyError, WitContractError,
};

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Role",
        "## Public surface",
        "## Configuration & Environment",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
        "## Verification & Test Coverage",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

fn assert_error_traits<E: Error + Send + Sync + 'static>() {}

#[test]
fn error_types_implement_standard_error_and_send_sync_traits() {
    assert_error_traits::<SourceMaterializationError>();
    assert_error_traits::<DependencyMaterializationError>();
    assert_error_traits::<SourcePolicyError>();
    assert_error_traits::<CargoMetadataError>();
    assert_error_traits::<ComponentArtifactError>();
    assert_error_traits::<BuildEvidenceError>();
    assert_error_traits::<WitContractError>();
    assert_error_traits::<PublicationBundleError>();
    assert_error_traits::<ArtifactDescriptorError>();
}

#[test]
fn error_types_display_informative_messages() {
    assert_eq!(
        SourceMaterializationError::DigestMismatch.to_string(),
        "source archive digest mismatch"
    );
    assert_eq!(
        SourceMaterializationError::UnsafeArchive.to_string(),
        "unsafe source archive rejected by policy"
    );
    assert_eq!(
        DependencyMaterializationError::EndpointDenied.to_string(),
        "dependency endpoint denied by network policy"
    );
    assert_eq!(
        SourcePolicyError::BuildScriptDenied.to_string(),
        "build script execution denied by policy"
    );
    assert_eq!(
        CargoMetadataError::NativeLinkDenied.to_string(),
        "cargo metadata native link denied by policy"
    );
    assert_eq!(
        ComponentArtifactError::InspectionFailed.to_string(),
        "component artifact inspection failed"
    );
    assert_eq!(
        BuildEvidenceError::SbomInvalid.to_string(),
        "SBOM evidence is invalid"
    );
    assert_eq!(
        WitContractError::Mismatch.to_string(),
        "WIT contract mismatch"
    );
    assert_eq!(
        PublicationBundleError::Invalid.to_string(),
        "publication bundle is invalid"
    );
    assert_eq!(
        ArtifactDescriptorError::Invalid.to_string(),
        "artifact descriptor is invalid"
    );
}

#[test]
fn debug_derives_are_implemented_for_worker_types() {
    assert!(!format!("{:?}", ComponentArtifactError::ResourceLimit).is_empty());
    assert!(!format!("{:?}", BuildEvidenceError::ProvenanceInvalid).is_empty());
    assert!(!format!("{:?}", WitContractError::ResourceLimit).is_empty());
    assert!(!format!("{:?}", PublicationBundleError::ResourceLimit).is_empty());
    assert!(!format!("{:?}", ArtifactDescriptorError::ResourceLimit).is_empty());
}
