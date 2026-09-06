use fly::{
    FlyResult, GrapesJsCodec, PageSelection, RenderPolicy, RuntimeContextScenario,
    RuntimeScenarioRenderMatrix, render_runtime_scenario_matrix,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioRenderRequest {
    pub project_data: Value,
    pub selection: PageSelection,
    #[serde(default)]
    pub policy: RenderPolicy,
    #[serde(default)]
    pub scenarios: Vec<RuntimeContextScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioRenderResponse {
    pub matrix: RuntimeScenarioRenderMatrix,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeScenarioRenderer;

impl PageBuilderRuntimeScenarioRenderer {
    pub fn render(
        &self,
        request: PageBuilderRuntimeScenarioRenderRequest,
    ) -> FlyResult<PageBuilderRuntimeScenarioRenderResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeScenarioRenderResponse {
            matrix: render_runtime_scenario_matrix(
                &document,
                &request.selection,
                &request.policy,
                &request.scenarios,
            ),
        })
    }
}

pub fn render_page_builder_runtime_scenarios(
    request: PageBuilderRuntimeScenarioRenderRequest,
) -> FlyResult<PageBuilderRuntimeScenarioRenderResponse> {
    PageBuilderRuntimeScenarioRenderer.render(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn consumer_can_render_runtime_scenario_matrix() {
        let response =
            render_page_builder_runtime_scenarios(PageBuilderRuntimeScenarioRenderRequest {
                project_data: json!({
                    "pages": [{
                        "id": "home",
                        "component": {
                            "id": "root",
                            "type": "wrapper",
                            "components": [{ "id": "title", "type": "text" }]
                        }
                    }],
                    "flyRuntimeBindings": [{
                        "id": "title-content",
                        "component_id": "title",
                        "path": "page.title",
                        "target": "field",
                        "name": "content"
                    }]
                }),
                selection: PageSelection::Id("home".to_string()),
                policy: RenderPolicy::default(),
                scenarios: vec![
                    RuntimeContextScenario::new(
                        "one",
                        "One",
                        json!({ "page": { "title": "First" } }),
                    ),
                    RuntimeContextScenario::new(
                        "two",
                        "Two",
                        json!({ "page": { "title": "Second" } }),
                    ),
                ],
            })
            .expect("scenario render response");
        assert_eq!(response.matrix.rendered_count, 2);
        assert_eq!(response.matrix.unique_html_outputs, 2);
        assert!(response.matrix.is_renderable());
    }
}
