#![cfg(feature = "server")]

use async_trait::async_trait;
use fly::{
    GrapesJsCodec, PageSelection, RenderPolicy, RuntimeContextScenario,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleasePolicy,
};
use rustok_api::{PortActor, PortContext};
use rustok_page_builder::adapters::FlyAdapterBackedPageBuilderService;
use rustok_page_builder::dto::{
    PreviewPageBuilderInput, PublishPageBuilderInput, PublishPageBuilderResult,
};
use rustok_page_builder::preview_port::PageBuilderPreviewRenderingPort;
use rustok_page_builder::runtime_scenario_release::{
    PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE, PageBuilderScenarioBaselineStore,
};
use rustok_page_builder::service::{
    PageBuilderCapabilityService, PageBuilderProjectSaveResult, PageBuilderProjectStore,
    PageBuilderServiceResult,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
struct CountingProjectStore {
    writes: Arc<AtomicUsize>,
}

#[async_trait]
impl PageBuilderProjectStore for CountingProjectStore {
    async fn load_project(
        &self,
        _context: &PortContext,
        _page_id: &str,
    ) -> PageBuilderServiceResult<Option<Value>> {
        Ok(None)
    }

    async fn save_project(
        &self,
        _context: &PortContext,
        page_id: &str,
        _revision_id: &str,
        _project_data: Value,
    ) -> PageBuilderServiceResult<PageBuilderProjectSaveResult> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(PageBuilderProjectSaveResult {
            page_id: page_id.to_string(),
            revision_id: "rev-persisted".to_string(),
            published: false,
        })
    }
}

#[derive(Clone, Default)]
struct NoopRenderer;

#[async_trait]
impl PageBuilderPreviewRenderingPort for NoopRenderer {
    async fn render_preview(
        &self,
        _context: &PortContext,
        _input: &PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<String> {
        Ok(String::new())
    }
}

#[derive(Clone)]
struct FixedBaselineStore {
    baseline: Option<RuntimeScenarioReleaseBaseline>,
}

#[async_trait]
impl PageBuilderScenarioBaselineStore for FixedBaselineStore {
    async fn load_scenario_baseline(
        &self,
        _context: &PortContext,
        _page_id: &str,
    ) -> PageBuilderServiceResult<Option<RuntimeScenarioReleaseBaseline>> {
        Ok(self.baseline.clone())
    }

    async fn save_scenario_baseline(
        &self,
        _context: &PortContext,
        _page_id: &str,
        _baseline: RuntimeScenarioReleaseBaseline,
    ) -> PageBuilderServiceResult<()> {
        Ok(())
    }
}

fn context() -> PortContext {
    PortContext::new("tenant-1", PortActor::user("author-1"), "en", "corr-1")
}

fn project(page_id: &str) -> Value {
    json!({
        "pages": [{
            "id": page_id,
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

fn baseline() -> RuntimeScenarioReleaseBaseline {
    let document = GrapesJsCodec::decode_value(project("home")).expect("baseline document");
    RuntimeScenarioReleaseBaseline::capture(
        "baseline-1",
        &document,
        &PageSelection::Id("home".to_string()),
        &RenderPolicy::default(),
        &[RuntimeContextScenario::new(
            "default",
            "Default",
            json!({ "page": { "title": "Welcome" } }),
        )],
    )
}

async fn publish(
    service: &FlyAdapterBackedPageBuilderService<
        CountingProjectStore,
        NoopRenderer,
        rustok_page_builder::runtime_telemetry::NoopPageBuilderRuntimeTelemetry,
        FixedBaselineStore,
    >,
    project_data: Value,
) -> PageBuilderServiceResult<PublishPageBuilderResult> {
    service
        .publish(
            &context(),
            PublishPageBuilderInput {
                page_id: "home".to_string(),
                revision_id: "rev-2".to_string(),
                project_data,
            },
        )
        .await
}

#[tokio::test]
async fn stable_baseline_allows_project_write() {
    let store = CountingProjectStore::default();
    let writes = Arc::clone(&store.writes);
    let service = FlyAdapterBackedPageBuilderService::new(store, NoopRenderer)
        .with_scenario_release_gate(
            FixedBaselineStore {
                baseline: Some(baseline()),
            },
            RuntimeScenarioReleasePolicy::require_stable(),
        );

    let result = publish(&service, project("home"))
        .await
        .expect("stable publish");
    assert_eq!(result.page_id, "home");
    assert_eq!(result.revision_id, "rev-persisted");
    assert!(!result.published);
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn broken_regression_blocks_before_project_write() {
    let store = CountingProjectStore::default();
    let writes = Arc::clone(&store.writes);
    let service = FlyAdapterBackedPageBuilderService::new(store, NoopRenderer)
        .with_scenario_release_gate(
            FixedBaselineStore {
                baseline: Some(baseline()),
            },
            RuntimeScenarioReleasePolicy::block_broken(),
        );

    let error = publish(&service, project("other"))
        .await
        .expect_err("broken regression must be rejected");
    assert!(
        error
            .to_string()
            .contains(PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE)
    );
    assert_eq!(writes.load(Ordering::SeqCst), 0);
}
