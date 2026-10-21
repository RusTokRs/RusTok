//! Regression coverage for the owner-facing reply update window and the
//! moderation guards that were hardened together with it (sqlite backend).
//!
//! - deleted replies are immutable to author edits: the delete-side snapshot
//!   stays canonical until a restore, so the update window closes permanently
//!   with a typed `ForumError::ReplyNotFound` instead of mutating archived
//!   content;
//! - reply moderation (`approve_reply`) refuses to publish content into
//!   archived topics: `ForumError::TopicArchived` fails closed instead of
//!   inflating visible counters on a terminal thread;
//! - solution assignment (`mark_solution`) refuses archived topics for the
//!   same reason.

#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CreateReplyInput, ForumError, ForumModule, ModerationService, ReplyService, UpdateReplyInput,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn author_cannot_edit_deleted_reply() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, false).await;
    seed_topic(&db, tenant_id, category_id, topic_id, author_id, false).await;

    let service = ReplyService::new(db.clone(), event_bus(db.clone()))
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let owner = SecurityContext::new(UserRole::Manager, Some(author_id));

    let reply = service
        .create(tenant_id, owner.clone(), topic_id, reply_input("original"))
        .await
        .expect("unmoderated reply should publish");

    service
        .delete(tenant_id, reply.id, owner.clone())
        .await
        .expect("owner should soft-delete reply explicitly");
    assert_eq!(reply_status(&db, reply.id).await, "deleted");

    let error = service
        .update(
            tenant_id,
            reply.id,
            owner,
            UpdateReplyInput {
                locale: "en".to_string(),
                content: Some(rustok_api::RichTextDocument::single_paragraph("edited")),
            },
        )
        .await
        .expect_err("deleted reply must be immutable to author edits");
    assert!(
        matches!(error, ForumError::ReplyNotFound(id) if id == reply.id),
        "deleted reply edit must surface typed ReplyNotFound, got {error:?}"
    );
    assert_eq!(reply_status(&db, reply.id).await, "deleted");
}

#[tokio::test]
async fn moderator_cannot_approve_reply_in_archived_topic() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let moderator_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, true).await;
    seed_topic(&db, tenant_id, category_id, topic_id, author_id, false).await;

    let service = ReplyService::new(db.clone(), event_bus(db.clone()))
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let owner = SecurityContext::new(UserRole::Manager, Some(author_id));
    let admin = SecurityContext::new(UserRole::Admin, Some(moderator_id));

    let pending = service
        .create(tenant_id, owner, topic_id, reply_input("pending reply"))
        .await
        .expect("moderated reply should be stored as pending");
    assert_eq!(reply_status(&db, pending.id).await, "pending");

    ModerationService::new(db.clone(), event_bus(db.clone()))
        .archive_topic(tenant_id, topic_id, admin.clone())
        .await
        .expect("moderator should archive the topic");

    let error = ModerationService::new(db.clone(), event_bus(db.clone()))
        .approve_reply(tenant_id, pending.id, topic_id, admin)
        .await
        .expect_err("archived topic must reject reply approval");
    assert!(
        matches!(error, ForumError::TopicArchived),
        "reply approval in an archived topic must fail closed, got {error:?}"
    );
    assert_eq!(reply_status(&db, pending.id).await, "pending");
}

#[tokio::test]
async fn moderator_cannot_mark_solution_in_archived_topic() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let moderator_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();

    seed_category(&db, tenant_id, category_id, false).await;
    seed_topic(&db, tenant_id, category_id, topic_id, author_id, false).await;

    let service = ReplyService::new(db.clone(), event_bus(db.clone()))
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let owner = SecurityContext::new(UserRole::Manager, Some(author_id));
    let admin = SecurityContext::new(UserRole::Admin, Some(moderator_id));

    let reply = service
        .create(tenant_id, owner, topic_id, reply_input("answer"))
        .await
        .expect("unmoderated reply should publish");

    ModerationService::new(db.clone(), event_bus(db.clone()))
        .archive_topic(tenant_id, topic_id, admin.clone())
        .await
        .expect("moderator should archive the topic");

    let error = ModerationService::new(db.clone(), event_bus(db.clone()))
        .mark_solution(tenant_id, topic_id, reply.id, admin)
        .await
        .expect_err("archived topic must reject solution assignment");
    assert!(
        matches!(error, ForumError::TopicArchived),
        "solution assignment in an archived topic must fail closed, got {error:?}"
    );
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
        "sqlite:file:forum_reply_update_window_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("reply update window database should connect");
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
}

async fn reply_status(db: &DatabaseConnection, reply_id: Uuid) -> String {
    db.query_one_raw(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!(
            "SELECT status AS value FROM forum_replies WHERE id = {}",
            sql_uuid(reply_id)
        ),
    ))
    .await
    .expect("reply status query should execute")
    .expect("reply status query should return a row")
    .try_get("", "value")
    .expect("reply status should decode")
}
