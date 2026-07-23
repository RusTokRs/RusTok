#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
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
fn transport_exposes_only_persistent_consumer_group_delivery() {
    let transport = include_str!("transport.rs");
    for removed_api in [
        "subscribe_as_group",
        "consume_next_as_group",
        "ack_consumed",
    ] {
        assert!(
            !transport.contains(removed_api),
            "legacy consumer API {removed_api} must not be reintroduced"
        );
    }
}
