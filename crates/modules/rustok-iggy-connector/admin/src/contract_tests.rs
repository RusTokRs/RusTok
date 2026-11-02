#[cfg(test)]
mod tests {
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
    fn test_crate_api_compliance() {
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
}

