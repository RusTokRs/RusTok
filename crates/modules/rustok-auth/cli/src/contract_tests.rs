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
fn auth_cli_command_provider_descriptors() {
    use crate::AuthCommandProvider;
    use rustok_cli_core::CommandProvider;
    use rustok_runtime::RuntimeComposition;

    let provider = AuthCommandProvider {
        runtime: RuntimeComposition::without_database(serde_json::json!({})),
    };
    let commands = provider.commands();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].namespace, "oauth");
    assert_eq!(commands[0].name, "create-app");
    assert_eq!(commands[1].namespace, "auth");
    assert_eq!(commands[1].name, "sessions-cleanup");
}
