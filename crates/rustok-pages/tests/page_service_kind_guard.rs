use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::PagesModule;
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::error::PagesError;
use rustok_pages::services::{PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED, PageService};
use rustok_test_utils::{db::setup_test_db, helpers::admin_context};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> (PageService, Uuid, SecurityContext) {
    let db = setup_test_db().await;
    let schema = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema)
        .await
        .expect("outbox migrations");
    for migration in PagesModule.migrations() {
        migration.up(&schema).await.expect("pages migrations");
    }
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (PageService::new(db, bus), Uuid::new_v4(), admin_context())
}

async fn create_draft(
    service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
) -> rustok_pages::dto::PageResponse {
    service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Page".to_string(),
                    slug: Some("page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("draft page")
}

async fn create_builder_draft(
    service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
) -> rustok_pages::dto::PageResponse {
    service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Builder Page".to_string(),
                    slug: Some("builder-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(json!({
                        "assets": [],
                        "styles": [],
                        "pages": [{
                            "id": "main",
                            "component": {
                                "id": "root",
                                "type": "wrapper",
                                "components": []
                            }
                        }]
                    })),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("builder draft page")
}

#[tokio::test]
async fn lifecycle_operations_reject_unknown_page_ids() {
    let (service, tenant_id, security) = setup().await;
    let unknown = Uuid::new_v4();
    assert!(matches!(
        service
            .publish_non_builder(tenant_id, security.clone(), unknown)
            .await,
        Err(PagesError::PageNotFound(id)) if id == unknown
    ));
    assert!(matches!(
        service.unpublish(tenant_id, security.clone(), unknown).await,
        Err(PagesError::PageNotFound(id)) if id == unknown
    ));
    assert!(matches!(
        service.delete(tenant_id, security, unknown).await,
        Err(PagesError::PageNotFound(id)) if id == unknown
    ));
}

#[tokio::test]
async fn explicit_non_builder_publish_and_unpublish_advance_metadata_version() {
    let (service, tenant_id, security) = setup().await;
    let draft = create_draft(&service, tenant_id, security.clone()).await;
    let published = service
        .publish_non_builder_if_current(tenant_id, security.clone(), draft.id, Some(draft.version))
        .await
        .expect("non-builder publish");
    assert_eq!(published.version, draft.version + 1);

    let unpublished = service
        .unpublish_if_current(tenant_id, security, draft.id, Some(published.version))
        .await
        .expect("unpublish");
    assert_eq!(unpublished.version, published.version + 1);
}

#[tokio::test]
async fn non_builder_lifecycle_rejects_builder_documents_with_stable_code() {
    let (service, tenant_id, security) = setup().await;
    let draft = create_builder_draft(&service, tenant_id, security.clone()).await;
    let result = service
        .publish_non_builder_if_current(tenant_id, security, draft.id, Some(draft.version))
        .await;
    assert!(matches!(
        result,
        Err(PagesError::Rich(error))
            if error.error_code.as_deref() == Some(PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED)
    ));
}

#[tokio::test]
async fn published_pages_must_be_unpublished_before_delete() {
    let (service, tenant_id, security) = setup().await;
    let draft = create_draft(&service, tenant_id, security.clone()).await;
    let published = service
        .publish_non_builder_if_current(tenant_id, security.clone(), draft.id, Some(draft.version))
        .await
        .expect("non-builder publish");

    assert!(matches!(
        service.delete(tenant_id, security, published.id).await,
        Err(PagesError::CannotDeletePublished)
    ));
}
