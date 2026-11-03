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
fn default_engine_compiles_valid_script() {
    let engine = crate::create_default_engine();
    let mut scope = rhai::Scope::new();
    let result = engine.compile("test", "let a = 10; a + 5", &mut scope);
    assert!(result.is_ok());
}

#[test]
fn alloy_authoring_error_display_and_equality() {
    use crate::AlloyAuthoringError;

    assert_eq!(AlloyAuthoringError::NotFound, AlloyAuthoringError::NotFound);
    assert_ne!(AlloyAuthoringError::NotFound, AlloyAuthoringError::Invalid);

    let err = AlloyAuthoringError::RevisionConflict {
        expected_version: 3,
    };
    assert_eq!(err.to_string(), "Alloy script revision conflict");
}
