use fly::{
    BlockDefinition, ComponentDefinition, ComponentNode, ComponentObject, FlyError, FlyResult,
    RegistrySet,
};
use fly_ui::{
    ContributionAdapter, ContributionAssemblyPolicy, ContributionAssemblyResult,
    ContributionDescriptor, ModuleContributionManifest, Presentation, PropertyEditorRequest,
    RendererRequest, ResolvedPropertyEditor, ResolvedRenderer, UiError, UiResult,
    build_admin_contribution_registry_from_manifests,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/forum_contribution_manifest.rs"));

const OWNER_SCHEMA_REF_FORMAT: &str = "forum_widget_owner_schema_ref_v1";
const COMPONENT_PROPS_FIELD: &str = "props";
const FORUM_BLOCK_CATEGORY: &str = "forum";

static GENERATED_FORUM_CONTRIBUTION_MANIFEST: LazyLock<ModuleContributionManifest> =
    LazyLock::new(|| {
        serde_json::from_str(GENERATED_FORUM_CONTRIBUTION_MANIFEST_JSON)
            .expect("build-generated Forum contribution manifest must deserialize")
    });

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumWidgetOwnerSchemaRef {
    pub format: String,
    pub schema_id: String,
    pub catalog_endpoint: String,
    pub validate_endpoint: String,
    pub owner_data_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetRenderModel {
    pub component_id: String,
    pub widget_type: String,
    pub presentation: Presentation,
    pub props: Value,
    pub owner_schema: ForumWidgetOwnerSchemaRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetPropertyEditorModel {
    pub component_id: String,
    pub widget_type: String,
    pub props: Value,
    pub owner_schema: ForumWidgetOwnerSchemaRef,
}

/// Forum-owned adapter for Fly contribution contracts.
///
/// It resolves canonical widget identity and opaque configuration only. It never executes Forum
/// reads or copies Forum JSON schemas into the Fly layer. Owner catalog/validation, property
/// transport and preview transport remain the authoritative Forum boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForumContributionAdapter;

impl ContributionAdapter for ForumContributionAdapter {
    type Rendered = ForumWidgetRenderModel;
    type PropertyEditor = ForumWidgetPropertyEditorModel;

    fn render(
        &self,
        resolved: ResolvedRenderer<'_>,
        request: &RendererRequest<'_>,
    ) -> UiResult<Self::Rendered> {
        ensure_forum_provider(request.provider)?;
        ensure_component_type(
            request.component_type,
            resolved.renderer.component_type.as_str(),
        )?;
        Ok(ForumWidgetRenderModel {
            component_id: request.component_id.to_string(),
            widget_type: request.component_type.to_string(),
            presentation: request.presentation,
            props: component_props(request.component)?,
            owner_schema: owner_schema_for_component(request.component_type)?,
        })
    }

    fn property_editor(
        &self,
        resolved: ResolvedPropertyEditor<'_>,
        request: &PropertyEditorRequest<'_>,
    ) -> UiResult<Self::PropertyEditor> {
        ensure_forum_provider(request.provider)?;
        ensure_component_type(
            request.component_type,
            resolved.property_editor.component_type.as_str(),
        )?;
        Ok(ForumWidgetPropertyEditorModel {
            component_id: request.component_id.to_string(),
            widget_type: request.component_type.to_string(),
            props: component_props(request.component)?,
            owner_schema: owner_schema_ref(&resolved.property_editor.property_schema)?,
        })
    }
}

/// Build-generated module contribution metadata sourced only from `../rustok-module.toml`.
pub fn forum_contribution_manifest() -> ModuleContributionManifest {
    GENERATED_FORUM_CONTRIBUTION_MANIFEST.clone()
}

pub fn forum_widget_contribution() -> ContributionDescriptor {
    generated_contribution(FORUM_WIDGET_CONTRIBUTION_ID).clone()
}

pub fn forum_widget_preview_contribution() -> ContributionDescriptor {
    generated_contribution(FORUM_WIDGET_PREVIEW_CONTRIBUTION_ID).clone()
}

pub fn forum_full_admin_contribution_policy() -> ContributionAssemblyPolicy {
    ContributionAssemblyPolicy {
        enabled_modules: BTreeSet::from([FORUM_MODULE_ID.to_string()]),
        enabled_providers: BTreeSet::from([FORUM_OWNER_PROVIDER.to_string()]),
        capabilities: string_set(FORUM_BUILDER_CAPABILITIES),
        permissions: string_set(FORUM_REQUIRED_PERMISSIONS),
        ..ContributionAssemblyPolicy::default()
    }
}

pub fn build_forum_admin_contribution_registry(
    policy: &ContributionAssemblyPolicy,
) -> ContributionAssemblyResult {
    build_admin_contribution_registry_from_manifests([forum_contribution_manifest()], policy)
}

/// Register Forum widget component/block definitions into a Fly registry set.
///
/// Block ids intentionally equal component types (`forum.*`) so canonical module metadata remains
/// the only mapping authority. The Fly component persists only versioned widget configuration in
/// `props`; Forum owner data is never copied into the builder document.
pub fn register_forum_fly_widgets(registries: &mut RegistrySet) -> FlyResult<()> {
    let catalog = generated_contribution(FORUM_WIDGET_CONTRIBUTION_ID);
    let preview = generated_contribution(FORUM_WIDGET_PREVIEW_CONTRIBUTION_ID);
    for block_id in &catalog.blocks {
        let renderer = preview
            .renderers
            .iter()
            .find(|renderer| renderer.component_type == *block_id)
            .ok_or_else(|| {
                FlyError::Decode(format!(
                    "generated Forum block `{block_id}` has no preview renderer contract"
                ))
            })?;
        let label = catalog
            .messages
            .get(&renderer.accessibility.label_message_id)
            .or_else(|| {
                preview
                    .messages
                    .get(&renderer.accessibility.label_message_id)
            })
            .cloned()
            .ok_or_else(|| {
                FlyError::Decode(format!(
                    "generated Forum renderer `{}` is missing accessibility message `{}`",
                    renderer.id, renderer.accessibility.label_message_id
                ))
            })?;

        registries.components.register(ComponentDefinition {
            id: block_id.clone(),
            provider: FORUM_OWNER_PROVIDER.to_string(),
            allowed_children: Vec::new(),
            accepts_any_child: false,
            is_container: false,
        })?;

        let mut extensions = Map::new();
        extensions.insert(COMPONENT_PROPS_FIELD.to_string(), Value::Object(Map::new()));
        registries.blocks.register(BlockDefinition {
            id: block_id.clone(),
            label,
            category: FORUM_BLOCK_CATEGORY.to_string(),
            component: ComponentNode::Object(Box::new(ComponentObject {
                component_type: Some(block_id.clone()),
                provider: Some(FORUM_OWNER_PROVIDER.to_string()),
                extensions,
                ..ComponentObject::default()
            })),
        })?;
    }
    Ok(())
}

pub fn forum_fly_registry_set() -> FlyResult<RegistrySet> {
    let mut registries = RegistrySet::with_builtins();
    register_forum_fly_widgets(&mut registries)?;
    Ok(registries)
}

fn generated_contribution(id: &str) -> &'static ContributionDescriptor {
    GENERATED_FORUM_CONTRIBUTION_MANIFEST
        .admin
        .iter()
        .find(|contribution| contribution.id == id)
        .unwrap_or_else(|| panic!("generated Forum admin contribution `{id}` is missing"))
}

fn owner_schema_for_component(component_type: &str) -> UiResult<ForumWidgetOwnerSchemaRef> {
    let editor = generated_contribution(FORUM_WIDGET_CONTRIBUTION_ID)
        .property_editors
        .iter()
        .find(|editor| editor.component_type == component_type)
        .ok_or_else(|| {
            UiError::AdapterRejected(format!(
                "Forum widget `{component_type}` has no owner-backed property contract"
            ))
        })?;
    owner_schema_ref(&editor.property_schema)
}

fn ensure_forum_provider(provider: &str) -> UiResult<()> {
    if provider.trim() == FORUM_OWNER_PROVIDER {
        Ok(())
    } else {
        Err(UiError::AdapterRejected(format!(
            "Forum adapter received provider `{}` instead of `{FORUM_OWNER_PROVIDER}`",
            provider.trim()
        )))
    }
}

fn ensure_component_type(actual: &str, expected: &str) -> UiResult<()> {
    if actual.trim() == expected.trim() {
        Ok(())
    } else {
        Err(UiError::AdapterRejected(format!(
            "Forum adapter component type `{}` does not match resolved contract `{}`",
            actual.trim(),
            expected.trim()
        )))
    }
}

fn component_props(component: &Value) -> UiResult<Value> {
    let object = component.as_object().ok_or_else(|| {
        UiError::AdapterRejected("Forum widget component must be a JSON object".to_string())
    })?;
    let props = object
        .get(COMPONENT_PROPS_FIELD)
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    if props.is_object() {
        Ok(props)
    } else {
        Err(UiError::AdapterRejected(
            "Forum widget `props` must remain a JSON object".to_string(),
        ))
    }
}

fn owner_schema_ref(value: &Value) -> UiResult<ForumWidgetOwnerSchemaRef> {
    let schema =
        serde_json::from_value::<ForumWidgetOwnerSchemaRef>(value.clone()).map_err(|error| {
            UiError::AdapterRejected(format!(
                "Forum widget owner schema reference is invalid: {error}"
            ))
        })?;
    if schema.format != OWNER_SCHEMA_REF_FORMAT {
        return Err(UiError::AdapterRejected(format!(
            "Forum widget owner schema format `{}` is unsupported",
            schema.format
        )));
    }
    for (label, field) in [
        ("schema_id", schema.schema_id.as_str()),
        ("catalog_endpoint", schema.catalog_endpoint.as_str()),
        ("validate_endpoint", schema.validate_endpoint.as_str()),
        ("owner_data_state", schema.owner_data_state.as_str()),
    ] {
        if field.trim().is_empty() {
            return Err(UiError::AdapterRejected(format!(
                "Forum widget owner schema reference requires non-empty `{label}`"
            )));
        }
    }
    Ok(schema)
}

fn string_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::{
        PropertyEditorRequest, RendererRequest, edit_contribution_properties, render_contribution,
    };
    use serde_json::to_value;

    #[test]
    fn generated_manifest_registers_split_authoring_and_preview_contracts() {
        let result =
            build_forum_admin_contribution_registry(&forum_full_admin_contribution_policy());
        assert!(result.is_valid(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.registered_contributions, 2);

        let catalog = result
            .registry
            .get(FORUM_WIDGET_CONTRIBUTION_ID)
            .expect("Forum widget catalog contribution");
        assert_eq!(catalog.blocks.len(), FORUM_WIDGET_COMPONENT_TYPES.len());
        assert!(catalog.renderers.is_empty());
        assert_eq!(
            catalog.property_editors.len(),
            FORUM_WIDGET_COMPONENT_TYPES.len()
        );

        let preview = result
            .registry
            .get(FORUM_WIDGET_PREVIEW_CONTRIBUTION_ID)
            .expect("Forum widget preview contribution");
        assert!(preview.blocks.is_empty());
        assert_eq!(preview.renderers.len(), FORUM_WIDGET_COMPONENT_TYPES.len());
        assert!(preview.property_editors.is_empty());
    }

    #[test]
    fn preview_off_keeps_authoring_contracts_but_filters_renderers() {
        let mut policy = forum_full_admin_contribution_policy();
        policy.capabilities.remove("preview");
        let result = build_forum_admin_contribution_registry(&policy);
        assert!(result.is_valid(), "diagnostics: {:?}", result.diagnostics);
        assert!(result.registry.get(FORUM_WIDGET_CONTRIBUTION_ID).is_some());
        assert!(
            result
                .registry
                .get(FORUM_WIDGET_PREVIEW_CONTRIBUTION_ID)
                .is_none()
        );
    }

    #[test]
    fn fly_registry_uses_widget_type_as_block_and_component_identity() {
        let registries = forum_fly_registry_set().expect("Forum Fly registry");
        for component_type in FORUM_WIDGET_COMPONENT_TYPES {
            let component = registries
                .components
                .get(component_type)
                .expect("Forum component definition");
            assert_eq!(component.provider, FORUM_OWNER_PROVIDER);
            let block = registries
                .blocks
                .get(component_type)
                .expect("Forum block definition");
            assert_eq!(
                block
                    .component
                    .as_object()
                    .and_then(|value| value.provider.as_deref()),
                Some(FORUM_OWNER_PROVIDER)
            );
        }
    }

    #[test]
    fn adapter_resolves_preview_and_property_contract_without_copying_owner_schema() {
        let assembly =
            build_forum_admin_contribution_registry(&forum_full_admin_contribution_policy());
        let component_type = FORUM_WIDGET_COMPONENT_TYPES[0];
        let registries = forum_fly_registry_set().expect("Forum Fly registry");
        let component = to_value(
            &registries
                .blocks
                .get(component_type)
                .expect("Forum block")
                .component,
        )
        .expect("serialize Forum block component");
        let capabilities = string_set(FORUM_BUILDER_CAPABILITIES);

        let rendered = render_contribution(
            &assembly.registry,
            &ForumContributionAdapter,
            &RendererRequest {
                component_id: "forum-widget-1",
                provider: FORUM_OWNER_PROVIDER,
                component_type,
                presentation: Presentation::Preview,
                component: &component,
            },
            &capabilities,
        )
        .expect("Forum preview contract");
        assert_eq!(rendered.widget_type, component_type);
        assert_eq!(rendered.owner_schema.format, OWNER_SCHEMA_REF_FORMAT);
        assert_eq!(
            rendered.owner_schema.owner_data_state,
            "owner_property_editor_ready"
        );

        let editor = edit_contribution_properties(
            &assembly.registry,
            &ForumContributionAdapter,
            &PropertyEditorRequest {
                component_id: "forum-widget-1",
                provider: FORUM_OWNER_PROVIDER,
                component_type,
                presentation: Presentation::Full,
                component: &component,
            },
            &capabilities,
        )
        .expect("Forum property editor contract");
        assert_eq!(editor.widget_type, component_type);
        assert!(editor.owner_schema.schema_id.starts_with("forum."));
    }

    #[test]
    fn missing_forum_permission_filters_both_contributions_before_registration() {
        let mut policy = forum_full_admin_contribution_policy();
        policy.permissions.clear();
        let result = build_forum_admin_contribution_registry(&policy);
        assert!(result.is_valid());
        assert_eq!(result.registered_contributions, 0);
        assert_eq!(result.skipped_contributions, 2);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "contribution_permission_missing")
        );
    }
}
