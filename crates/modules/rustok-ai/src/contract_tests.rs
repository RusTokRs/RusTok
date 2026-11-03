#[cfg(test)]
mod tests {
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
            assert!(
                api.contains(section),
                "CRATE_API.md must contain '{section}'"
            );
        }
    }

    #[test]
    fn test_agent_kind_slugs() {
        use crate::agent::AgentKind;
        assert_eq!(AgentKind::Product.slug(), "product");
        assert_eq!(AgentKind::Code.slug(), "code");
        assert_eq!(AgentKind::Orchestrator.slug(), "orchestrator");
        assert_eq!(AgentKind::Review.slug(), "review");
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_ai_module_descriptor() {
        use rustok_core::RusToKModule;
        let module = crate::AiModule;
        assert_eq!(module.slug(), "ai");
        assert_eq!(module.name(), "AI");
        assert_eq!(module.kind(), rustok_core::ModuleKind::Core);
    }
}
