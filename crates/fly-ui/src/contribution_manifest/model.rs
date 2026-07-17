use crate::ContributionDescriptor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleContributionManifest {
    pub module_id: String,
    pub owner_provider: String,
    #[serde(default)]
    pub target_providers: BTreeSet<String>,
    #[serde(default)]
    pub dependencies: BTreeSet<String>,
    #[serde(default)]
    pub required_permissions: BTreeSet<String>,
    #[serde(default)]
    pub admin: Vec<ContributionDescriptor>,
    #[serde(default)]
    pub storefront: Vec<ContributionDescriptor>,
}

impl ModuleContributionManifest {
    pub fn allows_target_provider(&self, provider: &str) -> bool {
        let provider = provider.trim();
        provider == self.owner_provider.trim() || self.target_providers.contains(provider)
    }
}
