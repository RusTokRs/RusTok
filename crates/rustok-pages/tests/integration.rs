use rustok_core::SecurityContext;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput, UpdatePageInput};
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
async fn test_page_lifecycle() -> TestResult<()> {
    let mut ctx = test_context().await?;

    let input = CreatePageInput {
        template: None,
        publish: true,
        translations: vec![PageTranslationInput {
            locale: "en".to_string(),
            title: "Test Page".to_string(),
            slug: None,
            meta_title: None,
            meta_description: None,
        }],
        body: Some(PageBodyInput {
            locale: "en".to_string(),
            content: "Hello, Pages!".to_string(),
            format: Some("markdown".to_string()),
            content_json: None,
        }),
        blocks: None,
        channel_slugs: None,
    };

    let page = ctx
        .service
        .create(ctx.tenant_id, SecurityContext::system(), input)
        .await?;

    let created_event = next_event(&mut ctx.events).await?;
    assert!(matches!(
        created_event.event,
        DomainEvent::NodeCreated { node_id, .. } if node_id == page.id
    ));

    let indexed = wait_for_index(&ctx, page.id).await?;
    assert_eq!(indexed.title, "Test Page");

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires database/migrations + indexer wiring"]
async fn test_create_page_rt_json_v1_sanitizes_payload() -> TestResult<()> {
    let ctx = test_context().await?;
    let locale = "en";

    let input = CreatePageInput {
        template: None,
        publish: false,
        translations: vec![PageTranslationInput {
            locale: locale.to_string(),
            title: "RT page".to_string(),
            slug: Some("rt-page".to_string()),
            meta_title: None,
            meta_description: None,
        }],
        body: Some(PageBodyInput {
            locale: locale.to_string(),
            content: String::new(),
            format: Some("rt_json_v1".to_string()),
            content_json: Some(serde_json::json!({
                "version": "rt_json_v1",
                "locale": locale,
                "doc": {
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Hello",
                                    "marks": [
                                        { "type": "bold" },
                                        { "type": "unknown_mark" }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            })),
        }),
        blocks: None,
        channel_slugs: None,
    };

    let page = ctx
        .service
        .create(ctx.tenant_id, SecurityContext::system(), input)
        .await?;

    let body = page.body.expect("body expected");
    assert_eq!(body.format, "rt_json_v1");
    let content_json = body.content_json.expect("content_json expected");
    let marks = content_json["doc"]["content"][0]["content"][0]["marks"]
        .as_array()
        .expect("marks array expected");
    assert_eq!(marks.len(), 1, "unknown mark should be sanitized out");

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires database/migrations + indexer wiring"]
async fn test_update_page_rt_json_v1_sanitizes_payload() -> TestResult<()> {
    let ctx = test_context().await?;
    let locale = "en";

    let created = ctx
        .service
        .create(
            ctx.tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                template: None,
                publish: false,
                translations: vec![PageTranslationInput {
                    locale: locale.to_string(),
                    title: "Page".to_string(),
                    slug: Some("page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                body: Some(PageBodyInput {
                    locale: locale.to_string(),
                    content: "Initial".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
            },
        )
        .await?;

    let updated = ctx
        .service
        .update(
            ctx.tenant_id,
            SecurityContext::system(),
            created.id,
            UpdatePageInput {
                translations: None,
                template: None,
                status: None,
                body: Some(PageBodyInput {
                    locale: locale.to_string(),
                    content: String::new(),
                    format: Some("rt_json_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "version": "rt_json_v1",
                        "locale": locale,
                        "doc": {
                            "type": "doc",
                            "content": [
                                {
                                    "type": "script"
                                },
                                {
                                    "type": "paragraph",
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": "Updated"
                                        }
                                    ]
                                }
                            ]
                        }
                    })),
                }),
                channel_slugs: None,
            },
        )
        .await?;

    let body = updated.body.expect("updated body expected");
    assert_eq!(body.format, "rt_json_v1");
    let content = body.content_json.expect("content_json expected");
    let nodes = content["doc"]["content"]
        .as_array()
        .expect("doc content should be array");
    assert_eq!(nodes.len(), 1, "unsupported node should be sanitized out");

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

struct IndexedPage {
    title: String,
}

async fn wait_for_index(_ctx: &TestContext, _page_id: Uuid) -> TestResult<IndexedPage> {
    Err("wire index module or test double for read model lookup".into())
}
