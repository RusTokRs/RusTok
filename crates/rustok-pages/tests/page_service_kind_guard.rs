use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::PagesModule;
use rustok_pages::dto::{CreatePageInput, PageTranslationInput};
use rustok_pages::error::PagesError;
use rustok_pages::services::PageService;
use rustok_test_utils::{db::setup_test_db, helpers::admin_context};
use sea_orm_migration::{MigrationTrait, SchemaManager};
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

#[tokio::test]
async fn lifecycle_operations_reject_unknown_page_ids() {
    let (service, tenant_id, security) = setup().await;
    let unknown = Uuid::new_v4();
    assert!(matches!(
        service.publish(tenant_id, security.clone(), unknown).await,
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
async fn explicit_publish_and_unpublish_advance_metadata_version() {
    let (service, tenant_id, security) = setup().await;
    let draft = create_draft(&service, tenant_id, security.clone()).await;
    let published = service
        .publish_if_current(tenant_id, security.clone(), draft.id, Some(draft.version))
        .await
        .expect("publish");
    assert_eq!(published.version, draft.version + 1);

    let unpublished = service
        .unpublish_if_current(tenant_id, security, draft.id, Some(published.version))
        .await
        .expect("unpublish");
    assert_eq!(unpublished.version, published.version + 1);
}

#[tokio::test]
async fn published_pages_must_be_unpublished_before_delete() {
    let (service, tenant_id, security) = setup().await;
    let draft = create_draft(&service, tenant_id, security.clone()).await;
    let published = service
        .publish_if_current(tenant_id, security.clone(), draft.id, Some(draft.version))
        .await
        .expect("publish");

    assert!(matches!(
        service.delete(tenant_id, security, published.id).await,
        Err(PagesError::CannotDeletePublished)
    ));
}
