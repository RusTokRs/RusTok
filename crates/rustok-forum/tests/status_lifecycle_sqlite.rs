use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_rejects_unknown_forum_lifecycle_statuses() -> TestResult<()> {
    let db = setup_sqlite().await?;
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();

    execute(
        &db,
        format!(
            "INSERT INTO forum_categories
                (id, tenant_id, position, moderated, topic_count, reply_count)
             VALUES
                ('{category_id}', '{tenant_id}', 0, 0, 0, 0)"
        ),
    )
    .await?;

    assert_rejected(
        &db,
        format!(
            "INSERT INTO forum_topics
                (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES
                ('{}', '{tenant_id}', '{category_id}', 'unknown', '{{}}', 0, 0, 0)",
            Uuid::new_v4()
        ),
        "unknown topic insert status",
    )
    .await?;

    execute(
        &db,
        format!(
            "INSERT INTO forum_topics
                (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES
                ('{topic_id}', '{tenant_id}', '{category_id}', 'open', '{{}}', 0, 0, 0)"
        ),
    )
    .await?;

    assert_rejected(
        &db,
        format!("UPDATE forum_topics SET status = 'unknown' WHERE id = '{topic_id}'"),
        "unknown topic update status",
    )
    .await?;

    assert_rejected(
        &db,
        format!(
            "INSERT INTO forum_replies
                (id, tenant_id, topic_id, status, position)
             VALUES
                ('{}', '{tenant_id}', '{topic_id}', 'unknown', 1)",
            Uuid::new_v4()
        ),
        "unknown reply insert status",
    )
    .await?;

    execute(
        &db,
        format!(
            "INSERT INTO forum_replies
                (id, tenant_id, topic_id, status, position)
             VALUES
                ('{reply_id}', '{tenant_id}', '{topic_id}', 'approved', 1)"
        ),
    )
    .await?;

    assert_rejected(
        &db,
        format!("UPDATE forum_replies SET status = 'unknown' WHERE id = '{reply_id}'"),
        "unknown reply update status",
    )
    .await?;

    for status in ["closed", "archived", "open"] {
        execute(
            &db,
            format!("UPDATE forum_topics SET status = '{status}' WHERE id = '{topic_id}'"),
        )
        .await?;
    }
    for status in [
        "pending",
        "approved",
        "rejected",
        "hidden",
        "flagged",
        "deleted",
    ] {
        execute(
            &db,
            format!("UPDATE forum_replies SET status = '{status}' WHERE id = '{reply_id}'"),
        )
        .await?;
    }

    Ok(())
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_status_lifecycle_{}?mode=memory&cache=shared",
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
CREATE TABLE taxonomy_terms (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_value TEXT NOT NULL DEFAULT '',
    canonical_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
)
"#
        .to_string(),
    )
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn execute(db: &DatabaseConnection, sql: String) -> TestResult<()> {
    db.execute_unprepared(&sql).await?;
    Ok(())
}

async fn assert_rejected(
    db: &DatabaseConnection,
    sql: String,
    label: &str,
) -> TestResult<()> {
    let result = db.execute_unprepared(&sql).await;
    assert!(result.is_err(), "{label} must be rejected");
    Ok(())
}
