#![cfg(feature = "server")]

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext};
use rustok_page_builder::adapters::FlyAdapterBackedPageBuilderService;
use rustok_page_builder::dto::{PageBuilderPreviewRuntime, PreviewPageBuilderInput};
use rustok_page_builder::preview_port::PageBuilderPreviewRenderingPort;
use rustok_page_builder::service::{
    PageBuilderCapabilityService, PageBuilderProjectSaveResult, PageBuilderProjectStore,
    PageBuilderServiceResult,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Default)]
struct NoopStore;

#[async_trait]
impl PageBuilderProjectStore for NoopStore {
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
        Ok(PageBuilderProjectSaveResult {
            page_id: page_id.to_string(),
            revision_id: "unused".to_string(),
            published: false,
        })
    }
}

#[derive(Clone, Default)]
struct RecordingRenderer {
    calls: Arc<AtomicUsize>,
    runtime: Arc<Mutex<Option<PageBuilderPreviewRuntime>>>,
}

#[async_trait]
impl PageBuilderPreviewRenderingPort for RecordingRenderer {
    async fn render_preview(
        &self,
        _context: &PortContext,
        input: &PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.runtime.lock().expect("runtime lock") = Some(input.runtime.clone());
        Ok(format!(
            "<h1>{}</h1>",
            input.runtime.context["page"]["title"]
                .as_str()
                .unwrap_or_default()
        ))
    }
}

fn port_context() -> PortContext {
    PortContext::new("tenant-1", PortActor::user("author-1"), "en", "preview-1")
}

fn project() -> Value {
    json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": []
            }
        }]
    })
}

#[tokio::test]
async fn preview_passes_canonical_runtime_to_port_and_response() {
    let renderer = RecordingRenderer::default();
    let observed = Arc::clone(&renderer.runtime);
    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, renderer);

    let response = service
        .preview(
            &port_context(),
            PreviewPageBuilderInput::new("home", project()).with_runtime(
                PageBuilderPreviewRuntime::new(
                    json!({ "page": { "title": "Scenario title" } }),
                    Some("mobile-checkout".to_string()),
                ),
            ),
        )
        .await
        .expect("preview response");

    assert_eq!(response.runtime_scenario_id.as_deref(), Some("mobile-checkout"));
    assert_eq!(response.html, "<h1>Scenario title</h1>");
    let observed = observed.lock().expect("runtime lock").clone().expect("runtime");
    assert_eq!(observed.context["page"]["title"], "Scenario title");
    assert_eq!(observed.scenario_id.as_deref(), Some("mobile-checkout"));
}

#[tokio::test]
async fn preview_rejects_non_object_runtime_before_renderer() {
    let renderer = RecordingRenderer::default();
    let calls = Arc::clone(&renderer.calls);
    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, renderer);

    let error = service
        .preview(
            &port_context(),
            PreviewPageBuilderInput::new("home", project())
                .with_runtime(PageBuilderPreviewRuntime::new(json!(["invalid"]), None)),
        )
        .await
        .expect_err("non-object context must fail");

    assert!(error.to_string().contains("runtime context must be a JSON object"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn preview_rejects_oversized_runtime_before_renderer() {
    let renderer = RecordingRenderer::default();
    let calls = Arc::clone(&renderer.calls);
    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, renderer);
    let oversized = "x".repeat(256 * 1024);

    let error = service
        .preview(
            &port_context(),
            PreviewPageBuilderInput::new("home", project()).with_runtime(
                PageBuilderPreviewRuntime::new(json!({ "payload": oversized }), None),
            ),
        )
        .await
        .expect_err("oversized context must fail");

    assert!(error.to_string().contains("runtime context exceeds 262144 bytes"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
