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
