//! Pages layout catalog: tenant/locale isolation, CAS, symbol references and safe deletion.
use std::error::Error;

use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_pages::entities::site_symbol;
use rustok_pages::{PageTemplateService, PagesModule};
use sea_orm::{ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Set};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

async fn fixture() -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(format!("sqlite:file:page_templates_{}?mode=memory&cache=shared", Uuid::new_v4()));
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared("CREATE TABLE pages (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, author_id TEXT, status TEXT NOT NULL, template TEXT NOT NULL, metadata TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, published_at TEXT, archived_at TEXT, version INTEGER NOT NULL)").await?;
    let manager = SchemaManager::new(&db);
    // The catalog is an additive migration; create only its two owner tables.
    let migrations = PagesModule.migrations();
    for migration in migrations.into_iter().rev().take(2).collect::<Vec<_>>().into_iter().rev() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn symbol(db: &DatabaseConnection, tenant_id: Uuid, locale: &str, id: &str) -> TestResult<()> {
    let now = Utc::now();
    site_symbol::ActiveModel {
        tenant_id: Set(tenant_id), locale: Set(locale.into()), symbol_id: Set(id.into()),
        name: Set(Some(id.into())),
        content: Set(json!({"id": id, "components": [{"id": "section", "type": "text", "content": "Shared"}]})),
        created_at: Set(now.into()), updated_at: Set(now.into()),
    }.insert(db).await?;
    Ok(())
}

#[tokio::test]
async fn catalog_is_tenant_and_exact_locale_scoped_with_revision_cas() -> TestResult<()> {
    let db = fixture().await?;
    let tenant = Uuid::new_v4();
    let other = Uuid::new_v4();
    let service = PageTemplateService::new(db.clone());
    let actor = SecurityContext::system();
    symbol(&db, tenant, "en", "header").await?;
    assert!(service.save(tenant, &actor, "site", "ru", vec!["header".into()], vec![], None).await.is_err());
    assert!(service.save(other, &actor, "site", "en", vec!["header".into()], vec![], None).await.is_err());
    let created = service.save(tenant, &actor, "site", "en", vec!["header".into()], vec![], None).await?;
    assert_eq!(created.revision, 1);
    assert!(service.save(tenant, &actor, "site", "en", vec![], vec![], None).await.is_err());
    let updated = service.save(tenant, &actor, "site", "en", vec![], vec!["header".into()], Some(1)).await?;
    assert_eq!(updated.revision, 2);
    assert!(service.save(tenant, &actor, "site", "en", vec![], vec![], Some(1)).await.is_err());
    assert_eq!(service.list(tenant, &actor, "en").await?, vec![updated]);
    assert!(service.list(other, &actor, "en").await?.is_empty());
    assert!(service.list(tenant, &actor, "ru").await?.is_empty());
    assert!(service.delete(tenant, &actor, "site", "en", 1).await.is_err());
    db.execute_unprepared(&format!("INSERT INTO pages(id, tenant_id, status, template, metadata, created_at, updated_at, version) VALUES ('{}','{}','draft','site','{{}}',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,1)", Uuid::new_v4(), tenant)).await?;
    assert!(service.delete(tenant, &actor, "site", "en", 2).await.is_err(), "assigned layout cannot be removed");
    db.execute_unprepared("DELETE FROM pages").await?;
    service.delete(tenant, &actor, "site", "en", 2).await?;
    assert!(service.list(tenant, &actor, "en").await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn preview_resolves_layout_without_writing_the_source() -> TestResult<()> {
    let db = fixture().await?;
    let tenant = Uuid::new_v4();
    let service = PageTemplateService::new(db.clone());
    let actor = SecurityContext::system();
    symbol(&db, tenant, "en", "header").await?;
    service.save(tenant, &actor, "site", "en", vec!["header".into()], vec![], None).await?;
    let source = json!({"pages":[{"id":"home","flyPageMeta":{"title":"Home","description":"Preview","slug":"home"},"component":{"id":"root","type":"wrapper","components":[{"id":"body","type":"text","content":"Body"}]}}]}).to_string();
    let project = service.preview_document(tenant, &actor, "site", "en", &source).await?;
    assert_eq!(project["pages"][0]["component"]["components"][0]["tagName"], "header");
    assert_eq!(project["pages"][0]["component"]["components"][1]["content"], "Body");
    assert!(project.get("flySymbols").is_none());
    assert!(service.preview_document(tenant, &actor, "site", "ru", &source).await.is_err());
    let unsafe_source = source.replace("\"type\":\"text\"", "\"type\":\"iframe\"");
    assert!(service.preview_document(tenant, &actor, "site", "en", &unsafe_source).await.is_err());
    assert_eq!(source, json!({"pages":[{"id":"home","flyPageMeta":{"title":"Home","description":"Preview","slug":"home"},"component":{"id":"root","type":"wrapper","components":[{"id":"body","type":"text","content":"Body"}]}}]}).to_string());
    Ok(())
}

#[tokio::test]
async fn authoring_requires_tenant_wide_authority() -> TestResult<()> {
    let db = fixture().await?;
    let tenant = Uuid::new_v4();
    let service = PageTemplateService::new(db);
    let public = SecurityContext::public_read();
    assert!(service.list(tenant, &public, "en").await.is_err());
    assert!(service.save(tenant, &public, "site", "en", vec![], vec![], None).await.is_err());
    Ok(())
}
