use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageTranslationInput, SavePageDocumentInput,
};
use rustok_pages::services::PageService;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, PageService, Uuid) {
    let db = setup_test_db().await;
    let schema = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema)
        .await
        .expect("outbox migration should apply");
    for migration in PagesModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("Pages migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    (db, service, Uuid::new_v4())
}

fn translation(locale: &str, title: &str, slug: Option<&str>) -> PageTranslationInput {
    PageTranslationInput {
        locale: locale.to_string(),
        title: title.to_string(),
        slug: slug.map(ToOwned::to_owned),
        meta_title: Some(format!("{title} SEO")),
        meta_description: Some(format!("{title} description")),
    }
}

fn builder_body(locale: &str) -> PageBodyInput {
    PageBodyInput {
        locale: locale.to_string(),
        content: String::new(),
        format: Some("grapesjs".to_string()),
        content_json: Some(serde_json::json!({
            "pages": [{
                "id": "main",
                "component": {"id": "root", "type": "wrapper", "components": []}
            }]
        })),
    }
}

#[tokio::test]
async fn unicode_slug_and_seo_storage_are_language_agnostic() {
    let (_db, service, tenant_id) = setup().await;
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("ru", "Дом", None)],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("unicode draft should be created");
    let page = service
        .publish_non_builder_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(draft.version),
        )
        .await
        .expect("unicode page should be published");

    assert_eq!(
        page.translation
            .as_ref()
            .and_then(|item| item.slug.as_deref()),
        Some("дом")
    );
    assert!(page.metadata.get("seo").is_none());
    let routed = service
        .get_by_slug(tenant_id, SecurityContext::system(), "ru", "дом")
        .await
        .expect("unicode route lookup should succeed")
        .expect("unicode route should resolve");
    assert_eq!(routed.id, page.id);
}

#[tokio::test]
async fn create_rejects_body_locale_without_translation() {
    let (_db, service, tenant_id) = setup().await;
    let error = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("en", "Home", Some("home"))],
                template: None,
                body: Some(builder_body("de")),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect_err("create must reject a body without same-locale metadata");
    assert!(
        error
            .to_string()
            .contains("requires a matching page translation")
    );
}

#[tokio::test]
async fn document_locale_requires_matching_translation() {
    let (_db, service, tenant_id) = setup().await;
    let page = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("en", "Home", Some("home"))],
                template: None,
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page should be created");

    let error = service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            page.id,
            SavePageDocumentInput {
                expected_revision: format!("page:{}:initial", page.id),
                body: builder_body("de"),
            },
        )
        .await
        .expect_err("body without translation must fail");
    assert!(
        error
            .to_string()
            .contains("requires a matching page translation")
    );
}

#[tokio::test]
async fn response_never_mixes_translation_and_body_locales() {
    let (_db, service, tenant_id) = setup().await;
    let page = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![
                    translation("en", "Home", Some("home")),
                    translation("ru", "Дом", Some("dom")),
                ],
                template: None,
                body: Some(builder_body("en")),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page should be created");

    let localized = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            page.id,
            "ru",
            Some("en"),
        )
        .await
        .expect("localized page should resolve");
    assert_eq!(localized.effective_locale.as_deref(), Some("ru"));
    assert_eq!(
        localized
            .translation
            .as_ref()
            .and_then(|item| item.slug.as_deref()),
        Some("dom")
    );
    assert!(
        localized.body.is_none(),
        "English body must not be mixed into Russian metadata"
    );
}

#[tokio::test]
async fn sqlite_db_rejects_cross_locale_body_rows() {
    let (db, service, tenant_id) = setup().await;
    let page = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("en", "Home", Some("home"))],
                template: None,
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page should be created");

    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO page_bodies (id, page_id, locale, content, format, updated_at, tenant_id) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, ?)",
            [
                Uuid::new_v4().into(),
                page.id.into(),
                "de".into(),
                "{}".into(),
                "grapesjs".into(),
                tenant_id.into(),
            ],
        ))
        .await;
    assert!(
        result.is_err(),
        "DB trigger must reject a body locale without translation"
    );
}
