use crate::{
    materialize_runtime, render_page, FlyResult, PageSelection, ProjectDocument, RenderPolicy,
    RenderedPage, RuntimeMaterialization, ValidationDiagnostic,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeRenderResult {
    pub page: RenderedPage,
    pub diagnostics: Vec<ValidationDiagnostic>,
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
    let RuntimeMaterialization {
        document,
        diagnostics,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    } = materialize_runtime(document, context);
    let page = render_page(&document, selection, policy)?;
    Ok(RuntimeRenderResult {
        page,
        diagnostics,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    #[test]
    fn runtime_renderer_expands_repeaters_before_sanitized_rendering() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "row",
                        "type": "text",
                        "content": "{{item.name}}"
                    }]
                }
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
            &json!({ "items": [{ "name": "One" }, { "name": "Two" }] }),
        )
        .expect("runtime render");
        assert_eq!(result.repeated_nodes, 2);
        assert!(result.page.html.contains("One"));
        assert!(result.page.html.contains("Two"));
        assert!(!result.page.html.contains("{{item.name}}"));
    }
}
