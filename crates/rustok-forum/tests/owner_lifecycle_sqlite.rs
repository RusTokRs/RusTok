use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CreateReplyInput, ForumError, ForumModule, ModerationService, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn owner_reply_commands_enforce_lock_moderation_and_soft_delete() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let moderator_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let locked_topic_id = Uuid::new_v4();
    let moderated_topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, true).await;
    seed_topic(
        &db,
        tenant_id,
        category_id,
        locked_topic_id,
        author_id,
        true,
    )
    .await;
    seed_topic(
        &db,
        tenant_id,
        category_id,
        moderated_topic_id,
        author_id,
        false,
    )
    .await;

    let service = ReplyService::new(db.clone(), event_bus(db.clone()));
    let owner = SecurityContext::new(UserRole::Manager, Some(author_id));
    let locked_error = service
        .create(
            tenant_id,
            owner.clone(),
            locked_topic_id,
            reply_input("locked reply"),
        )
        .await
        .expect_err("locked topic must reject owner reply command");
    assert!(matches!(locked_error, ForumError::TopicLocked));

    let pending = service
        .create(
            tenant_id,
            owner.clone(),
            moderated_topic_id,
            reply_input("pending reply"),
        )
        .await
        .expect("moderated reply should be stored as pending");
    assert_eq!(pending.status, "pending");
    assert_eq!(topic_reply_count(&db, moderated_topic_id).await, 0);
    assert_eq!(category_reply_count(&db, category_id).await, 0);
    assert_eq!(event_count(&db, "forum.topic.replied").await, 0);

    ModerationService::new(db.clone(), event_bus(db.clone()))
        .approve_reply(
            tenant_id,
            pending.id,
            moderated_topic_id,
            SecurityContext::new(UserRole::Admin, Some(moderator_id)),
        )
        .await
        .expect("moderator should publish pending reply");
    assert_eq!(topic_reply_count(&db, moderated_topic_id).await, 1);
    assert_eq!(category_reply_count(&db, category_id).await, 1);
    assert_eq!(event_count(&db, "forum.topic.replied").await, 1);

    service
        .delete(tenant_id, pending.id, owner)
        .await
        .expect("owner should soft-delete reply explicitly");
    assert_eq!(reply_status(&db, pending.id).await, "deleted");
    assert!(reply_deleted(&db, pending.id).await);
    assert_eq!(reply_body(&db, pending.id).await, "[deleted]");
    assert_eq!(reply_revision_count(&db, pending.id).await, 1);
    assert_eq!(topic_reply_count(&db, moderated_topic_id).await, 0);
    assert_eq!(category_reply_count(&db, category_id).await, 0);
}

#[tokio::test]
async fn owner_topic_delete_redacts_thread_and_preserves_revisions() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, false).await;
    seed_topic(
        &db,
        tenant_id,
        category_id,
        topic_id,
        author_id,
        false,
    )
    .await;

    let owner = SecurityContext::new(UserRole::Manager, Some(author_id));
    let reply = ReplyService::new(db.clone(), event_bus(db.clone()))
        .create(
            tenant_id,
            owner.clone(),
            topic_id,
            reply_input("public reply"),
        )
        .await
        .expect("public reply should be created");
    assert_eq!(reply.status, "approved");
    assert_eq!(topic_reply_count(&db, topic_id).await, 1);

    TopicService::new(db.clone(), event_bus(db.clone()))
        .delete(tenant_id, topic_id, owner)
        .await
        .expect("owner should explicitly soft-delete topic thread");

    assert!(topic_deleted(&db, topic_id).await);
    assert_eq!(topic_status(&db, topic_id).await, "archived");
    assert_eq!(topic_title(&db, topic_id).await, "[deleted]");
    assert_eq!(topic_body(&db, topic_id).await, "[deleted]");
    assert_eq!(reply_status(&db, reply.id).await, "deleted");
    assert!(reply_deleted(&db, reply.id).await);
    assert_eq!(reply_body(&db, reply.id).await, "[deleted]");
    assert_eq!(topic_revision_count(&db, topic_id).await, 1);
    assert_eq!(reply_revision_count(&db, reply.id).await, 1);
    assert_eq!(category_topic_count(&db, category_id).await, 0);
    assert_eq!(category_reply_count(&db, category_id).await, 0);
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

async fn setup_db() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:forum_owner_lifecycle_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum owner lifecycle database should connect");
    let manager = SchemaManager::new(&db);

    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("forum migration should apply");
    }

    db
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    moderated: bool,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories \
         (id, tenant_id, position, moderated, topic_count, reply_count) \
         VALUES ('{category_id}', '{tenant_id}', 0, {}, 0, 0)",
        if moderated { 1 } else { 0 }
    ))
    .await
    .expect("category seed should succeed");
}

async fn seed_topic(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    author_id: Uuid,
    locked: bool,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, author_id, status, metadata, is_pinned, is_locked, reply_count) \
         VALUES ('{topic_id}', '{tenant_id}', '{category_id}', '{author_id}', 'open', '{{}}', 0, {}, 0); \
         INSERT INTO forum_topic_translations \
         (id, topic_id, tenant_id, locale, title, slug, body, body_format) \
         VALUES ('{}', '{topic_id}', '{tenant_id}', 'en', 'Topic', 'topic-{topic_id}', 'Topic body', 'markdown')",
        if locked { 1 } else { 0 },
        Uuid::new_v4(),
    ))
    .await
    .expect("topic seed should succeed");

    db.execute_unprepared(&format!(
        "UPDATE forum_categories SET topic_count = 1 WHERE id = '{category_id}'"
    ))
    .await
    .expect("category topic count seed should succeed");
}

async fn scalar_i64(db: &DatabaseConnection, sql: String) -> i64 {
    db.query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should decode")
}

async fn scalar_string(db: &DatabaseConnection, sql: String) -> String {
    db.query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should decode")
}

async fn topic_reply_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!("SELECT reply_count AS value FROM forum_topics WHERE id = '{topic_id}'"),
    )
    .await
}

async fn category_topic_count(db: &DatabaseConnection, category_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!("SELECT topic_count AS value FROM forum_categories WHERE id = '{category_id}'"),
    )
    .await
}

async fn category_reply_count(db: &DatabaseConnection, category_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!("SELECT reply_count AS value FROM forum_categories WHERE id = '{category_id}'"),
    )
    .await
}

async fn event_count(db: &DatabaseConnection, event_type: &str) -> i64 {
    scalar_i64(
        db,
        format!("SELECT COUNT(*) AS value FROM sys_events WHERE event_type = '{event_type}'"),
    )
    .await
}

async fn reply_status(db: &DatabaseConnection, reply_id: Uuid) -> String {
    scalar_string(
        db,
        format!("SELECT status AS value FROM forum_replies WHERE id = '{reply_id}'"),
    )
    .await
}

async fn topic_status(db: &DatabaseConnection, topic_id: Uuid) -> String {
    scalar_string(
        db,
        format!("SELECT status AS value FROM forum_topics WHERE id = '{topic_id}'"),
    )
    .await
}

async fn reply_body(db: &DatabaseConnection, reply_id: Uuid) -> String {
    scalar_string(
        db,
        format!("SELECT body AS value FROM forum_reply_bodies WHERE reply_id = '{reply_id}'"),
    )
    .await
}

async fn topic_title(db: &DatabaseConnection, topic_id: Uuid) -> String {
    scalar_string(
        db,
        format!("SELECT title AS value FROM forum_topic_translations WHERE topic_id = '{topic_id}'"),
    )
    .await
}

async fn topic_body(db: &DatabaseConnection, topic_id: Uuid) -> String {
    scalar_string(
        db,
        format!("SELECT body AS value FROM forum_topic_translations WHERE topic_id = '{topic_id}'"),
    )
    .await
}

async fn reply_deleted(db: &DatabaseConnection, reply_id: Uuid) -> bool {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_replies WHERE id = '{reply_id}' AND deleted_at IS NOT NULL"
        ),
    )
    .await
        == 1
}

async fn topic_deleted(db: &DatabaseConnection, topic_id: Uuid) -> bool {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_topics WHERE id = '{topic_id}' AND deleted_at IS NOT NULL"
        ),
    )
    .await
        == 1
}

async fn reply_revision_count(db: &DatabaseConnection, reply_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_reply_revisions WHERE reply_id = '{reply_id}'"
        ),
    )
    .await
}

async fn topic_revision_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_topic_revisions WHERE topic_id = '{topic_id}'"
        ),
    )
    .await
}
