use crate::{
    FlyResult, PageSelection, ProjectDocument, RenderPolicy, RenderedPage,
    RuntimeProjectMaterialization, ValidationDiagnostic, materialize_project_with_runtime_context,
    render_page,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeRenderResult {
    pub page: RenderedPage,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub context_type_mismatches: usize,
    pub resolved_internal_links: usize,
    pub fallback_internal_links: usize,
    pub unresolved_internal_links: usize,
    #[serde(default)]
    pub materialized_forms: usize,
    #[serde(default)]
    pub native_actions: usize,
    #[serde(default)]
    pub custom_actions: usize,
    #[serde(default)]
    pub fallback_actions: usize,
    #[serde(default)]
    pub unresolved_actions: usize,
    pub applied_bindings: usize,
    pub fallback_bindings: usize,
    pub unresolved_bindings: usize,
    pub evaluated_conditions: usize,
    pub hidden_components: usize,
    pub repeated_nodes: usize,
}

impl RuntimeRenderResult {
    pub fn document_html(&self) -> String {
        self.page.document_html()
    }
}

pub fn render_page_with_runtime_context(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
    context: &Value,
) -> FlyResult<RuntimeRenderResult> {
    let RuntimeProjectMaterialization {
        document,
        effective_context: _,
        diagnostics,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        context_type_mismatches,
        resolved_internal_links,
        fallback_internal_links,
        unresolved_internal_links,
        materialized_forms,
        native_actions,
        custom_actions,
        fallback_actions,
        unresolved_actions,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    } = materialize_project_with_runtime_context(document, context);
    let page = render_page(&document, selection, policy)?;
    Ok(RuntimeRenderResult {
        page,
        diagnostics,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        context_type_mismatches,
        resolved_internal_links,
        fallback_internal_links,
        unresolved_internal_links,
        materialized_forms,
        native_actions,
        custom_actions,
        fallback_actions,
        unresolved_actions,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    #[test]
    fn runtime_renderer_applies_context_bindings_and_repeaters_in_order() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "content": "Static"
                    }, {
                        "id": "row",
                        "type": "text",
                        "content": "{{item.name}}"
                    }]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "prefix",
                "path": "page.prefix",
                "kind": "string",
                "default": "Featured"
            }],
            "flyRuntimeComputed": [{
                "id": "title",
                "path": "page.title",
                "expression": {
                    "op": "format",
                    "template": "{{page.prefix}} products"
                }
            }],
            "flyRuntimeBindings": [{
                "id": "heading-content",
                "component_id": "heading",
                "path": "page.title",
                "target": "field",
                "name": "content",
                "transform": "uppercase"
            }],
            "flyRuntimeRepeaters": [{
                "id": "rows",
                "component_id": "row",
                "path": "items"
            }]
        }))
        .expect("document");
        let result = render_page_with_runtime_context(
            &document,
            &PageSelection::Id("home".to_string()),
            &RenderPolicy::default(),
            &json!({
                "items": [{ "name": "One" }, { "name": "Two" }]
            }),
        )
        .expect("runtime render");
        assert_eq!(result.defaults_applied, 1);
        assert_eq!(result.computed_applied, 1);
        assert_eq!(result.resolved_internal_links, 0);
        assert_eq!(result.materialized_forms, 0);
        assert_eq!(result.native_actions, 0);
        assert_eq!(result.applied_bindings, 1);
        assert_eq!(result.repeated_nodes, 2);
        assert!(result.page.html.contains("FEATURED PRODUCTS"));
        assert!(result.page.html.contains("One"));
        assert!(result.page.html.contains("Two"));
        assert!(!result.page.html.contains("{{item.name}}"));
    }

    #[test]
    fn runtime_renderer_emits_native_forms_and_locale_aware_actions() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "default_locale": "ru",
                "supported_locales": ["ru", "en"]
            },
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": { "$localized": { "en": "home", "ru": "glavnaya" } } },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "contact-form",
                        "type": "wrapper",
                        "flyForm": {
                            "id": "contact",
                            "method": "post",
                            "action_url": "/contact"
                        },
                        "components": [{
                            "id": "submit",
                            "type": "button",
                            "content": "Send",
                            "flyAction": { "kind": "submit_form", "form_id": "contact" }
                        }]
                    }, {
                        "id": "about",
                        "type": "button",
                        "content": "About",
                        "flyAction": { "kind": "navigate_page", "page_id": "about-page" }
                    }]
                }
            }, {
                "id": "about-page",
                "flyPageMeta": { "slug": { "$localized": { "en": "about", "ru": "o-nas" } } },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }))
        .expect("document");

        let result = render_page_with_runtime_context(
            &document,
            &PageSelection::Id("home".to_string()),
            &RenderPolicy::default(),
            &json!({ "$locale": "ru" }),
        )
        .expect("runtime render");
        assert_eq!(result.materialized_forms, 1);
        assert_eq!(result.native_actions, 2);
        assert!(result.page.html.contains("<form"));
        assert!(result.page.html.contains("action=\"/contact\""));
        assert!(result.page.html.contains("form=\"contact\""));
        assert!(result.page.html.contains("href=\"/o-nas\""));
    }
}
