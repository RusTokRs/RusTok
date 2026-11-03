#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[cfg(feature = "athanor")]
#[test]
fn athanor_config_validation_and_constants() {
    use crate::{ATHANOR_SOURCE_ID, AthanorRagConfig};
    use rustok_ai::RagError;
    use std::path::PathBuf;
    use uuid::Uuid;

    assert_eq!(ATHANOR_SOURCE_ID, "athanor");

    let tenant_id = Uuid::new_v4();
    let zero_result = AthanorRagConfig::new(PathBuf::from("/tmp"), tenant_id, 0);
    assert!(matches!(zero_result, Err(RagError::InvalidLimit { max: 0 })));

    let valid_config = AthanorRagConfig::new(PathBuf::from("/tmp/rag"), tenant_id, 16)
        .expect("config should be valid");
    assert_eq!(valid_config.max_atoms, 16);
    assert_eq!(valid_config.tenant_id, tenant_id);
    assert_eq!(valid_config.root, PathBuf::from("/tmp/rag"));
}


