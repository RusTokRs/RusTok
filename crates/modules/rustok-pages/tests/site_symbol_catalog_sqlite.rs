//! Site symbols catalog: tenant/locale isolation, descriptor validation,
//! and deletion reference protection across page templates and page bodies.

use std::error::Error;

use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_pages::entities::page_body;
use rustok_pages::{
    PageTemplateService, PagesModule, SaveSiteSymbolInput, SiteSymbolService,
};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Set,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

async fn fixture() -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:site_symbols_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE pages (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, author_id TEXT, status TEXT NOT NULL, template TEXT NOT NULL, metadata TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, published_at TEXT, archived_at TEXT, version INTEGER NOT NULL)",
    )
    .await?;
    db.execute_unprepared(
        "CREATE TABLE page_bodies (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, page_id TEXT NOT NULL, locale TEXT NOT NULL, content TEXT NOT NULL, format TEXT NOT NULL, updated_at TEXT NOT NULL)",
    )
    .await?;
    db.execute_unprepared(
        "CREATE TABLE page_body_drafts (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, page_id TEXT NOT NULL, locale TEXT NOT NULL, content TEXT NOT NULL, format TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, created_by TEXT)",
    )
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        let name = migration.name();
        if name.ends_with("create_site_symbols") || name.ends_with("create_page_templates") {
            migration.up(&manager).await?;
        }
    }
    Ok(db)
}

#[tokio::test]
async fn save_validates_identifiers_and_symbol_descriptors() -> TestResult<()> {
    let db = fixture().await?;
    let tenant = Uuid::new_v4();
    let service = SiteSymbolService::new(db);
    let actor = SecurityContext::system();

    // Invalid identifier fails
    assert!(
        service
            .save(
                tenant,
                &actor,
                SaveSiteSymbolInput {
                    symbol_id: "invalid/slash".into(),
                    locale: "en".into(),
                    name: Some("Bad ID".into()),
                    content: json!({"components": []}),
                },
            )
            .await
            .is_err()
    );

    // Empty or malformed descriptor fails
    assert!(
        service
            .save(
                tenant,
                &actor,
                SaveSiteSymbolInput {
                    symbol_id: "valid-header".into(),
                    locale: "en".into(),
                    name: Some("Bad Content".into()),
                    content: json!(null),
                },
            )
            .await
            .is_err()
    );

    // Valid symbol descriptor succeeds
    let saved = service
        .save(
            tenant,
            &actor,
            SaveSiteSymbolInput {
                symbol_id: "site-header".into(),
                locale: "en".into(),
                name: Some("Site Header".into()),
                content: json!({
                    "id": "site-header",
                    "name": "Site Header",
                    "components": [{
                        "id": "c1",
                        "type": "text",
                        "content": "Navigation"
                    }]
                }),
            },
        )
        .await?;

    assert_eq!(saved.symbol_id, "site-header");
    assert_eq!(saved.locale, "en");
    assert_eq!(saved.name.as_deref(), Some("Site Header"));
    assert_eq!(saved.content["components"][0]["content"], "Navigation");

    // Updating existing symbol
    let updated = service
        .save(
            tenant,
            &actor,
            SaveSiteSymbolInput {
                symbol_id: "site-header".into(),
                locale: "en".into(),
                name: Some("Updated Header".into()),
                content: json!({
                    "id": "site-header",
                    "components": [{
                        "id": "c1",
                        "type": "text",
                        "content": "Updated Navigation"
                    }]
                }),
            },
        )
        .await?;

    assert_eq!(updated.name.as_deref(), Some("Updated Header"));
    assert_eq!(updated.content["components"][0]["content"], "Updated Navigation");

    Ok(())
}

#[tokio::test]
async fn tenant_and_locale_isolation() -> TestResult<()> {
    let db = fixture().await?;
    let tenant1 = Uuid::new_v4();
    let tenant2 = Uuid::new_v4();
    let service = SiteSymbolService::new(db);
    let actor = SecurityContext::system();

    service
        .save(
            tenant1,
            &actor,
            SaveSiteSymbolInput {
                symbol_id: "main-nav".into(),
                locale: "en".into(),
                name: Some("Main Nav EN".into()),
                content: json!({"id": "main-nav", "components": []}),
            },
        )
        .await?;

    service
        .save(
            tenant1,
            &actor,
            SaveSiteSymbolInput {
                symbol_id: "main-nav".into(),
                locale: "ru".into(),
                name: Some("Main Nav RU".into()),
                content: json!({"id": "main-nav", "components": []}),
            },
        )
        .await?;

    let list_en = service.list(tenant1, &actor, "en").await?;
    assert_eq!(list_en.len(), 1);
    assert_eq!(list_en[0].name.as_deref(), Some("Main Nav EN"));

    let list_ru = service.list(tenant1, &actor, "ru").await?;
    assert_eq!(list_ru.len(), 1);
    assert_eq!(list_ru[0].name.as_deref(), Some("Main Nav RU"));

    let list_other = service.list(tenant2, &actor, "en").await?;
    assert!(list_other.is_empty());

    let fetched = service.get(tenant1, &actor, "main-nav", "en").await?;
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().symbol_id, "main-nav");

    let fetched_missing = service.get(tenant2, &actor, "main-nav", "en").await?;
    assert!(fetched_missing.is_none());

    Ok(())
}

#[tokio::test]
async fn deletion_reference_protection() -> TestResult<()> {
    let db = fixture().await?;
    let tenant = Uuid::new_v4();
    let symbol_service = SiteSymbolService::new(db.clone());
    let template_service = PageTemplateService::new(db.clone());
    let actor = SecurityContext::system();

    // 1. Create site symbol "site-footer"
    symbol_service
        .save(
            tenant,
            &actor,
            SaveSiteSymbolInput {
                symbol_id: "site-footer".into(),
                locale: "en".into(),
                name: Some("Site Footer".into()),
                content: json!({"id": "site-footer", "components": []}),
            },
        )
        .await?;

    // 2. Reference in a page template
    template_service
        .save(
            tenant,
            &actor,
            "landing-layout",
            "en",
            vec![],
            vec!["site-footer".into()],
            None,
        )
        .await?;

    // Attempting to delete symbol while referenced by template fails
    let delete_err = symbol_service
        .delete(tenant, &actor, "site-footer", "en")
        .await;
    assert!(delete_err.is_err(), "Must reject deleting symbol referenced by template");

    // Remove the template
    template_service
        .delete(tenant, &actor, "landing-layout", "en", 1)
        .await?;

    // 3. Reference in a page body
    let page_id = Uuid::new_v4();
    let body_content = json!({
        "pages": [{
            "id": "p1",
            "component": {
                "id": "c1",
                "type": "wrapper",
                "components": [{
                    "id": "instance1",
                    "symbolId": "site-footer"
                }]
            }
        }]
    })
    .to_string();

    page_body::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant),
        page_id: Set(page_id),
        locale: Set("en".into()),
        content: Set(body_content),
        format: Set(PAGE_BUILDER_DOCUMENT_FORMAT.into()),
        updated_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    // Attempting to delete symbol while referenced by page body fails
    let delete_page_err = symbol_service
        .delete(tenant, &actor, "site-footer", "en")
        .await;
    assert!(delete_page_err.is_err(), "Must reject deleting symbol referenced by page body");

    // Delete the page body
    db.execute_unprepared("DELETE FROM page_bodies").await?;

    // 4. Deleting now succeeds
    symbol_service
        .delete(tenant, &actor, "site-footer", "en")
        .await?;

    let remaining = symbol_service.list(tenant, &actor, "en").await?;
    assert!(remaining.is_empty());

    Ok(())
}
