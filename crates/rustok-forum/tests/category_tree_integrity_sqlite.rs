use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_rejects_self_parent_and_category_cycles() -> TestResult<()> {
    let db = setup_sqlite().await?;
    let tenant_id = Uuid::new_v4();
    let root_id = Uuid::new_v4();
    let child_id = Uuid::new_v4();
    let grandchild_id = Uuid::new_v4();

    execute(
        &db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{root_id}', '{tenant_id}', 0, 0, 0, 0),
    ('{child_id}', '{tenant_id}', 1, 0, 0, 0),
    ('{grandchild_id}', '{tenant_id}', 2, 0, 0, 0);

UPDATE forum_categories SET parent_id = '{root_id}' WHERE id = '{child_id}';
UPDATE forum_categories SET parent_id = '{child_id}' WHERE id = '{grandchild_id}';
"#,
        ),
    )
    .await?;

    assert_rejected(
        &db,
        format!("UPDATE forum_categories SET parent_id = '{root_id}' WHERE id = '{root_id}'"),
        "self-parent category",
    )
    .await?;

    assert_rejected(
        &db,
        format!("UPDATE forum_categories SET parent_id = '{grandchild_id}' WHERE id = '{root_id}'"),
        "three-level category cycle",
    )
    .await?;

    execute(
        &db,
        format!("UPDATE forum_categories SET parent_id = '{root_id}' WHERE id = '{grandchild_id}'"),
    )
    .await?;
    execute(
        &db,
        format!("UPDATE forum_categories SET parent_id = NULL WHERE id = '{grandchild_id}'"),
    )
    .await?;

    Ok(())
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_tree_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    execute(
        &db,
        r#"
CREATE TABLE users (
    id TEXT NOT NULL PRIMARY KEY,
    tenant_id TEXT NOT NULL
)
"#
        .to_string(),
    )
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn execute(db: &DatabaseConnection, sql: String) -> TestResult<()> {
    db.execute_unprepared(&sql).await?;
    Ok(())
}

async fn assert_rejected(db: &DatabaseConnection, sql: String, label: &str) -> TestResult<()> {
    let result = db.execute_unprepared(&sql).await;
    assert!(result.is_err(), "{label} must be rejected");
    Ok(())
}
