use fly::{
    FlyResult, GrapesJsCodec, RuntimeContextDependencyGraph, analyze_runtime_context_dependencies,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeDependencyRequest {
    pub project_data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeDependencyResponse {
    pub graph: RuntimeContextDependencyGraph,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeDependencyInspector;

impl PageBuilderRuntimeDependencyInspector {
    pub fn analyze(
        &self,
        request: PageBuilderRuntimeDependencyRequest,
    ) -> FlyResult<PageBuilderRuntimeDependencyResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeDependencyResponse {
            graph: analyze_runtime_context_dependencies(&document),
        })
    }
}

pub fn analyze_page_builder_runtime_dependencies(
    request: PageBuilderRuntimeDependencyRequest,
) -> FlyResult<PageBuilderRuntimeDependencyResponse> {
    PageBuilderRuntimeDependencyInspector.analyze(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn consumer_can_inspect_runtime_dependency_graph() {
        let response =
            analyze_page_builder_runtime_dependencies(PageBuilderRuntimeDependencyRequest {
                project_data: json!({
                    "pages": [{
                        "component": {
                            "id": "root",
                            "type": "wrapper",
                            "components": [{ "id": "title", "type": "text" }]
                        }
                    }],
                    "flyRuntimeContextSchema": [{
                        "id": "first",
                        "path": "user.first",
                        "kind": "string"
                    }],
                    "flyRuntimeComputed": [{
                        "id": "label",
                        "path": "user.label",
                        "expression": {
                            "op": "format",
                            "template": "Hello {{user.first}}"
                        }
                    }],
                    "flyRuntimeBindings": [{
                        "id": "title",
                        "component_id": "title",
                        "path": "user.label",
                        "target": "field",
                        "name": "content"
                    }]
                }),
            })
            .expect("dependency response");
        assert_eq!(response.graph.declared_field_count, 1);
        assert_eq!(response.graph.computed_count, 1);
        assert_eq!(response.graph.computed_evaluation_order, vec!["user.label"]);
        assert_eq!(
            response
                .graph
                .node("user.label")
                .map(|node| node.consumers.len()),
            Some(1)
        );
    }
}
