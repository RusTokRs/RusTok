use rustok_core::SecurityContext;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::services::PageService;
use tokio::sync::broadcast;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct TestContext {
    service: PageService,
    events: broadcast::Receiver<EventEnvelope>,
    tenant_id: Uuid,
}

#[tokio::test]
#[ignore = "Integration test requires database/migrations + indexer wiring"]
async fn page_create_emits_domain_event() -> TestResult<()> {
    let mut ctx = test_context().await?;
    let page = ctx
        .service
        .create(
            ctx.tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                template: None,
                publish: false,
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Test Page".to_string(),
                    slug: Some("test-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(serde_json::json!({
                        "pages": [{
                            "id": "main",
                            "component": {"id": "root", "type": "wrapper", "components": []}
                        }]
                    })),
                }),
                channel_slugs: None,
            },
        )
        .await?;

    let event = next_event(&mut ctx.events).await?;
    assert!(matches!(
        event.event,
        DomainEvent::NodeCreated { node_id, .. } if node_id == page.id
    ));
    Ok(())
}

async fn test_context() -> TestResult<TestContext> {
    Err("create test database connection and apply migrations".into())
}

async fn next_event(
    receiver: &mut broadcast::Receiver<EventEnvelope>,
) -> TestResult<EventEnvelope> {
    let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .map_err(|_| "timed out waiting for event")??;
    Ok(envelope)
}
