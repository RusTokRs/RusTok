//! Page starter presets catalog and instantiation tests.
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::{PageService, PageStarterService, PagesModule, SavePageStarterInput};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:page_starters_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await?;
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
        .await?;

    db.execute_unprepared(
        "CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            settings TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await?;

    db.execute_unprepared(
        "CREATE TABLE pages (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            author_id TEXT,
            status TEXT NOT NULL,
            template TEXT NOT NULL,
            metadata TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            published_at TEXT,
            archived_at TEXT,
            version INTEGER NOT NULL
        )",
    )
    .await?;

    let manager = SchemaManager::new(&db);
    let _ = rustok_outbox::SysEventsMigration.up(&manager).await;
    let migrations = PagesModule.migrations();
    // Run the additive migrations for bodies, translations, symbols, templates, and starters
    for migration in migrations {
        let _ = migration.up(&manager).await;
    }
    Ok(db)
}

#[tokio::test]
async fn starter_catalog_returns_builtins_when_tenant_has_no_overrides() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let starter_service = PageStarterService::new(db);
    let actor = SecurityContext::system();

    let all = starter_service.list(tenant_id, &actor, None).await?;
    assert!(all.len() >= 3, "Expected at least 3 built-in starters");

    let landing_only = starter_service
        .list(tenant_id, &actor, Some("landing"))
        .await?;
    assert_eq!(landing_only.len(), 1);
    assert_eq!(landing_only[0].key, "landing-product");

    let about = starter_service
        .get(tenant_id, &actor, "about-company")
        .await?;
    assert!(about.is_some());
    assert_eq!(about.unwrap().category, "about");

    Ok(())
}

#[tokio::test]
async fn custom_starters_are_tenant_isolated_and_instantiate_into_clean_pages() -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let db = setup_db(tenant_a).await?;
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_b}')"))
        .await?;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let page_service = PageService::new(db.clone(), event_bus);
    let starter_service = PageStarterService::new(db);
    let actor = SecurityContext::system();

    // 1. Tenant A saves a custom starter
    let custom = starter_service
        .save(
            tenant_a,
            &actor,
            SavePageStarterInput {
                key: "custom-promo".to_string(),
                category: "marketing".to_string(),
                name: "Custom Summer Promo".to_string(),
                description: Some("Tenant-specific promotional blueprint.".to_string()),
                thumbnail_url: None,
                default_template_key: Some("default".to_string()),
                content: json!({
                    "flyDocumentKind": "starter_preset",
                    "pages": [{
                        "id": "p1",
                        "component": {
                            "id": "root",
                            "type": "wrapper",
                            "components": [
                                { "id": "promo-h1", "type": "heading", "tagName": "h1", "content": "Summer Deal" },
                                { "id": "promo-text", "type": "text", "content": "Limited time offer." }
                            ]
                        }
                    }]
                }),
            },
        )
        .await?;

    assert_eq!(custom.key, "custom-promo");

    // Tenant A sees custom starter
    let starters_a = starter_service.list(tenant_a, &actor, None).await?;
    assert_eq!(starters_a.len(), 1);
    assert_eq!(starters_a[0].key, "custom-promo");

    // Tenant B sees default built-ins (tenant isolation)
    let starters_b = starter_service.list(tenant_b, &actor, None).await?;
    assert!(
        starters_b.iter().all(|s| s.key != "custom-promo"),
        "Tenant B must not see Tenant A's custom starter"
    );

    // 2. Instantiate custom starter into a clean, remapped page document
    let instantiated_json = PageStarterService::instantiate_starter(&custom)?;
    let parsed: serde_json::Value = serde_json::from_str(&instantiated_json)?;

    // Remapped IDs must differ from original
    let comp_ids = parsed["pages"][0]["component"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(!comp_ids.contains(&"promo-h1".to_string()));
    assert!(!comp_ids.contains(&"promo-text".to_string()));

    // 3. Create a real page using the instantiated starter
    let page = page_service
        .create(
            tenant_a,
            actor.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Summer Promotion Page".to_string(),
                    slug: Some("summer-promo".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: parsed,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;

    assert_eq!(page.template, "default");
    assert!(page.body.is_some());
    let page_body = page.body.unwrap();
    assert!(page_body.content.contains("Summer Deal"));
    assert!(page_body.content.contains("Limited time offer."));

    Ok(())
}
