#[test]
fn crate_api_defines_current_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Current document contract",
        "## Events",
        "## Domain invariants",
        "## Adapter obligations",
        "There is no public block entity",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain current contract marker: {marker}"
        );
    }
}
