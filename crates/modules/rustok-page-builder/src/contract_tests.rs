use rustok_core::module::{ModuleKind, RusToKModule};

use crate::PageBuilderModule;

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
fn module_contract_metadata_matches() {
    let module = PageBuilderModule::new();
    assert_eq!(module.slug(), "page_builder");
    assert_eq!(module.name(), "Page Builder");
    assert_eq!(module.kind(), ModuleKind::Optional);
}
