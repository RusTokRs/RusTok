use crate::{
    ContributionRegistry, Presentation, ResolvedPropertyEditor, ResolvedRenderer, UiError, UiResult,
};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy)]
pub struct RendererRequest<'a> {
    pub component_id: &'a str,
    pub provider: &'a str,
    pub component_type: &'a str,
    pub presentation: Presentation,
    pub component: &'a Value,
}

#[derive(Debug, Clone, Copy)]
pub struct PropertyEditorRequest<'a> {
    pub component_id: &'a str,
    pub provider: &'a str,
    pub component_type: &'a str,
    pub presentation: Presentation,
    pub component: &'a Value,
}

/// Framework adapters implement this contract without owning contribution discovery or policy.
///
/// `fly-ui` resolves the descriptor first. A Leptos, Dioxus, server-rendered, or test adapter only
/// receives a validated renderer/property-editor pair and the canonical component value.
pub trait ContributionAdapter {
    type Rendered;
    type PropertyEditor;

    fn render(
        &self,
        resolved: ResolvedRenderer<'_>,
        request: &RendererRequest<'_>,
    ) -> UiResult<Self::Rendered>;

    fn property_editor(
        &self,
        resolved: ResolvedPropertyEditor<'_>,
        request: &PropertyEditorRequest<'_>,
    ) -> UiResult<Self::PropertyEditor>;
}

pub fn render_contribution<A: ContributionAdapter>(
    registry: &ContributionRegistry,
    adapter: &A,
    request: &RendererRequest<'_>,
    capabilities: &BTreeSet<String>,
) -> UiResult<A::Rendered> {
    let resolved = registry
        .resolve_renderer(
            request.provider,
            request.component_type,
            request.presentation,
            capabilities,
        )
        .ok_or_else(|| {
            UiError::RendererUnavailable(renderer_lookup_id(
                request.provider,
                request.component_type,
                request.presentation,
            ))
        })?;
    adapter.render(resolved, request)
}

pub fn edit_contribution_properties<A: ContributionAdapter>(
    registry: &ContributionRegistry,
    adapter: &A,
    request: &PropertyEditorRequest<'_>,
    capabilities: &BTreeSet<String>,
) -> UiResult<A::PropertyEditor> {
    if !request.presentation.is_editable() {
        return Err(UiError::ReadOnly);
    }
    let resolved = registry
        .resolve_property_editor(request.provider, request.component_type, capabilities)
        .ok_or_else(|| {
            UiError::PropertyEditorUnavailable(property_editor_lookup_id(
                request.provider,
                request.component_type,
            ))
        })?;
    adapter.property_editor(resolved, request)
}

fn renderer_lookup_id(provider: &str, component_type: &str, presentation: Presentation) -> String {
    format!(
        "{}:{}:{}",
        provider.trim(),
        component_type.trim(),
        presentation.as_str()
    )
}

fn property_editor_lookup_id(provider: &str, component_type: &str) -> String {
    format!("{}:{}", provider.trim(), component_type.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccessibilityMetadata, ContributionDescriptor, PropertyEditorDescriptor, RendererDescriptor,
    };
    use serde_json::{Map, json};
    use std::collections::{BTreeMap, BTreeSet};

    struct MockAdapter;

    impl ContributionAdapter for MockAdapter {
        type Rendered = String;
        type PropertyEditor = String;

        fn render(
            &self,
            resolved: ResolvedRenderer<'_>,
            request: &RendererRequest<'_>,
        ) -> UiResult<Self::Rendered> {
            Ok(format!(
                "{}:{}:{}",
                resolved.renderer.id,
                request.component_id,
                request.presentation.as_str()
            ))
        }

        fn property_editor(
            &self,
            resolved: ResolvedPropertyEditor<'_>,
            request: &PropertyEditorRequest<'_>,
        ) -> UiResult<Self::PropertyEditor> {
            Ok(format!(
                "{}:{}",
                resolved.property_editor.id, request.component_id
            ))
        }
    }

    fn registry() -> ContributionRegistry {
        let accessibility = AccessibilityMetadata {
            label_message_id: "mock.label".to_string(),
            description_message_id: None,
            keyboard_hint_message_id: None,
        };
        let mut registry = ContributionRegistry::default();
        registry
            .register(ContributionDescriptor {
                id: "mock.contribution".to_string(),
                provider: "mock.provider".to_string(),
                required_capabilities: BTreeSet::from(["mock.render".to_string()]),
                blocks: Vec::new(),
                renderers: vec![RendererDescriptor {
                    id: "mock.renderer".to_string(),
                    component_type: "mock-card".to_string(),
                    provider: "mock.provider".to_string(),
                    presentations: BTreeSet::from([
                        Presentation::Full,
                        Presentation::Inline,
                        Presentation::Preview,
                        Presentation::ReadOnly,
                    ]),
                    accessibility: accessibility.clone(),
                }],
                property_editors: vec![PropertyEditorDescriptor {
                    id: "mock.properties".to_string(),
                    component_type: "mock-card".to_string(),
                    provider: "mock.provider".to_string(),
                    property_schema: json!({ "type": "object" }),
                    accessibility,
                }],
                messages: BTreeMap::from([("mock.label".to_string(), "Mock card".to_string())]),
                metadata: Map::new(),
            })
            .expect("registry");
        registry
    }

    #[test]
    fn one_mock_adapter_renders_all_presentations() {
        let registry = registry();
        let capabilities = BTreeSet::from(["mock.render".to_string()]);
        let component = json!({ "id": "card" });
        for presentation in [
            Presentation::Full,
            Presentation::Inline,
            Presentation::Preview,
            Presentation::ReadOnly,
        ] {
            let rendered = render_contribution(
                &registry,
                &MockAdapter,
                &RendererRequest {
                    component_id: "card",
                    provider: "mock.provider",
                    component_type: "mock-card",
                    presentation,
                    component: &component,
                },
                &capabilities,
            )
            .expect("render");
            assert!(rendered.ends_with(presentation.as_str()));
        }
    }

    #[test]
    fn property_editor_is_available_only_in_editable_presentations() {
        let registry = registry();
        let capabilities = BTreeSet::from(["mock.render".to_string()]);
        let component = json!({ "id": "card" });
        let full = PropertyEditorRequest {
            component_id: "card",
            provider: "mock.provider",
            component_type: "mock-card",
            presentation: Presentation::Full,
            component: &component,
        };
        let preview = PropertyEditorRequest {
            presentation: Presentation::Preview,
            ..full
        };
        assert_eq!(
            edit_contribution_properties(&registry, &MockAdapter, &full, &capabilities)
                .expect("editor"),
            "mock.properties:card"
        );
        assert_eq!(
            edit_contribution_properties(&registry, &MockAdapter, &preview, &capabilities),
            Err(UiError::ReadOnly)
        );
    }

    #[test]
    fn missing_capability_returns_typed_lookup_error() {
        let component = json!({ "id": "card" });
        let error = render_contribution(
            &registry(),
            &MockAdapter,
            &RendererRequest {
                component_id: "card",
                provider: "mock.provider",
                component_type: "mock-card",
                presentation: Presentation::Full,
                component: &component,
            },
            &BTreeSet::new(),
        )
        .expect_err("missing renderer");
        assert!(matches!(error, UiError::RendererUnavailable(_)));
    }
}
