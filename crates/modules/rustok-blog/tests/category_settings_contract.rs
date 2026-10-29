use std::sync::Arc;

use rustok_blog::BlogModule;
use rustok_blog::dto::CreateCategoryInput;
use rustok_blog::services::CategoryService;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup_with_blog_schema() -> (DatabaseConnection, Box<dyn MigrationTrait>) {
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
    assert_eq!(
        migrations.len(),
        1,
        "Blog module should have a single consolidated migration"
    );
    let schema_migration = migrations.remove(0);
    schema_migration
        .up(&manager)
        .await
        .expect("Blog consolidated schema migration should apply");
    (db, schema_migration)
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
    let _receiver = transport.subscribe();
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
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Sqlite => "UPDATE blog_categories SET settings = ? WHERE id = ?",
        DatabaseBackend::Postgres => {
            "UPDATE blog_categories SET settings = CAST($1 AS jsonb) WHERE id = $2"
        }
        other => panic!("unsupported test database backend: {other:?}"),
    };
    db.execute_raw(Statement::from_sql_and_values(
        backend,
        sql,
        [
            serde_json::to_string(&settings)
                .expect("settings JSON should serialize")
                .into(),
            category_id.into(),
        ],
    ))
    .await
    .map(|_| ())
}

#[tokio::test]
async fn settings_contract_guards_database_invariants_and_rejects_invalid_values() {
    let (db, schema_migration) = setup_with_blog_schema().await;
    let category_id = create_category(&db).await;

    // Clean object settings are accepted
    write_settings(&db, category_id, serde_json::json!({ "theme": "dark" }))
        .await
        .expect("clean object settings must be accepted");

    // Non-object settings (array) must be rejected
    let scalar_error = write_settings(&db, category_id, serde_json::json!([]))
        .await
        .expect_err("database guard must reject non-object settings");
    assert!(scalar_error.to_string().contains("settings"));

    // Oversized settings (>64 KiB) must be rejected
    let oversized = serde_json::json!({ "padding": "x".repeat(70 * 1024) });
    let oversized_error = write_settings(&db, category_id, oversized)
        .await
        .expect_err("database guard must reject oversized settings");
    assert!(oversized_error.to_string().contains("settings"));

    // Rollback and reapply of schema migration works cleanly
    let manager = SchemaManager::new(&db);
    schema_migration
        .down(&manager)
        .await
        .expect("schema rollback should drop tables cleanly");
    schema_migration
        .up(&manager)
        .await
        .expect("schema should reapply cleanly");
}
