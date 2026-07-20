use serde::Deserialize;

/// Embedded commerce FBA consumer registry.
///
/// The JSON file remains the machine-readable cross-module contract, while this
/// module gives runtime/composition code a typed, module-owned entry point
/// instead of re-reading an ad-hoc path.
pub const COMMERCE_FBA_REGISTRY_JSON: &str =
    include_str!("../contracts/commerce-fba-registry.json");
pub const COMMERCE_DOMAIN_PROVIDER_INVOCATION_TRACE_JSON: &str =
    include_str!("../contracts/evidence/commerce-domain-provider-invocation-trace.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommerceFbaRegistry {
    pub schema_version: u16,
    pub module: String,
    pub role: String,
    pub status: String,
    pub contract_version: String,
    pub providers: Vec<CommerceFbaProviderDependency>,
    pub evidence: CommerceFbaEvidence,
    pub rollout: CommerceFbaRollout,
}

impl CommerceFbaRegistry {
    pub fn provider(&self, module: &str) -> Option<&CommerceFbaProviderDependency> {
        self.providers
            .iter()
            .find(|provider| provider.module == module)
    }

    pub fn provider_modules(&self) -> Vec<&str> {
        self.providers
            .iter()
            .map(|provider| provider.module.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommerceFbaProviderDependency {
    pub module: String,
    pub contract_version: String,
    pub registry: String,
    pub ports: Vec<String>,
    pub profiles: Vec<String>,
    pub fallback_profiles: Vec<String>,
    pub degraded_modes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommerceFbaEvidence {
    pub local_plan: String,
    pub central_board: String,
    pub verifier: String,
    pub runtime_invocation_trace: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommerceFbaRollout {
    pub profiles: Vec<String>,
    pub remote_profile: String,
    pub boundary_status: String,
}

pub fn commerce_fba_registry() -> Result<CommerceFbaRegistry, serde_json::Error> {
    serde_json::from_str(COMMERCE_FBA_REGISTRY_JSON)
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommerceDomainProviderInvocationTrace {
    pub schema_version: u16,
    pub generated_from: String,
    pub runner: String,
    pub status: String,
    pub purpose: String,
    pub modules: Vec<CommerceDomainProviderInvocationTraceEntry>,
}

impl CommerceDomainProviderInvocationTrace {
    pub fn provider_entry(
        &self,
        provider_module: &str,
    ) -> Option<&CommerceDomainProviderInvocationTraceEntry> {
        self.modules
            .iter()
            .find(|entry| entry.provider_module == provider_module)
    }

    pub fn consumer_entries(
        &self,
        consumer_module: &str,
    ) -> Vec<&CommerceDomainProviderInvocationTraceEntry> {
        self.modules
            .iter()
            .filter(|entry| entry.consumer_module == consumer_module)
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommerceDomainProviderInvocationTraceEntry {
    pub provider_module: String,
    pub consumer_module: String,
    pub provider_registry: String,
    pub runtime_contract_smoke: String,
    pub contract_version: String,
    pub ports: Vec<String>,
    pub operations: Vec<String>,
    pub fallback_profiles: Vec<String>,
    pub degraded_modes: Vec<String>,
    pub consumer_fallback_profiles: Vec<String>,
    pub consumer_degraded_modes: Vec<String>,
}

pub fn commerce_domain_provider_invocation_trace()
-> Result<CommerceDomainProviderInvocationTrace, serde_json::Error> {
    serde_json::from_str(COMMERCE_DOMAIN_PROVIDER_INVOCATION_TRACE_JSON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_is_parseable_and_keeps_consumer_identity() {
        let registry = commerce_fba_registry().expect("commerce FBA registry parses");

        assert_eq!(registry.schema_version, 1);
        assert_eq!(registry.module, "commerce");
        assert_eq!(registry.role, "orchestrator_consumer");
        assert_eq!(registry.rollout.boundary_status, "consumer_metadata_locked");
        assert!(registry.providers.iter().any(|provider| {
            provider.module == "payment"
                && provider
                    .ports
                    .contains(&"PaymentCollectionPort".to_string())
        }));
        assert_eq!(
            registry
                .provider("product")
                .map(|provider| provider.registry.as_str()),
            Some("crates/rustok-product/contracts/product-fba-registry.json")
        );
        assert!(registry.provider_modules().contains(&"inventory"));
        assert_eq!(
            registry.evidence.runtime_invocation_trace,
            "crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json"
        );
    }

    #[test]
    fn embedded_invocation_trace_is_parseable_and_links_tax_through_cart() {
        let trace = commerce_domain_provider_invocation_trace()
            .expect("commerce-domain provider invocation trace parses");

        assert_eq!(trace.schema_version, 1);
        assert_eq!(
            trace.generated_from,
            "crates/rustok-commerce/contracts/commerce-fba-registry.json"
        );
        assert!(
            trace
                .modules
                .iter()
                .any(|entry| entry.provider_module == "product"
                    && entry.consumer_module == "commerce"
                    && entry.ports.contains(&"ProductCatalogReadPort".to_string()))
        );
        assert_eq!(
            trace
                .provider_entry("pricing")
                .map(|entry| entry.contract_version.as_str()),
            Some("pricing.read_projection.v1")
        );
        assert!(
            trace
                .consumer_entries("commerce")
                .iter()
                .any(|entry| entry.provider_module == "cart")
        );
        assert!(
            trace
                .modules
                .iter()
                .any(|entry| entry.provider_module == "tax"
                    && entry.consumer_module == "cart"
                    && entry.ports.contains(&"TaxCalculationPort".to_string()))
        );
    }
}
