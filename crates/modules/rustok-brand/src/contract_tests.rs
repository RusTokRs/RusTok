#[test]
fn brand_module_matches_canonical_contract() {
    let api = include_str!("../CRATE_API.md");
    assert!(api.contains("## 1. Platform Capability Bindings"));
    assert!(api.contains("## 2. Domain Collaborations"));
    assert!(api.contains("## 3. Minimum Contract Set"));
    assert!(api.contains("## 4. Input DTOs / Commands"));
    assert!(api.contains("## 5. Domain Invariants"));
    assert!(api.contains("## 6. Events / Outbox Side Effects"));
    assert!(api.contains("## 7. Errors / Failure Codes"));

    use rustok_core::{ModuleKind, RusToKModule};
    let module = crate::BrandModule;
    assert_eq!(module.slug(), "brand");
    assert_eq!(module.name(), "Brand");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["product"]);
}
