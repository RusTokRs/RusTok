use rustok_core::module::RusToKModule;

#[test]
fn crate_api_defines_current_contract_sections() {
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
        "## Current document contract",
        "## Events",
        "## Domain invariants",
        "## Adapter obligations",
        "There is no public block entity",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain current contract marker: {marker}"
        );
    }
}

#[test]
fn module_metadata_matches_contract() {
    let module = crate::PagesModule;
    assert_eq!(module.slug(), "pages");
    assert_eq!(module.name(), "Pages");
}
