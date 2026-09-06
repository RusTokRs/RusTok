use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::PagesModule;
use rustok_pages::dto::{CreatePageInput, ListPagesFilter, PageTranslationInput};
use rustok_pages::error::PagesError;
use rustok_pages::services::PageService;
use rustok_test_utils::{
    db::setup_test_db,
    helpers::{admin_context, customer_context, manager_context},
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> (PageService, Uuid) {
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
    (PageService::new(db, bus), Uuid::new_v4())
}

async fn create_page(
    service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> rustok_pages::dto::PageResponse {
    service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: slug.to_string(),
                    slug: Some(slug.to_string()),
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
        .expect("page")
}

#[tokio::test]
async fn manager_cannot_publish_during_create_or_non_builder_lifecycle_transition() {
    let (service, tenant_id) = setup().await;
    let manager = manager_context();
    let create = service
        .create(
            tenant_id,
            manager.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Published".to_string(),
                    slug: Some("published".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: true,
            },
        )
        .await;
    assert!(matches!(create, Err(PagesError::Validation(_))));

    let draft = create_page(&service, tenant_id, manager.clone(), "draft").await;
    let publish = service
        .publish_non_builder_if_current(tenant_id, manager, draft.id, Some(draft.version))
        .await;
    assert!(matches!(publish, Err(PagesError::Forbidden(_))));
}

#[tokio::test]
async fn customer_reads_only_published_pages() {
    let (service, tenant_id) = setup().await;
    let admin = admin_context();
    let customer = customer_context();
    let draft = create_page(&service, tenant_id, admin.clone(), "draft").await;
    let publish_draft = create_page(&service, tenant_id, admin.clone(), "published").await;
    let published = service
        .publish_non_builder_if_current(
            tenant_id,
            admin,
            publish_draft.id,
            Some(publish_draft.version),
        )
        .await
        .expect("non-builder publish");

    assert!(matches!(
        service.get(tenant_id, customer.clone(), draft.id).await,
        Err(PagesError::Forbidden(_))
    ));
    assert_eq!(
        service
            .get(tenant_id, customer.clone(), published.id)
            .await
            .expect("published read")
            .id,
        published.id
    );
    let (items, total) = service
        .list(
            tenant_id,
            customer,
            ListPagesFilter {
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("public list");
    assert_eq!(total, 1);
    assert_eq!(items[0].id, published.id);
}
