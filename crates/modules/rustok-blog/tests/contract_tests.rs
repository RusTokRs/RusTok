#[test]
fn test_blog_crate_api_compliance() {
    let api = include_str!("../CRATE_API.md");
    for section in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Public surface",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(api.contains(section), "CRATE_API.md must contain '{section}'");
    }
}

