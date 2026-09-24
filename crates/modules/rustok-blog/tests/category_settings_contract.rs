use std::sync::Arc;

use rustok_blog::dto::CreateCategoryInput;
use rustok_blog::services::CategoryService;
use rustok_blog::BlogModule;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup_without_settings_contract() -> (DatabaseConnection, Box<dyn MigrationTrait>) {
    let db = rustok_test_utils::db::setup_test_db().await;
    let manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    let mut migrations = BlogModule.migrations();
    let settings_contract = migrations
        .pop()
        .expect("Blog settings contract migration should be registered");
    for migration in migrations {
        migration
            .up(&manager)
            .await
            .expect("Blog prerequisite migration should apply");
    }
    (db, settings_contract)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn input(name: &str) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: Some(name.to_ascii_lowercase().replace(' ', "-")),
        description: None,
        parent_id: None,
        position: Some(0),
        settings: serde_json::json!({}),
    }
}

async fn create_category(db: &DatabaseConnection) -> Uuid {
    let transport = Arc::new(MemoryTransport::new());
    let event_bus = TransactionalEventBus::new(transport);
    CategoryService::new(db.clone(), event_bus)
        .create(Uuid::new_v4(), admin(), input("Settings contract"))
        .await
        .expect("category should be created")
}

async fn write_settings(
    db: &DatabaseConnection,
    category_id: Uuid,
    settings: serde_json::Value,
) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE blog_categories SET settings = ? WHERE id = ?",
        [serde_json::to_string(&settings).expect("settings JSON should serialize").into(), category_id.into()],
    ))
    .await
    .map(|_| ())
}

#[tokio::test]
async fn settings_contract_fails_preflight_on_dirty_rows_and_retries_cleanly() {
    let (db, settings_contract) = setup_without_settings_contract().await;
    let category_id = create_category(&db).await;

    write_settings(&db, category_id, serde_json::json!([]))
        .await
        .expect("legacy schema must allow dirty settings before contract migration");

    let manager = SchemaManager::new(&db);
    let error = settings_contract
        .up(&manager)
        .await
        .expect_err("dirty settings must block contract installation");
    assert!(error.to_string().contains("invalid row"));

    write_settings(&db, category_id, serde_json::json!({}))
        .await
        .expect("dirty settings should be repairable before retry");
    settings_contract
        .up(&manager)
        .await
        .expect("clean settings should allow contract installation");

    settings_contract
        .down(&manager)
        .await
        .expect("contract rollback should remove database guards");
    write_settings(&db, category_id, serde_json::json!([]))
        .await
        .expect("rolled back contract should no longer guard settings");
    write_settings(&db, category_id, serde_json::json!({}))
        .await
        .expect("invalid probe value should be repairable before reapply");
    settings_contract
        .up(&manager)
        .await
        .expect("contract should reapply cleanly");

    let scalar_error = write_settings(&db, category_id, serde_json::json!([]))
        .await
        .expect_err("database guard must reject non-object settings");
    assert!(scalar_error.to_string().contains("blog category settings"));

    let oversized = serde_json::json!({ "padding": "x".repeat(70 * 1024) });
    let oversized_error = write_settings(&db, category_id, oversized)
        .await
        .expect_err("database guard must reject oversized settings");
    assert!(oversized_error.to_string().contains("blog category settings"));
}