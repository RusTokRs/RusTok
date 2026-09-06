use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{CreateReplyInput, ForumModule, ModerationService, ReplyService};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_enforces_locked_and_moderated_reply_semantics() -> TestResult<()> {
    let db = setup_sqlite().await?;
    apply_migrations(&db).await?;

    let locked = seed_forum(&db, false, true).await?;
    let locked_result = ReplyService::new(db.clone(), event_bus(db.clone()))
        .create(
            locked.tenant_id,
            customer_security(locked.author_id),
            locked.topic_id,
            reply_input("must be rejected"),
        )
        .await;
    if locked_result.is_ok() {
        return Err(test_error("locked topic accepted an ordinary reply"));
    }

    let moderated = seed_forum(&db, true, false).await?;
    let bus = event_bus(db.clone());
    let reply = ReplyService::new(db.clone(), bus.clone())
        .create(
            moderated.tenant_id,
            customer_security(moderated.author_id),
            moderated.topic_id,
            reply_input("pending reply"),
        )
        .await?;
    if reply.status != "pending" {
        return Err(test_error(format!(
            "moderated category produced unexpected reply status `{}`",
            reply.status
        )));
    }

    assert_public_state(&db, &moderated, 0, 0).await?;

    let moderation = ModerationService::new(db.clone(), bus);
    moderation
        .approve_reply(
            moderated.tenant_id,
            reply.id,
            moderated.topic_id,
            admin_security(),
        )
        .await?;
    assert_public_state(&db, &moderated, 1, 1).await?;

    moderation
        .hide_reply(
            moderated.tenant_id,
            reply.id,
            moderated.topic_id,
            admin_security(),
        )
        .await?;
    assert_public_state(&db, &moderated, 0, 1).await?;

    Ok(())
}

#[derive(Clone, Copy)]
struct ForumSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    author_id: Uuid,
}

async fn setup_sqlite() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_moderation_semantics_{}?mode=memory&cache=shared",
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
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await?;
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(())
}

fn sql_uuid(id: Uuid) -> String {
    format!("X'{}'", hex::encode(id.as_bytes()))
}

async fn seed_forum(
    db: &DatabaseConnection,
    moderated: bool,
    locked: bool,
) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
    };
    let cat_id = sql_uuid(seed.category_id);
    let tenant_id = sql_uuid(seed.tenant_id);
    let top_id = sql_uuid(seed.topic_id);
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories
            (id, tenant_id, position, moderated, topic_count, reply_count)
         VALUES
            ({cat_id}, {tenant_id}, 0, {}, 1, 0);
         INSERT INTO forum_topics
            (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
         VALUES
            ({top_id}, {tenant_id}, {cat_id}, 'open', '{{}}', {}, {}, 0);",
        if moderated { 1 } else { 0 },
        0,
        if locked { 1 } else { 0 },
    ))
    .await?;
    Ok(seed)
}

async fn assert_public_state(
    db: &DatabaseConnection,
    seed: &ForumSeed,
    expected_count: i64,
    expected_replied_events: i64,
) -> TestResult<()> {
    let tid = sql_uuid(seed.tenant_id);
    let topid = sql_uuid(seed.topic_id);
    let cid = sql_uuid(seed.category_id);
    let uid = sql_uuid(seed.author_id);
    let topic_count = scalar_i64(
        db,
        format!(
            "SELECT CAST(COALESCE((SELECT reply_count FROM forum_topics WHERE tenant_id = {tid} AND id = {topid}), 0) AS INTEGER) AS value",
        ),
    )
    .await?;
    let category_count = scalar_i64(
        db,
        format!(
            "SELECT CAST(COALESCE((SELECT reply_count FROM forum_categories WHERE tenant_id = {tid} AND id = {cid}), 0) AS INTEGER) AS value",
        ),
    )
    .await?;
    let user_count = scalar_i64(
        db,
        format!(
            "SELECT CAST(COALESCE((SELECT reply_count FROM forum_user_stats WHERE tenant_id = {tid} AND user_id = {uid}), 0) AS INTEGER) AS value",
        ),
    )
    .await?;
    let replied_events = scalar_i64(
        db,
        "SELECT CAST(COUNT(*) AS INTEGER) AS value
         FROM sys_events
         WHERE event_type = 'forum.topic.replied'",
    )
    .await?;

    if topic_count != expected_count
        || category_count != expected_count
        || user_count != expected_count
        || replied_events != expected_replied_events
    {
        return Err(test_error(format!(
            "unexpected public reply state: topic={topic_count}, category={category_count}, \
             user={user_count}, events={replied_events}; expected count={expected_count}, \
             events={expected_replied_events}"
        )));
    }
    Ok(())
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn customer_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(user_id))
}

fn admin_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn reply_input(content: &str) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".to_string(),
        content: rustok_api::RichTextDocument::single_paragraph(content),
        parent_reply_id: None,
    }
}

async fn scalar_i64(db: &DatabaseConnection, sql: impl Into<String>) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, sql.into()))
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
