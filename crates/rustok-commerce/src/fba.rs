use serde::Deserialize;

/// Embedded commerce FBA consumer registry.
///
/// The JSON file remains the machine-readable cross-module contract, while this
/// module gives runtime/composition code a typed, module-owned entry point
/// instead of re-reading an ad-hoc path.
pub const COMMERCE_FBA_REGISTRY_JSON: &str =
    include_str!("../contracts/commerce-fba-registry.json");

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
        assert!(registry
            .providers
            .iter()
            .any(|provider| provider.module == "payment"
                && provider
                    .ports
                    .contains(&"PaymentCollectionPort".to_string())));
    }
}
