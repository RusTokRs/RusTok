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
fn alloy_operation_default_and_slug_contract() {
    use crate::AlloyOperation;
    assert_eq!(AlloyOperation::default(), AlloyOperation::ListScripts);
    assert_eq!(AlloyOperation::ListScripts.slug(), "list_scripts");
    assert_eq!(AlloyOperation::GetScript.slug(), "get_script");
    assert_eq!(AlloyOperation::ValidateScript.slug(), "validate_script");
    assert_eq!(AlloyOperation::RunScript.slug(), "run_script");
}

