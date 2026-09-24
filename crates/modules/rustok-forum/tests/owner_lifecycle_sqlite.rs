use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CreateReplyInput, ForumError, ForumModule, ForumTopicMergeService, MergeForumTopicInput,
    ModerationService, ReplyService, TopicService,
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
        .delete(tenant_id, pending.id, owner.clone())
        .await
        .expect("owner should soft-delete reply explicitly");
    assert_eq!(reply_status(&db, pending.id).await, "deleted");
    assert!(reply_deleted(&db, pending.id).await);
    assert_eq!(
        reply_body(&db, pending.id).await,
        serde_json::to_string(&reply_input("pending reply").content).unwrap()
    );
    assert_eq!(reply_revision_count(&db, pending.id).await, 1);
    assert_eq!(topic_reply_count(&db, moderated_topic_id).await, 0);
    assert_eq!(category_reply_count(&db, category_id).await, 0);

    let repeated = service
        .delete(tenant_id, pending.id, owner)
        .await
        .expect_err("repeated reply deletion must return a typed error");
    assert!(matches!(repeated, ForumError::ReplyDeleted));
}

#[tokio::test]
async fn owner_topic_delete_redacts_thread_and_preserves_revisions() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, false).await;
    seed_topic(&db, tenant_id, category_id, topic_id, author_id, false).await;

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

    let service = TopicService::new(db.clone(), event_bus(db.clone()));
    service
        .delete(tenant_id, topic_id, owner.clone())
        .await
        .expect("owner should explicitly soft-delete topic thread");

    assert!(topic_deleted(&db, topic_id).await);
    assert_eq!(topic_status(&db, topic_id).await, "archived");
    assert_eq!(topic_title(&db, topic_id).await, "Topic");
    assert_eq!(topic_body(&db, topic_id).await, "Topic body");
    assert_eq!(reply_status(&db, reply.id).await, "deleted");
    assert!(reply_deleted(&db, reply.id).await);
    assert_eq!(
        reply_body(&db, reply.id).await,
        serde_json::to_string(&reply_input("public reply").content).unwrap()
    );
    assert_eq!(topic_revision_count(&db, topic_id).await, 1);
    assert_eq!(reply_revision_count(&db, reply.id).await, 1);
    assert_eq!(category_topic_count(&db, category_id).await, 0);
    assert_eq!(category_reply_count(&db, category_id).await, 0);

    let repeated = service
        .delete(tenant_id, topic_id, owner)
        .await
        .expect_err("repeated topic deletion must return a typed error");
    assert!(matches!(repeated, ForumError::TopicDeleted));
}


#[tokio::test]
async fn owner_topic_restore_rehydrates_closed_locked_solution_thread() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let moderator_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, false).await;
    seed_topic(&db, tenant_id, category_id, topic_id, author_id, false).await;

    let owner = SecurityContext::new(UserRole::Manager, Some(author_id));
    let moderator = SecurityContext::new(UserRole::Admin, Some(moderator_id));
    let reply_service = ReplyService::new(db.clone(), event_bus(db.clone()));
    let moderation_service = ModerationService::new(db.clone(), event_bus(db.clone()));
    let topic_service = TopicService::new(db.clone(), event_bus(db.clone()));

    let reply = reply_service
        .create(
            tenant_id,
            owner.clone(),
            topic_id,
            reply_input("accepted answer"),
        )
        .await
        .expect("reply should be created");
    assert_eq!(reply.status, "approved");

    moderation_service
        .mark_solution(tenant_id, topic_id, reply.id, moderator.clone())
        .await
        .expect("reply should become solution");
    moderation_service
        .close_topic(tenant_id, topic_id, moderator.clone())
        .await
        .expect("topic should be closed before deletion");
    moderation_service
        .lock_topic(tenant_id, topic_id, moderator.clone())
        .await
        .expect("topic should be locked before deletion");

    topic_service
        .delete(tenant_id, topic_id, owner)
        .await
        .expect("topic should be soft-deleted");

    assert!(topic_deleted(&db, topic_id).await);
    assert_eq!(topic_status(&db, topic_id).await, "archived");
    assert!(topic_locked(&db, topic_id).await);
    assert_eq!(reply_status(&db, reply.id).await, "deleted");
    assert_eq!(solution_count(&db, topic_id).await, 0);
    assert_eq!(category_topic_count(&db, category_id).await, 0);
    assert_eq!(category_reply_count(&db, category_id).await, 0);

    topic_service
        .restore(tenant_id, topic_id, moderator)
        .await
        .expect("topic should restore from delete snapshot");

    assert!(!topic_deleted(&db, topic_id).await);
    assert_eq!(topic_status(&db, topic_id).await, "closed");
    assert!(topic_locked(&db, topic_id).await);
    assert_eq!(reply_status(&db, reply.id).await, "approved");
    assert_eq!(topic_reply_count(&db, topic_id).await, 1);
    assert_eq!(category_topic_count(&db, category_id).await, 1);
    assert_eq!(category_reply_count(&db, category_id).await, 1);
    assert_eq!(solution_count(&db, topic_id).await, 1);
    assert_eq!(topic_snapshot_count(&db, topic_id).await, 0);
    assert_eq!(reply_snapshot_count(&db, topic_id).await, 0);
}

#[tokio::test]
async fn owner_topic_restore_rejects_merged_source_topic() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let target_topic_id = Uuid::new_v4();
    let source_topic_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, false).await;
    seed_topic(
        &db,
        tenant_id,
        category_id,
        target_topic_id,
        actor_id,
        false,
    )
    .await;
    seed_topic(
        &db,
        tenant_id,
        category_id,
        source_topic_id,
        actor_id,
        false,
    )
    .await;

    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    ForumTopicMergeService::new(db.clone(), event_bus(db.clone()))
        .merge_topic(
            tenant_id,
            target_topic_id,
            admin.clone(),
            MergeForumTopicInput {
                operation_id,
                source_topic_id,
                reason: "Duplicate topic".to_string(),
            },
        )
        .await
        .expect("source topic should merge into target");

    assert_eq!(topic_status(&db, source_topic_id).await, "archived");
    assert!(!topic_deleted(&db, source_topic_id).await);

    TopicService::new(db.clone(), event_bus(db.clone()))
        .delete(tenant_id, source_topic_id, admin.clone())
        .await
        .expect("merged source topic should be explicitly soft-deletable");

    let restore = TopicService::new(db.clone(), event_bus(db.clone()))
        .restore(tenant_id, source_topic_id, admin)
        .await
        .expect_err("merged source topic must never be restored");

    assert!(matches!(
        restore,
        ForumError::TopicRestoreUnavailable(id) if id == source_topic_id
    ));
    assert!(topic_deleted(&db, source_topic_id).await);
}

fn reply_input(content: &str) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".to_string(),
        content: rustok_api::RichTextDocument::single_paragraph(content),
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
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");
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

fn sql_uuid(id: Uuid) -> String {
    format!("X'{}'", id.simple().to_string().to_uppercase())
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    moderated: bool,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO taxonomy_terms \
         (id, tenant_id, kind, scope_type, scope_value, canonical_key, revision) \
         VALUES ({}, {}, 'category', 'module', 'forum', 'category-{}', 1); \
         INSERT INTO forum_categories \
         (id, tenant_id, moderated, topic_count, reply_count) \
         VALUES ({}, {}, {}, 0, 0); \
         INSERT INTO taxonomy_category_hierarchy \
         (tenant_id, term_id, parent_term_id, position) \
         VALUES ({}, {}, NULL, 0)",
        sql_uuid(category_id),
        sql_uuid(tenant_id),
        category_id,
        sql_uuid(category_id),
        sql_uuid(tenant_id),
        if moderated { 1 } else { 0 },
        sql_uuid(tenant_id),
        sql_uuid(category_id)
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
         VALUES ({}, {}, {}, {}, 'open', '{{}}', 0, {}, 0); \
         INSERT INTO forum_topic_translations \
         (id, topic_id, tenant_id, locale, title, slug, body) \
         VALUES ({}, {}, {}, 'en', 'Topic', 'topic-{topic_id}', 'Topic body')",
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
        sql_uuid(category_id),
        sql_uuid(author_id),
        if locked { 1 } else { 0 },
        sql_uuid(Uuid::new_v4()),
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
    ))
    .await
    .expect("topic seed should succeed");

    db.execute_unprepared(&format!(
        "UPDATE forum_categories SET topic_count = 1 WHERE id = {}",
        sql_uuid(category_id)
    ))
    .await
    .expect("category topic count seed should succeed");
}

async fn scalar_i64(db: &DatabaseConnection, sql: String) -> i64 {
    db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should decode")
}

async fn scalar_string(db: &DatabaseConnection, sql: String) -> String {
    db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should decode")
}

async fn topic_reply_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT reply_count AS value FROM forum_topics WHERE id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn category_topic_count(db: &DatabaseConnection, category_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT topic_count AS value FROM forum_categories WHERE id = {}",
            sql_uuid(category_id)
        ),
    )
    .await
}

async fn category_reply_count(db: &DatabaseConnection, category_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT reply_count AS value FROM forum_categories WHERE id = {}",
            sql_uuid(category_id)
        ),
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
        format!(
            "SELECT status AS value FROM forum_replies WHERE id = {}",
            sql_uuid(reply_id)
        ),
    )
    .await
}


async fn topic_locked(db: &DatabaseConnection, topic_id: Uuid) -> bool {
    scalar_i64(
        db,
        format!(
            "SELECT is_locked AS value FROM forum_topics WHERE id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
    == 1
}

async fn solution_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_solutions WHERE topic_id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn topic_snapshot_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_topic_delete_snapshots WHERE topic_id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn reply_snapshot_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_topic_reply_delete_snapshots WHERE topic_id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn topic_status(db: &DatabaseConnection, topic_id: Uuid) -> String {
    scalar_string(
        db,
        format!(
            "SELECT status AS value FROM forum_topics WHERE id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn reply_body(db: &DatabaseConnection, reply_id: Uuid) -> String {
    scalar_string(
        db,
        format!(
            "SELECT body AS value FROM forum_reply_bodies WHERE reply_id = {}",
            sql_uuid(reply_id)
        ),
    )
    .await
}

async fn topic_title(db: &DatabaseConnection, topic_id: Uuid) -> String {
    scalar_string(
        db,
        format!(
            "SELECT title AS value FROM forum_topic_translations WHERE topic_id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn topic_body(db: &DatabaseConnection, topic_id: Uuid) -> String {
    scalar_string(
        db,
        format!(
            "SELECT body AS value FROM forum_topic_translations WHERE topic_id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}

async fn reply_deleted(db: &DatabaseConnection, reply_id: Uuid) -> bool {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_replies WHERE id = {} AND deleted_at IS NOT NULL",
            sql_uuid(reply_id)
        ),
    )
    .await
        == 1
}

async fn topic_deleted(db: &DatabaseConnection, topic_id: Uuid) -> bool {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_topics WHERE id = {} AND deleted_at IS NOT NULL",
            sql_uuid(topic_id)
        ),
    )
    .await
        == 1
}

async fn reply_revision_count(db: &DatabaseConnection, reply_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_reply_revisions WHERE reply_id = {}",
            sql_uuid(reply_id)
        ),
    )
    .await
}

async fn topic_revision_count(db: &DatabaseConnection, topic_id: Uuid) -> i64 {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*) AS value FROM forum_topic_revisions WHERE topic_id = {}",
            sql_uuid(topic_id)
        ),
    )
    .await
}
