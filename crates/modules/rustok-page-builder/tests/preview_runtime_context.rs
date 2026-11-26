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
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
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

    assert_eq!(
        response.runtime_scenario_id.as_deref(),
        Some("mobile-checkout")
    );
    assert_eq!(response.html, "<h1>Scenario title</h1>");
    let observed = observed
        .lock()
        .expect("runtime lock")
        .clone()
        .expect("runtime");
    assert_eq!(observed.context["page"]["title"], "Scenario title");
    assert_eq!(observed.scenario_id.as_deref(), Some("mobile-checkout"));
}

#[tokio::test]
async fn preview_rejects_blank_page_id_before_renderer() {
    let renderer = RecordingRenderer::default();
    let calls = Arc::clone(&renderer.calls);
    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, renderer);

    for page_id in ["", " \t\n"] {
        let error = service
            .preview(&port_context(), PreviewPageBuilderInput::new(page_id, project()))
            .await
            .expect_err("blank page identity must fail");
        assert!(matches!(
            error,
            rustok_page_builder::service::PageBuilderServiceError::Validation(message)
                if message == "page_id must not be empty"
        ));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
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

    assert!(
        error
            .to_string()
            .contains("runtime context must be a JSON object")
    );
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

    assert!(
        error
            .to_string()
            .contains("runtime context exceeds 262144 bytes")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn preview_runtime_byte_budget_counts_escaped_utf8_and_accepts_exact_limit() {
    use rustok_page_builder::dto::MAX_PREVIEW_RUNTIME_CONTEXT_BYTES;

    // {"payload":""} is 14 bytes; each escaped newline is two bytes.
    let payload = "\n".repeat((MAX_PREVIEW_RUNTIME_CONTEXT_BYTES - 14) / 2);
    let exact = PageBuilderPreviewRuntime::new(json!({ "payload": payload }), None);
    assert_eq!(
        serde_json::to_vec(&exact.context).expect("compact JSON").len(),
        MAX_PREVIEW_RUNTIME_CONTEXT_BYTES
    );
    exact.validate().expect("exact byte budget");
    let oversized = PageBuilderPreviewRuntime::new(
        json!({ "payload": format!("{}é", exact.context["payload"].as_str().unwrap()) }),
        None,
    );
    assert!(oversized.validate().expect_err("over budget").to_string().contains("exceeds 262144 bytes"));
}

#[tokio::test]
async fn preview_rejects_deep_runtime_before_renderer_and_drops_it_safely() {
    let renderer = RecordingRenderer::default();
    let calls = Arc::clone(&renderer.calls);
    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, renderer);
    // Build without json! to avoid recursively serializing the nested Value.
    let mut nested = Value::Null;
    for _ in 0..50_000 {
        nested = Value::Array(vec![nested]);
    }
    let runtime = PageBuilderPreviewRuntime::new(
        Value::Object(serde_json::Map::from_iter([("payload".to_string(), nested)])),
        None,
    );
    let error = service
        .preview(
            &port_context(),
            PreviewPageBuilderInput::new("home", project()).with_runtime(runtime),
        )
        .await
        .expect_err("deep context must fail");
    assert!(error.to_string().contains("maximum JSON depth"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

fn deep_object() -> Value {
    let mut nested = Value::Null;
    for _ in 0..50_000 {
        nested = Value::Array(vec![nested]);
    }
    Value::Object(serde_json::Map::from_iter([("payload".to_string(), nested)]))
}

#[tokio::test]
async fn publish_rejects_deep_project_and_preserves_identity_error_precedence() {
    use rustok_page_builder::dto::PublishPageBuilderInput;
    use rustok_page_builder::service::PageBuilderServiceError;

    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, RecordingRenderer::default());
    for (page, revision, expected) in [
        ("", "rev-1", "page_id must not be empty"),
        ("home", "", "revision_id must not be empty"),
    ] {
        let error = service.publish(
            &port_context(),
            PublishPageBuilderInput::new(page, revision, deep_object()),
        ).await.expect_err("invalid identity");
        assert!(matches!(error, PageBuilderServiceError::Validation(message) if message == expected));
    }
    let error = service.publish(
        &port_context(),
        PublishPageBuilderInput::new("home", "rev-1", deep_object()),
    ).await.expect_err("deep project");
    assert!(matches!(error, PageBuilderServiceError::Validation(_)));
}

#[tokio::test]
async fn properties_rejects_deep_payload() {
    use rustok_page_builder::dto::BuilderNodePropertiesInput;
    use rustok_page_builder::service::PageBuilderServiceError;

    let service = FlyAdapterBackedPageBuilderService::new(NoopStore, RecordingRenderer::default());
    let error = service.properties(
        &port_context(),
        BuilderNodePropertiesInput {
            page_id: "home".into(),
            node_id: "root".into(),
            properties: deep_object(),
        },
    ).await.expect_err("deep properties");
    assert!(matches!(error, PageBuilderServiceError::Validation(_)));
}

struct DeepProjectStore;

#[async_trait]
impl PageBuilderProjectStore for DeepProjectStore {
    async fn load_project(
        &self,
        _context: &PortContext,
        _page_id: &str,
    ) -> PageBuilderServiceResult<Option<Value>> {
        Ok(Some(deep_object()))
    }

    async fn save_project(
        &self,
        _context: &PortContext,
        _page_id: &str,
        _revision_id: &str,
        _project_data: Value,
    ) -> PageBuilderServiceResult<PageBuilderProjectSaveResult> {
        unreachable!("read-only test store")
    }
}

#[tokio::test]
async fn tree_and_properties_reject_deep_project_returned_by_store() {
    use rustok_page_builder::dto::{BuilderNodePropertiesInput, BuilderTreeInput};
    use rustok_page_builder::service::PageBuilderServiceError;

    let service = FlyAdapterBackedPageBuilderService::new(
        DeepProjectStore,
        RecordingRenderer::default(),
    );
    let tree_error = service.tree(
        &port_context(),
        BuilderTreeInput { page_id: "home".into() },
    ).await.expect_err("deep stored project");
    assert!(matches!(tree_error, PageBuilderServiceError::Validation(_)));
    let properties_error = service.properties(
        &port_context(),
        BuilderNodePropertiesInput {
            page_id: "home".into(),
            node_id: "root".into(),
            properties: json!({}),
        },
    ).await.expect_err("deep stored project");
    assert!(matches!(properties_error, PageBuilderServiceError::Validation(_)));
}

#[tokio::test]
async fn denied_requests_discard_deep_payloads_without_invoking_provider() {
    use rustok_page_builder::dto::{PageBuilderCapabilityRequest, PublishPageBuilderInput};
    use rustok_page_builder::rollout::BuilderToggleProfile;
    use rustok_page_builder::service::{
        AuthorizedPageBuilderHandlers, CapabilityGuardedService, PageBuilderRequestAuth,
        PageBuilderServiceError,
    };

    let renderer = RecordingRenderer::default();
    let calls = Arc::clone(&renderer.calls);
    let provider = FlyAdapterBackedPageBuilderService::new(NoopStore, renderer);
    let guarded = CapabilityGuardedService::new(provider, BuilderToggleProfile::BuilderOff.flags());
    let disabled = guarded.preview(
        &port_context(),
        PreviewPageBuilderInput::new("home", deep_object())
            .with_runtime(PageBuilderPreviewRuntime::new(deep_object(), None)),
    ).await.expect_err("disabled preview");
    assert!(matches!(disabled, PageBuilderServiceError::CapabilityDisabled(_)));

    let handlers = AuthorizedPageBuilderHandlers::new(guarded);
    let denied = handlers.handle(
        &port_context(),
        &PageBuilderRequestAuth::new(vec![]),
        PageBuilderCapabilityRequest::Publish(PublishPageBuilderInput::new(
            "home", "rev-1", deep_object(),
        )),
    ).await.expect_err("unauthorized publish");
    assert!(matches!(denied, PageBuilderServiceError::Forbidden(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
