#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("readme = \"README.md\""),
        "manifest must declare readme"
    );
    assert!(
        manifest.contains("repository.workspace = true"),
        "manifest must inherit repository from workspace"
    );
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "manifest must inherit rust-version from workspace"
    );
    assert!(
        manifest.contains("[package.metadata.docs.rs]"),
        "manifest must configure docs.rs metadata"
    );
}

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

    assert!(
        !transport.contains("pub async fn replay"),
        "a replay API must not report success until it executes a broker-backed replay"
    );
}

#[test]
fn dlq_exposes_only_payload_preserving_retry() {
    let dlq = include_str!("dlq.rs");
    assert!(
        !dlq.contains("retry_from_dlq"),
        "an ID-only DLQ retry cannot preserve the payload and must not report success"
    );
}
