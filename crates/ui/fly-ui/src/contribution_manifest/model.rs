use crate::ContributionDescriptor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Module-owned contribution metadata with an explicit, version-pinned target-provider allowlist.
///
/// `owner_provider` identifies the module that owns lifecycle, policy and health. A contribution's
/// `provider` identifies the component contract that its renderer/editor extends. When
/// `target_providers` is empty, only the owner provider is allowed. Cross-provider extensions must
/// therefore be declared intentionally, for example `rustok.pages -> fly.builtin@1`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleContributionManifest {
    pub module_id: String,
    pub owner_provider: String,
    pub owner_version: String,
    #[serde(default)]
    pub target_providers: BTreeMap<String, String>,
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
    pub fn allows_target_provider(&self, provider: &str, version: &str) -> bool {
        let provider = provider.trim();
        let version = version.trim();
        if provider == self.owner_provider.trim() && version == self.owner_version.trim() {
            return true;
        }
        self.target_providers
            .get(provider)
            .is_some_and(|allowed| allowed.trim() == version)
    }
}
