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

    use rustok_core::{ModuleKind, RusToKModule};
    let module = crate::TenantModule;
    assert_eq!(module.slug(), "tenant");
    assert_eq!(module.name(), "Tenant");
    assert_eq!(module.kind(), ModuleKind::Core);
}
