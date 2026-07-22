use fly::{
    FlyResult, GrapesJsCodec, PageSelection, RenderPolicy, RuntimeContextScenario,
    RuntimeScenarioRenderDiff, RuntimeScenarioRenderSnapshot,
    diff_runtime_scenario_render_snapshots,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioSnapshotRequest {
    pub project_data: Value,
    pub selection: PageSelection,
    #[serde(default)]
    pub policy: RenderPolicy,
    #[serde(default)]
    pub scenarios: Vec<RuntimeContextScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioSnapshotResponse {
    pub snapshot: RuntimeScenarioRenderSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioDiffRequest {
    pub previous: RuntimeScenarioRenderSnapshot,
    pub next: RuntimeScenarioRenderSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioProjectDiffRequest {
    pub previous: RuntimeScenarioRenderSnapshot,
    pub project_data: Value,
    pub selection: PageSelection,
    #[serde(default)]
    pub policy: RenderPolicy,
    #[serde(default)]
    pub scenarios: Vec<RuntimeContextScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioDiffResponse {
    pub diff: RuntimeScenarioRenderDiff,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeScenarioRegressionInspector;

impl PageBuilderRuntimeScenarioRegressionInspector {
    pub fn snapshot(
        &self,
        request: PageBuilderRuntimeScenarioSnapshotRequest,
    ) -> FlyResult<PageBuilderRuntimeScenarioSnapshotResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeScenarioSnapshotResponse {
            snapshot: RuntimeScenarioRenderSnapshot::capture(
                &document,
                &request.selection,
                &request.policy,
                &request.scenarios,
            ),
        })
    }

    pub fn diff(
        &self,
        request: PageBuilderRuntimeScenarioDiffRequest,
    ) -> PageBuilderRuntimeScenarioDiffResponse {
        PageBuilderRuntimeScenarioDiffResponse {
            diff: diff_runtime_scenario_render_snapshots(&request.previous, &request.next),
        }
    }

    pub fn diff_project(
        &self,
        request: PageBuilderRuntimeScenarioProjectDiffRequest,
    ) -> FlyResult<PageBuilderRuntimeScenarioDiffResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        let next = RuntimeScenarioRenderSnapshot::capture(
            &document,
            &request.selection,
            &request.policy,
            &request.scenarios,
        );
        Ok(PageBuilderRuntimeScenarioDiffResponse {
            diff: diff_runtime_scenario_render_snapshots(&request.previous, &next),
        })
    }
}

pub fn snapshot_page_builder_runtime_scenarios(
    request: PageBuilderRuntimeScenarioSnapshotRequest,
) -> FlyResult<PageBuilderRuntimeScenarioSnapshotResponse> {
    PageBuilderRuntimeScenarioRegressionInspector.snapshot(request)
}

pub fn diff_page_builder_runtime_scenario_snapshots(
    request: PageBuilderRuntimeScenarioDiffRequest,
) -> PageBuilderRuntimeScenarioDiffResponse {
    PageBuilderRuntimeScenarioRegressionInspector.diff(request)
}

pub fn diff_page_builder_runtime_scenario_project(
    request: PageBuilderRuntimeScenarioProjectDiffRequest,
) -> FlyResult<PageBuilderRuntimeScenarioDiffResponse> {
    PageBuilderRuntimeScenarioRegressionInspector.diff_project(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::RuntimeScenarioRegressionStatus;
    use serde_json::json;

    fn project_data() -> Value {
        json!({
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
        })
    }

    fn scenarios(title: &str) -> Vec<RuntimeContextScenario> {
        vec![RuntimeContextScenario::new(
            "default",
            "Default",
            json!({ "page": { "title": title } }),
        )]
    }

    #[test]
    fn consumer_can_snapshot_and_diff_scenario_outputs() {
        let previous =
            snapshot_page_builder_runtime_scenarios(PageBuilderRuntimeScenarioSnapshotRequest {
                project_data: project_data(),
                selection: PageSelection::First,
                policy: RenderPolicy::default(),
                scenarios: scenarios("Welcome"),
            })
            .expect("previous snapshot")
            .snapshot;
        let response = diff_page_builder_runtime_scenario_project(
            PageBuilderRuntimeScenarioProjectDiffRequest {
                previous,
                project_data: project_data(),
                selection: PageSelection::First,
                policy: RenderPolicy::default(),
                scenarios: scenarios("Changed"),
            },
        )
        .expect("diff response");
        assert_eq!(
            response.diff.status,
            RuntimeScenarioRegressionStatus::RequiresReview
        );
        assert!(!response.diff.changes.is_empty());
    }

    #[test]
    fn snapshot_response_roundtrips() {
        let response =
            snapshot_page_builder_runtime_scenarios(PageBuilderRuntimeScenarioSnapshotRequest {
                project_data: project_data(),
                selection: PageSelection::First,
                policy: RenderPolicy::default(),
                scenarios: scenarios("Welcome"),
            })
            .expect("snapshot response");
        let value = serde_json::to_value(&response).expect("serialize response");
        let decoded: PageBuilderRuntimeScenarioSnapshotResponse =
            serde_json::from_value(value).expect("deserialize response");
        assert_eq!(decoded, response);
    }
}
