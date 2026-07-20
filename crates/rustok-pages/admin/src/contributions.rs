use fly_ui::{
    ContributionAssemblyPolicy, ContributionAssemblyResult, ContributionDescriptor,
    ModuleContributionManifest, build_admin_contribution_registry_from_manifests,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const PAGES_MODULE_ID: &str = "pages";
pub const PAGES_OWNER_PROVIDER: &str = "rustok.pages";
pub const FLY_BUILTIN_PROVIDER: &str = "fly.builtin";
pub const PAGES_LANDING_BLOCKS_CONTRIBUTION_ID: &str = "rustok.pages.landing-blocks";

pub const PAGES_BUILDER_CAPABILITIES: &[&str] = &["preview", "tree", "properties", "publish"];

pub const PAGES_LANDING_BLOCK_CAPABILITIES: &[&str] = &["tree", "properties"];

pub const PAGES_LANDING_BLOCK_IDS: &[&str] = &[
    "fly.hero",
    "fly.two_columns",
    "fly.feature_grid",
    "fly.cta",
    "fly.contact_form",
];

/// Module-owned metadata used by the generated Fly admin contribution registry.
///
/// Pages owns document lifecycle, while the referenced blocks belong to `fly.builtin`. The
/// cross-provider relationship is explicit; no renderer or property editor is advertised until
/// Pages has a real executable adapter for that provider.
pub fn pages_contribution_manifest() -> ModuleContributionManifest {
    ModuleContributionManifest {
        module_id: PAGES_MODULE_ID.to_string(),
        owner_provider: PAGES_OWNER_PROVIDER.to_string(),
        target_providers: BTreeSet::from([FLY_BUILTIN_PROVIDER.to_string()]),
        dependencies: BTreeSet::new(),
        required_permissions: BTreeSet::new(),
        admin: vec![pages_landing_blocks_contribution()],
        storefront: Vec::new(),
    }
}

pub fn pages_landing_blocks_contribution() -> ContributionDescriptor {
    ContributionDescriptor {
        id: PAGES_LANDING_BLOCKS_CONTRIBUTION_ID.to_string(),
        provider: FLY_BUILTIN_PROVIDER.to_string(),
        required_capabilities: capability_set(PAGES_LANDING_BLOCK_CAPABILITIES),
        blocks: PAGES_LANDING_BLOCK_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        renderers: Vec::new(),
        property_editors: Vec::new(),
        messages: BTreeMap::from([(
            "pages.builder.contributions.landingBlocks".to_string(),
            "Pages landing blocks".to_string(),
        )]),
        metadata: Map::from_iter([
            (
                "ownerProvider".to_string(),
                Value::String(PAGES_OWNER_PROVIDER.to_string()),
            ),
            ("format".to_string(), Value::String("grapesjs".to_string())),
            ("surface".to_string(), Value::String("admin".to_string())),
        ]),
    }
}

pub fn pages_admin_contribution_policy() -> ContributionAssemblyPolicy {
    ContributionAssemblyPolicy {
        enabled_modules: BTreeSet::from([PAGES_MODULE_ID.to_string()]),
        enabled_providers: BTreeSet::from([
            PAGES_OWNER_PROVIDER.to_string(),
            FLY_BUILTIN_PROVIDER.to_string(),
        ]),
        capabilities: capability_set(PAGES_BUILDER_CAPABILITIES),
        ..ContributionAssemblyPolicy::default()
    }
}

pub fn build_pages_admin_contribution_registry(
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    build_admin_contribution_registry_from_manifests([pages_contribution_manifest()], policy)
}

fn capability_set(capabilities: &[&str]) -> BTreeSet<String> {
    capabilities
        .iter()
        .map(|capability| (*capability).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::RegistrySet;

    #[test]
    fn manifest_explicitly_targets_the_fly_builtin_provider() {
        let manifest = pages_contribution_manifest();
        assert!(manifest.allows_target_provider(FLY_BUILTIN_PROVIDER));
        assert!(!manifest.allows_target_provider("other.provider"));
    }

    #[test]
    fn contributed_block_ids_exist_in_the_fly_registry() {
        let registries = RegistrySet::with_builtins();
        for block_id in PAGES_LANDING_BLOCK_IDS {
            assert!(
                registries.blocks.contains(block_id),
                "missing Fly block `{block_id}`"
            );
        }
    }

    #[test]
    fn admin_registry_contains_only_real_block_contracts() {
        let result = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
        assert!(result.is_valid());
        assert_eq!(result.registered_contributions, 1);
        let contribution = result
            .registry
            .get(PAGES_LANDING_BLOCKS_CONTRIBUTION_ID)
            .expect("Pages blocks contribution");
        assert_eq!(contribution.blocks.len(), PAGES_LANDING_BLOCK_IDS.len());
        assert!(contribution.renderers.is_empty());
        assert!(contribution.property_editors.is_empty());
        assert!(
            contribution
                .required_capabilities
                .is_subset(&pages_admin_contribution_policy().capabilities)
        );
    }

    #[test]
    fn contribution_policy_enables_owner_and_target_providers() {
        let policy = pages_admin_contribution_policy();
        assert!(policy.enabled_providers.contains(PAGES_OWNER_PROVIDER));
        assert!(policy.enabled_providers.contains(FLY_BUILTIN_PROVIDER));
    }

    #[test]
    fn capability_constants_match_the_module_manifest() {
        let module_manifest = include_str!("../../rustok-module.toml");
        for capability in PAGES_BUILDER_CAPABILITIES {
            assert!(
                module_manifest.contains(&format!("\"{capability}\"")),
                "Pages module manifest is missing builder capability `{capability}`"
            );
        }
    }
}
