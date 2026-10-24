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
fn tasks_module_contract_metadata_matches() {
    use rustok_core::module::{ModuleKind, RusToKModule};
    let module = crate::TasksModule::new();
    assert_eq!(module.slug(), "tasks");
    assert_eq!(module.name(), "Team Issues & Tasks");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["outbox"]);
}
