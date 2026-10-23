#[test]
fn tax_module_matches_canonical_contract() {
    let api = include_str!("../CRATE_API.md");
    assert!(api.contains("## Platform Capability Bindings"));
    assert!(api.contains("## Domain Collaborations"));
    assert!(api.contains("## Minimum Contract Set"));
    assert!(api.contains("### Input DTOs/Commands"));
    assert!(api.contains("### Domain Invariants"));
    assert!(api.contains("### Events / Outbox Side Effects"));
    assert!(api.contains("### Errors / Failure Codes"));

    use rustok_core::{ModuleKind, RusToKModule};
    let module = crate::TaxModule;
    assert_eq!(module.slug(), "tax");
    assert_eq!(module.name(), "Tax");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert!(module.dependencies().is_empty());
}
