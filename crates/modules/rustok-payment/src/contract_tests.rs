use rustok_core::module::{ModuleKind, RusToKModule};

use crate::PaymentModule;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public Modules",
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
    let module = PaymentModule::new();
    assert_eq!(module.slug(), "payment");
    assert_eq!(module.name(), "Payment");
    assert_eq!(module.kind(), ModuleKind::Optional);
}
