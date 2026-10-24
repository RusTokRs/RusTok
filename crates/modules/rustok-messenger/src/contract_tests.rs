use rustok_core::module::{ModuleKind, RusToKModule};

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
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
fn module_metadata_matches_contract() {
    let module = crate::MessengerModule::new();
    assert_eq!(module.slug(), "messenger");
    assert_eq!(module.name(), "Messenger & Team Chat");
    assert_eq!(module.kind(), ModuleKind::Optional);
}
