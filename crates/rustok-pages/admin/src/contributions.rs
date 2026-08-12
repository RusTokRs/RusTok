use fly_ui::{
    ContributionAssemblyPolicy, ContributionAssemblyResult, ContributionDescriptor,
    ModuleContributionManifest, build_admin_contribution_registry_from_manifests,
};
use rustok_page_builder_admin::ConsumerPropertyEditorSchema;
use std::collections::BTreeSet;
use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/pages_contribution_manifest.rs"));

static GENERATED_PAGES_CONTRIBUTION_MANIFEST: LazyLock<ModuleContributionManifest> =
    LazyLock::new(|| {
        serde_json::from_str(GENERATED_PAGES_CONTRIBUTION_MANIFEST_JSON)
            .expect("build-generated Pages contribution manifest must deserialize")
    });

/// Build-generated module contribution metadata sourced only from `../rustok-module.toml`.
///
/// The build script injects owner/provider versions from canonical module metadata. Pages runtime
/// never parses TOML and this module does not retain a handwritten contribution descriptor tree.
pub fn pages_contribution_manifest() -> ModuleContributionManifest {
    GENERATED_PAGES_CONTRIBUTION_MANIFEST.clone()
}

pub fn pages_landing_blocks_contribution() -> ContributionDescriptor {
    generated_admin_contribution(PAGES_LANDING_BLOCKS_CONTRIBUTION_ID)
}

pub fn pages_metadata_contribution() -> ContributionDescriptor {
    generated_admin_contribution(PAGES_METADATA_CONTRIBUTION_ID)
}

pub fn pages_metadata_property_schema() -> ConsumerPropertyEditorSchema {
    let contribution = pages_metadata_contribution();
    let editor = contribution
        .property_editors
        .iter()
        .find(|editor| editor.id == PAGES_METADATA_PROPERTY_EDITOR_ID)
        .unwrap_or_else(|| {
            panic!(
                "generated Pages metadata contribution is missing property editor `{PAGES_METADATA_PROPERTY_EDITOR_ID}`"
            )
        });
    let schema =
        serde_json::from_value::<ConsumerPropertyEditorSchema>(editor.property_schema.clone())
            .expect("generated Pages metadata property schema must deserialize");
    schema
        .validate()
        .expect("generated Pages metadata property schema must satisfy Page Builder contract");
    schema
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

fn generated_admin_contribution(id: &str) -> ContributionDescriptor {
    GENERATED_PAGES_CONTRIBUTION_MANIFEST
        .admin
        .iter()
        .find(|contribution| contribution.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("generated Pages admin contribution `{id}` is missing"))
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
    fn manifest_targets_fly_blocks_and_keeps_metadata_under_pages_owner() {
        let manifest = pages_contribution_manifest();
        assert!(
            manifest.allows_target_provider(FLY_BUILTIN_PROVIDER, FLY_BUILTIN_PROVIDER_VERSION,)
        );
        assert!(
            manifest.allows_target_provider(PAGES_OWNER_PROVIDER, PAGES_OWNER_PROVIDER_VERSION,)
        );
        assert!(!manifest.allows_target_provider("other.provider", "1"));
        assert!(!manifest.allows_target_provider(FLY_BUILTIN_PROVIDER, "2"));
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
    fn admin_registry_contains_blocks_and_executable_metadata_properties() {
        let result = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
        assert!(result.is_valid());
        assert_eq!(result.registered_contributions, 2);

        let blocks = result
            .registry
            .get(PAGES_LANDING_BLOCKS_CONTRIBUTION_ID)
            .expect("Pages blocks contribution");
        assert_eq!(blocks.blocks.len(), PAGES_LANDING_BLOCK_IDS.len());
        assert!(blocks.renderers.is_empty());
        assert!(blocks.property_editors.is_empty());

        let metadata = result
            .registry
            .get(PAGES_METADATA_CONTRIBUTION_ID)
            .expect("Pages metadata contribution");
        assert!(metadata.blocks.is_empty());
        assert!(metadata.renderers.is_empty());
        assert_eq!(metadata.property_editors.len(), 1);
        let registered_schema = serde_json::from_value::<ConsumerPropertyEditorSchema>(
            metadata.property_editors[0].property_schema.clone(),
        )
        .expect("registered metadata schema");
        registered_schema.validate().expect("valid metadata schema");
        assert_eq!(registered_schema, pages_metadata_property_schema());
    }

    #[test]
    fn contribution_policy_enables_owner_and_target_providers() {
        let policy = pages_admin_contribution_policy();
        assert!(policy.enabled_providers.contains(PAGES_OWNER_PROVIDER));
        assert!(policy.enabled_providers.contains(FLY_BUILTIN_PROVIDER));
    }

    #[test]
    fn generated_constants_match_canonical_module_metadata() {
        use rustok_page_builder_admin::PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT;
        let module_manifest = include_str!("../../rustok-module.toml");
        assert!(module_manifest.contains("[fba.builder_consumer.contribution_manifest]"));
        assert!(module_manifest.contains("role = \"landing_blocks\""));
        assert!(module_manifest.contains("role = \"metadata\""));
        for capability in PAGES_BUILDER_CAPABILITIES {
            assert!(module_manifest.contains(&format!("\"{capability}\"")));
        }
        for block_id in PAGES_LANDING_BLOCK_IDS {
            assert!(module_manifest.contains(&format!("\"{block_id}\"")));
        }
        assert_eq!(
            pages_metadata_property_schema().format,
            PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT
        );
    }

    #[test]
    fn storefront_surface_stays_empty_until_a_real_adapter_exists() {
        assert!(pages_contribution_manifest().storefront.is_empty());
    }
}
