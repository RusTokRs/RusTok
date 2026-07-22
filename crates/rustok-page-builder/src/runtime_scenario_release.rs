use fly::{FlyResult, GrapesJsCodec, evaluate_runtime_scenario_release};
pub use fly::{
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleaseMode,
    RuntimeScenarioReleasePolicy, RuntimeScenarioReleaseStatus, RuntimeScenarioRenderChange,
    RuntimeScenarioRenderChangeImpact,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE: &str = "SCENARIO_REGRESSION_BLOCKED";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderScenarioBaselineChange {
    pub baseline: Option<RuntimeScenarioReleaseBaseline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_note: Option<String>,
}

impl PageBuilderScenarioBaselineChange {
    pub fn save(baseline: RuntimeScenarioReleaseBaseline, promotion_note: Option<String>) -> Self {
        Self {
            baseline: Some(baseline),
            promotion_note,
        }
    }

    pub fn clear(promotion_note: Option<String>) -> Self {
        Self {
            baseline: None,
            promotion_note,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioReleaseRequest {
    pub project_data: Value,
    #[serde(default)]
    pub baseline: Option<RuntimeScenarioReleaseBaseline>,
    #[serde(default)]
    pub policy: RuntimeScenarioReleasePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeScenarioReleaseResponse {
    pub evaluation: RuntimeScenarioReleaseEvaluation,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeScenarioReleaseInspector;

impl PageBuilderRuntimeScenarioReleaseInspector {
    pub fn evaluate(
        &self,
        request: PageBuilderRuntimeScenarioReleaseRequest,
    ) -> FlyResult<PageBuilderRuntimeScenarioReleaseResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeScenarioReleaseResponse {
            evaluation: evaluate_runtime_scenario_release(
                &document,
                request.baseline.as_ref(),
                request.policy,
            ),
        })
    }
}

pub fn evaluate_page_builder_runtime_scenario_release(
    request: PageBuilderRuntimeScenarioReleaseRequest,
) -> FlyResult<PageBuilderRuntimeScenarioReleaseResponse> {
    PageBuilderRuntimeScenarioReleaseInspector.evaluate(request)
}

#[cfg(feature = "server")]
mod server {
    use super::*;
    use crate::service::{PageBuilderServiceError, PageBuilderServiceResult};
    use async_trait::async_trait;
    use rustok_api::PortContext;

    #[async_trait]
    pub trait PageBuilderScenarioBaselineStore: Send + Sync {
        async fn load_scenario_baseline(
            &self,
            context: &PortContext,
            page_id: &str,
        ) -> PageBuilderServiceResult<Option<RuntimeScenarioReleaseBaseline>>;

        async fn save_scenario_baseline(
            &self,
            context: &PortContext,
            page_id: &str,
            baseline: RuntimeScenarioReleaseBaseline,
        ) -> PageBuilderServiceResult<()>;
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub struct NoopPageBuilderScenarioBaselineStore;

    #[async_trait]
    impl PageBuilderScenarioBaselineStore for NoopPageBuilderScenarioBaselineStore {
        async fn load_scenario_baseline(
            &self,
            _context: &PortContext,
            _page_id: &str,
        ) -> PageBuilderServiceResult<Option<RuntimeScenarioReleaseBaseline>> {
            Ok(None)
        }

        async fn save_scenario_baseline(
            &self,
            _context: &PortContext,
            _page_id: &str,
            _baseline: RuntimeScenarioReleaseBaseline,
        ) -> PageBuilderServiceResult<()> {
            Err(PageBuilderServiceError::Runtime(
                "scenario baseline persistence is not configured".to_string(),
            ))
        }
    }

    pub fn release_gate_error(
        evaluation: &RuntimeScenarioReleaseEvaluation,
    ) -> PageBuilderServiceError {
        let details = evaluation
            .blocking_diagnostics()
            .take(4)
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>();
        let message = if details.is_empty() {
            format!(
                "{PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE}: runtime scenario release status {:?} is not allowed",
                evaluation.status
            )
        } else {
            format!(
                "{PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE}: {}",
                details.join("; ")
            )
        };
        PageBuilderServiceError::Validation(message)
    }
}

#[cfg(feature = "server")]
pub use server::*;

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{PageSelection, RenderPolicy, RuntimeContextScenario};
    use serde_json::json;

    fn project() -> Value {
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

    #[test]
    fn consumer_can_evaluate_persisted_release_baseline() {
        let project_data = project();
        let document = GrapesJsCodec::decode_value(project_data.clone()).expect("document");
        let baseline = RuntimeScenarioReleaseBaseline::capture(
            "baseline-1",
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &[RuntimeContextScenario::new(
                "default",
                "Default",
                json!({ "page": { "title": "Welcome" } }),
            )],
        );
        let response = evaluate_page_builder_runtime_scenario_release(
            PageBuilderRuntimeScenarioReleaseRequest {
                project_data,
                baseline: Some(baseline),
                policy: RuntimeScenarioReleasePolicy::require_stable(),
            },
        )
        .expect("release response");
        assert!(response.evaluation.allowed);
    }

    #[test]
    fn baseline_change_carries_review_note_separately() {
        let document = GrapesJsCodec::decode_value(project()).expect("document");
        let baseline = RuntimeScenarioReleaseBaseline::capture(
            "baseline-1",
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &[],
        );
        let change = PageBuilderScenarioBaselineChange::save(
            baseline,
            Some("Reviewed visual update".to_string()),
        );
        assert_eq!(
            change.promotion_note.as_deref(),
            Some("Reviewed visual update")
        );
    }
}
