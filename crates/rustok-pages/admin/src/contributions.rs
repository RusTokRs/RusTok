use fly_ui::{
    AccessibilityMetadata, ContributionAssemblyPolicy, ContributionAssemblyResult,
    ContributionDescriptor, ModuleContributionManifest, PropertyEditorDescriptor,
    build_admin_contribution_registry_from_manifests,
};
use rustok_page_builder_admin::{
    ConsumerPropertyEditorSchema, ConsumerPropertyFieldDescriptor, ConsumerPropertyFieldKind,
    PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const PAGES_MODULE_ID: &str = "pages";
pub const PAGES_OWNER_PROVIDER: &str = "rustok.pages";
pub const FLY_BUILTIN_PROVIDER: &str = "fly.builtin";
pub const PAGES_LANDING_BLOCKS_CONTRIBUTION_ID: &str = "rustok.pages.landing-blocks";
pub const PAGES_METADATA_CONTRIBUTION_ID: &str = "rustok.pages.metadata";
pub const PAGES_METADATA_PROPERTY_EDITOR_ID: &str = "rustok.pages.metadata.editor";
pub const PAGES_METADATA_COMPONENT_TYPE: &str = "rustok-pages-metadata";

pub const PAGES_BUILDER_CAPABILITIES: &[&str] = &["preview", "tree", "properties", "publish"];
pub const PAGES_LANDING_BLOCK_CAPABILITIES: &[&str] = &["tree", "properties"];
pub const PAGES_METADATA_CAPABILITIES: &[&str] = &["properties"];

pub const PAGES_LANDING_BLOCK_IDS: &[&str] = &[
    "fly.hero",
    "fly.two_columns",
    "fly.feature_grid",
    "fly.cta",
    "fly.contact_form",
];

/// Module-owned metadata used by the generated Fly admin contribution registry.
///
/// Pages owns document lifecycle and metadata persistence. Landing blocks explicitly target
/// `fly.builtin`, while the executable metadata editor remains under the `rustok.pages` owner
/// provider and calls the consumer facade rather than mutating the Fly document.
pub fn pages_contribution_manifest() -> ModuleContributionManifest {
    ModuleContributionManifest {
        module_id: PAGES_MODULE_ID.to_string(),
        owner_provider: PAGES_OWNER_PROVIDER.to_string(),
        target_providers: BTreeSet::from([FLY_BUILTIN_PROVIDER.to_string()]),
        dependencies: BTreeSet::new(),
        required_permissions: BTreeSet::new(),
        admin: vec![
            pages_landing_blocks_contribution(),
            pages_metadata_contribution(),
        ],
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

pub fn pages_metadata_property_schema() -> ConsumerPropertyEditorSchema {
    ConsumerPropertyEditorSchema {
        format: PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT.to_string(),
        id: "rustok.pages.metadata.schema".to_string(),
        title: "Page metadata".to_string(),
        description: Some(
            "Versioned Pages metadata. Saving these properties never writes the Fly document."
                .to_string(),
        ),
        fields: vec![
            property_field(
                "title",
                "Title",
                ConsumerPropertyFieldKind::Text,
                true,
                512,
                None,
                None,
            ),
            property_field(
                "slug",
                "Slug",
                ConsumerPropertyFieldKind::Text,
                true,
                512,
                None,
                None,
            ),
            property_field(
                "meta_title",
                "SEO title",
                ConsumerPropertyFieldKind::Text,
                false,
                512,
                None,
                None,
            ),
            property_field(
                "meta_description",
                "SEO description",
                ConsumerPropertyFieldKind::TextArea,
                false,
                4_096,
                None,
                None,
            ),
            property_field(
                "template",
                "Template",
                ConsumerPropertyFieldKind::Text,
                false,
                256,
                None,
                None,
            ),
            property_field(
                "channel_slugs",
                "Channels",
                ConsumerPropertyFieldKind::StringList,
                false,
                4_096,
                Some("Comma-separated channel slugs. Leave empty for every channel."),
                Some("web, mobile"),
            ),
        ],
    }
}

pub fn pages_metadata_contribution() -> ContributionDescriptor {
    let schema = pages_metadata_property_schema();
    ContributionDescriptor {
        id: PAGES_METADATA_CONTRIBUTION_ID.to_string(),
        provider: PAGES_OWNER_PROVIDER.to_string(),
        required_capabilities: capability_set(PAGES_METADATA_CAPABILITIES),
        blocks: Vec::new(),
        renderers: Vec::new(),
        property_editors: vec![PropertyEditorDescriptor {
            id: PAGES_METADATA_PROPERTY_EDITOR_ID.to_string(),
            component_type: PAGES_METADATA_COMPONENT_TYPE.to_string(),
            provider: PAGES_OWNER_PROVIDER.to_string(),
            property_schema: serde_json::to_value(schema)
                .expect("Pages metadata property schema must be serializable"),
            accessibility: AccessibilityMetadata {
                label_message_id: "pages.builder.contributions.metadata.label".to_string(),
                description_message_id: Some(
                    "pages.builder.contributions.metadata.description".to_string(),
                ),
                keyboard_hint_message_id: None,
            },
        }],
        messages: BTreeMap::from([
            (
                "pages.builder.contributions.metadata.label".to_string(),
                "Page metadata".to_string(),
            ),
            (
                "pages.builder.contributions.metadata.description".to_string(),
                "Edit versioned Pages metadata without modifying the Fly document.".to_string(),
            ),
        ]),
        metadata: Map::from_iter([
            (
                "ownerProvider".to_string(),
                Value::String(PAGES_OWNER_PROVIDER.to_string()),
            ),
            (
                "persistence".to_string(),
                Value::String("consumer_facade".to_string()),
            ),
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

fn property_field(
    id: &str,
    label: &str,
    kind: ConsumerPropertyFieldKind,
    required: bool,
    max_bytes: usize,
    help: Option<&str>,
    placeholder: Option<&str>,
) -> ConsumerPropertyFieldDescriptor {
    ConsumerPropertyFieldDescriptor {
        id: id.to_string(),
        label: label.to_string(),
        help: help.map(ToString::to_string),
        kind,
        required,
        max_bytes,
        placeholder: placeholder.map(ToString::to_string),
    }
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
        assert!(manifest.allows_target_provider(FLY_BUILTIN_PROVIDER));
        assert!(manifest.allows_target_provider(PAGES_OWNER_PROVIDER));
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
