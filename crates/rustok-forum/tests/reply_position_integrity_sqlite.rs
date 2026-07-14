use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{CreateReplyInput, ForumModule, ReplyService};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_enforces_unique_positive_reply_positions() -> TestResult<()> {
    let db = setup_sqlite().await?;
    apply_migrations(&db).await?;
    let seed = seed_forum(&db).await?;

    let service = ReplyService::new(db.clone(), event_bus(db.clone()));
    for index in 0..3 {
        service
            .create(
                seed.tenant_id,
                customer_security(),
                seed.topic_id,
                reply_input(&format!("reply {index}")),
            )
            .await?;
    }

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT CAST(position AS INTEGER) AS value
                 FROM forum_replies
                 WHERE tenant_id = '{}' AND topic_id = '{}'
                 ORDER BY position",
                seed.tenant_id, seed.topic_id
            ),
        ))
        .await?;
    let positions = rows
        .into_iter()
        .map(|row| row.try_get("", "value"))
        .collect::<Result<Vec<i64>, _>>()?;
    if positions != vec![1, 2, 3] {
        return Err(test_error(format!(
            "sequential SQLite positions are invalid: {positions:?}"
        )));
    }

    let first_id = scalar_text(
        &db,
        format!(
            "SELECT CAST(id AS TEXT) AS value
             FROM forum_replies
             WHERE tenant_id = '{}' AND topic_id = '{}'
             ORDER BY position
             LIMIT 1",
            seed.tenant_id, seed.topic_id
        ),
    )
    .await?;
    let last_id = scalar_text(
        &db,
        format!(
            "SELECT CAST(id AS TEXT) AS value
             FROM forum_replies
             WHERE tenant_id = '{}' AND topic_id = '{}'
             ORDER BY position DESC
             LIMIT 1",
            seed.tenant_id, seed.topic_id
        ),
    )
    .await?;

    assert_rejected(
        &db,
        format!("UPDATE forum_replies SET position = 1 WHERE id = '{last_id}'"),
        "duplicate reply position update",
    )
    .await?;
    assert_rejected(
        &db,
        format!("UPDATE forum_replies SET position = 0 WHERE id = '{first_id}'"),
        "non-positive reply position update",
    )
    .await?;
    assert_rejected(
        &db,
        format!(
            "INSERT INTO forum_replies
                (id, tenant_id, topic_id, status, position)
             VALUES
                ('{}', '{}', '{}', 'approved', 2)",
            Uuid::new_v4(),
            seed.tenant_id,
            seed.topic_id
        ),
        "duplicate reply position insert",
    )
    .await?;

    Ok(())
}

#[derive(Clone, Copy)]
struct ForumSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_reply_positions_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn apply_migrations(db: &DatabaseConnection) -> TestResult<()> {
    let manager = SchemaManager::new(db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(())
}

async fn seed_forum(db: &DatabaseConnection) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
    };
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories
            (id, tenant_id, position, moderated, topic_count, reply_count)
         VALUES
            ('{}', '{}', 0, 0, 1, 0);
         INSERT INTO forum_topics
            (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
         VALUES
            ('{}', '{}', '{}', 'open', '{{}}', 0, 0, 0);",
        seed.category_id, seed.tenant_id, seed.topic_id, seed.tenant_id, seed.category_id,
    ))
    .await?;
    Ok(seed)
}

async fn assert_rejected(db: &DatabaseConnection, sql: String, label: &str) -> TestResult<()> {
    if db.execute_unprepared(&sql).await.is_ok() {
        return Err(test_error(format!("{label} must be rejected")));
    }
    Ok(())
}

async fn scalar_text(db: &DatabaseConnection, sql: String) -> TestResult<String> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn customer_security() -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()))
}

fn reply_input(content: &str) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".to_string(),
        content: content.to_string(),
        content_format: "markdown".to_string(),
        content_json: None,
        parent_reply_id: None,
    }
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
