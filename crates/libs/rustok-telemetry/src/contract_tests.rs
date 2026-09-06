#[test]
fn crate_api_defines_public_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public modules",
        "## Primary public types and functions",
        "## Contract invariants",
        "## Errors",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}
