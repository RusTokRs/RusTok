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

#[test]
fn flex_module_metadata_matches_contract() {
    use rustok_core::module::RusToKModule;
    let module = crate::FlexModule;
    assert_eq!(module.slug(), "flex");
    assert_eq!(module.name(), "Flex");
    assert!(!module.description().is_empty());
}

#[test]
fn entity_type_validation_and_normalization() {
    use crate::entity_type::{
        is_valid_flex_entity_type, normalize_flex_entity_type, TAXONOMY_CATEGORY_ENTITY_TYPE,
    };

    assert!(is_valid_flex_entity_type("user"));
    assert!(is_valid_flex_entity_type(TAXONOMY_CATEGORY_ENTITY_TYPE));
    assert_eq!(
        normalize_flex_entity_type(" User ").as_deref(),
        Some("user")
    );
    assert!(!is_valid_flex_entity_type("invalid..name"));
}
