//! Verification of reviewed publish layout template composition into immutable artifacts.
use std::error::Error;
use std::sync::Arc;

use chrono::Utc;
use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{page_static_landing_artifact, site_symbol};
use rustok_pages::services::{PageService, PageTemplateService};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_reviewed_publish_layout_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn insert_site_symbol(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    symbol_id: &str,
    text: &str,
) -> TestResult<()> {
    let now = Utc::now();
    site_symbol::ActiveModel {
        tenant_id: Set(tenant_id),
        locale: Set(locale.into()),
        symbol_id: Set(symbol_id.into()),
        name: Set(Some(symbol_id.into())),
        content: Set(json!({
            "id": symbol_id,
            "components": [
                {
                    "id": format!("{symbol_id}-content"),
                    "type": "text",
                    "content": text
                }
            ]
        })),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

fn valid_fly_project_json(content_text: &str) -> serde_json::Value {
    json!({
        "pages": [
            {
                "id": "landing-page",
                "flyPageMeta": {
                    "title": "Layout Test Page",
                    "description": "Verification of layout template publish composition",
                    "slug": "about-rustok"
                },
                "component": {
                    "id": "root-wrapper",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "body-heading",
                            "type": "heading",
                            "tagName": "h1",
                            "content": content_text
                        }
                    ]
                }
            }
        ]
    })
}

#[tokio::test]
async fn publish_reviewed_composes_page_template_layout_into_published_artifact() -> TestResult<()>
{
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let template_service = PageTemplateService::new(db.clone());
    let actor = SecurityContext::system();

    // 1. Create site symbols: header and footer
    insert_site_symbol(
        &db,
        tenant_id,
        "en",
        "site-header",
        "Global Header Navigation",
    )
    .await?;
    insert_site_symbol(
        &db,
        tenant_id,
        "en",
        "site-footer",
        "Global Footer Copyright",
    )
    .await?;

    // 2. Create a page template "main-layout" for "en" referencing the symbols
    template_service
        .save(
            tenant_id,
            &actor,
            "main-layout",
            "en",
            vec!["site-header".to_string()],
            vec!["site-footer".to_string()],
            None,
        )
        .await?;

    // 3. Create a page referencing template "main-layout"
    let page = service
        .create(
            tenant_id,
            actor.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "About RusToK".to_string(),
                    slug: Some("about-rustok".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("main-layout".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: valid_fly_project_json("Unique Main Body Content"),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;

    let body_response = page
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("page is missing its body"))?;
    let body_revision = body_response.updated_at.clone();

    // 4. Publish reviewed
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "layout-composition-test",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;

    let publish_result = service
        .publish_reviewed(
            tenant_id,
            actor.clone(),
            page.id,
            PublishPageInput {
                expected_version: page.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_revision,
                }],
                idempotency_key: "layout-publish-key-v1".to_string(),
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format.clone(),
                    scenario_id: reviewed.scenario_id.clone(),
                    context: reviewed.context.clone(),
                    review_hash: reviewed.review_hash.clone(),
                },
            },
        )
        .await?;

    assert_eq!(publish_result.page_id, page.id);
    assert!(!publish_result.replayed);

    // 5. Inspect the generated static landing artifact
    let artifact = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(page.id))
        .filter(page_static_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published static artifact is missing"))?;

    let html = artifact.document_html;
    // Verify that Header, Body, and Footer are present in the compiled document HTML
    assert!(
        html.contains("Global Header Navigation"),
        "HTML should contain layout header symbol content. Got: {html}"
    );
    assert!(
        html.contains("Unique Main Body Content"),
        "HTML should contain page body content. Got: {html}"
    );
    assert!(
        html.contains("Global Footer Copyright"),
        "HTML should contain layout footer symbol content. Got: {html}"
    );

    // Verify ordering: Header before Body, Body before Footer
    let header_idx = html.find("Global Header Navigation").unwrap();
    let body_idx = html.find("Unique Main Body Content").unwrap();
    let footer_idx = html.find("Global Footer Copyright").unwrap();
    assert!(
        header_idx < body_idx,
        "Header must appear before Body in rendered document"
    );
    assert!(
        body_idx < footer_idx,
        "Body must appear before Footer in rendered document"
    );

    // 6. Idempotent publish replay returns replayed receipt
    let replayed = service
        .publish_reviewed(
            tenant_id,
            actor,
            page.id,
            PublishPageInput {
                expected_version: page.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_response.updated_at.clone(),
                }],
                idempotency_key: "layout-publish-key-v1".to_string(),
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format,
                    scenario_id: reviewed.scenario_id,
                    context: reviewed.context,
                    review_hash: reviewed.review_hash,
                },
            },
        )
        .await?;
    assert!(replayed.replayed);

    Ok(())
}

#[tokio::test]
async fn publish_reviewed_fails_closed_when_template_is_missing() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let actor = SecurityContext::system();

    // Create a page with a non-existent template
    let page = service
        .create(
            tenant_id,
            actor.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Missing Template Page".to_string(),
                    slug: Some("missing-template-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("non-existent-layout".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: valid_fly_project_json("Orphan Page Content"),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;

    let body_response = page.body.as_ref().unwrap();
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "missing-layout-test",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;

    let result = service
        .publish_reviewed(
            tenant_id,
            actor,
            page.id,
            PublishPageInput {
                expected_version: page.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_response.updated_at.clone(),
                }],
                idempotency_key: "missing-template-publish-key".to_string(),
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format,
                    scenario_id: reviewed.scenario_id,
                    context: reviewed.context,
                    review_hash: reviewed.review_hash,
                },
            },
        )
        .await;

    assert!(
        result.is_err(),
        "Publishing with missing template must fail"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Page template `non-existent-layout` is not defined for locale `en`"),
        "Expected missing template error, got: {err_msg}"
    );

    Ok(())
}

#[tokio::test]
async fn save_document_strips_layout_nodes_before_persistence() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let actor = SecurityContext::system();

    let page = service
        .create(
            tenant_id,
            actor.clone(),
            CreatePageInput {
                template: Some("site".to_string()),
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Test Page".to_string(),
                    slug: Some("test-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: json!({
                        "pages": [{
                            "id": "p1",
                            "component": {
                                "id": "root",
                                "type": "wrapper",
                                "components": [
                                    { "id": "b1", "type": "heading", "tagName": "h1", "content": "Original Heading" }
                                ]
                            }
                        }]
                    }),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;

    let body_response = page.body.as_ref().unwrap();

    // Client sends back a document that includes composed layout slots
    let composed_content = json!({
        "pages": [{
            "id": "p1",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [
                    {
                        "id": "layout-header-1",
                        "type": "symbol",
                        "tagName": "header",
                        "attributes": {
                            "data-fly-layout-slot": "header",
                            "data-fly-locked": "true"
                        },
                        "components": [
                            { "id": "header-text", "type": "text", "content": "Header Symbol" }
                        ]
                    },
                    { "id": "b1", "type": "heading", "tagName": "h1", "content": "Updated Heading" },
                    {
                        "id": "layout-footer-1",
                        "type": "symbol",
                        "tagName": "footer",
                        "attributes": {
                            "data-fly-layout-slot": "footer",
                            "data-fly-locked": "true"
                        },
                        "components": [
                            { "id": "footer-text", "type": "text", "content": "Footer Symbol" }
                        ]
                    }
                ]
            }
        }]
    });

    let saved = service
        .save_document(
            tenant_id,
            actor.clone(),
            page.id,
            rustok_pages::dto::SavePageDocumentInput {
                body: PageBodyInput {
                    locale: "en".to_string(),
                    document: composed_content,
                },
                expected_revision: body_response.updated_at.clone(),
            },
        )
        .await?;

    let saved_body = saved.body.as_ref().unwrap();
    assert!(
        !saved_body.content.contains("data-fly-layout-slot"),
        "Persisted body must not contain layout slot attributes"
    );
    assert!(
        !saved_body.content.contains("layout-header-1"),
        "Persisted body must strip header slot node"
    );
    assert!(
        !saved_body.content.contains("layout-footer-1"),
        "Persisted body must strip footer slot node"
    );
    assert!(
        saved_body.content.contains("Updated Heading"),
        "Persisted body must retain authored page components"
    );

    let doc: serde_json::Value = serde_json::from_str(&saved_body.content)?;
    let components = doc["pages"][0]["component"]["components"]
        .as_array()
        .unwrap();
    assert_eq!(
        components.len(),
        1,
        "Only the 1 page body component must remain"
    );
    assert_eq!(components[0]["id"], "b1");

    Ok(())
}
