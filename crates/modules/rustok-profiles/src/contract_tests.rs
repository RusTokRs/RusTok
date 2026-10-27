#[test]
fn profiles_module_matches_canonical_contract() {
    let api = include_str!("../CRATE_API.md");
    assert!(api.contains("## Public surface"));
    assert!(api.contains("## Platform Capability Bindings"));
    assert!(api.contains("## Domain Collaborations"));
    assert!(api.contains("## Minimum Contract Set"));
    assert!(api.contains("### Input DTOs/Commands"));
    assert!(api.contains("### Domain Invariants"));
    assert!(api.contains("### Events / Outbox Side Effects"));
    assert!(api.contains("### Errors / Failure Codes"));

    use rustok_core::{ModuleKind, RusToKModule};
    let module = crate::ProfilesModule;
    assert_eq!(module.slug(), "profiles");
    assert_eq!(module.name(), "Profiles");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["media", "social_graph", "taxonomy"]);
}
