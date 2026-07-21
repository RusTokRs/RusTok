use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::dto::{CreatePageInput, PageTranslationInput};
use rustok_pages::services::PageService;
use rustok_pages::PagesModule;
use rustok_test_utils::db::setup_test_db;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> (PageService, Uuid) {
    let db = setup_test_db().await;
    let module = PagesModule;
    let schema = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema)
        .await
        .expect("failed to apply outbox migrations");
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("failed to apply pages migrations");
    }

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (PageService::new(db, event_bus), Uuid::new_v4())
}

async fn create_translated_page(service: &PageService, tenant_id: Uuid) -> Uuid {
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![
                    PageTranslationInput {
                        locale: "en".to_string(),
                        title: "Home".to_string(),
                        slug: Some("home".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                    PageTranslationInput {
                        locale: "ru".to_string(),
                        title: "Дом".to_string(),
                        slug: Some("dom".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                ],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page draft should be created");
    service
        .publish_non_builder_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(draft.version),
        )
        .await
        .expect("page should be published")
        .id
}

#[tokio::test]
async fn get_by_slug_falls_back_to_platform_locale() {
    let (service, tenant_id) = setup().await;
    let page_id = create_translated_page(&service, tenant_id).await;

    let page = service
        .get_by_slug_with_locale_fallback(tenant_id, SecurityContext::system(), "fr", "home", None)
        .await
        .expect("lookup should succeed")
        .expect("page should resolve");

    assert_eq!(page.id, page_id);
    assert_eq!(page.effective_locale.as_deref(), Some("en"));
    assert_eq!(
        page.translation.and_then(|translation| translation.slug),
        Some("home".to_string())
    );
}

#[tokio::test]
async fn get_by_slug_respects_explicit_fallback_locale() {
    let (service, tenant_id) = setup().await;
    let page_id = create_translated_page(&service, tenant_id).await;

    let page = service
        .get_by_slug_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            "fr",
            "dom",
            Some("ru"),
        )
        .await
        .expect("lookup should succeed")
        .expect("page should resolve");

    assert_eq!(page.id, page_id);
    assert_eq!(page.effective_locale.as_deref(), Some("ru"));
    assert_eq!(
        page.translation.and_then(|translation| translation.slug),
        Some("dom".to_string())
    );
}

#[tokio::test]
async fn get_with_locale_fallback_normalizes_requested_and_fallback_locale() {
    let (service, tenant_id) = setup().await;
    let page_id = create_translated_page(&service, tenant_id).await;

    let page = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            page_id,
            "FR",
            Some("RU"),
        )
        .await
        .expect("lookup should succeed");

    assert_eq!(page.requested_locale.as_deref(), Some("fr"));
    assert_eq!(page.effective_locale.as_deref(), Some("ru"));
    assert_eq!(
        page.translation.and_then(|translation| translation.slug),
        Some("dom".to_string())
    );
}
