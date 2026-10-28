#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ModerationService, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:forum_publication_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL)",
    )
    .await?;
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    Ok((db, events))
}

fn security(role: UserRole, user: Uuid) -> SecurityContext {
    SecurityContext::new(role, Some(user))
}

async fn category(
    db: &DatabaseConnection,
    tenant: Uuid,
    actor: SecurityContext,
    moderated: bool,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant,
            actor,
            CreateCategoryInput {
                locale: "en".to_owned(),
                name: "Lifecycle category".to_owned(),
                slug: "lifecycle-category".to_owned(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated,
            },
        )
        .await?
        .id)
}

async fn topic(
    db: &DatabaseConnection,
    events: &TransactionalEventBus,
    tenant: Uuid,
    author: SecurityContext,
    category_id: Uuid,
    held: bool,
) -> TestResult<Uuid> {
    let settings = posting_cooldown::zero_cooldown_providers_with(
        serde_json::json!({ "pre_moderation_enabled": held }),
    );
    Ok(TopicService::new(db.clone(), events.clone())
        .with_settings_providers(settings)
        .create(
            tenant,
            author,
            CreateTopicInput {
                locale: "en".to_owned(),
                category_id,
                title: "Publication lifecycle".to_owned(),
                slug: Some("publication-lifecycle".to_owned()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

async fn scalar(db: &DatabaseConnection, sql: String) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await?
        .ok_or_else(|| std::io::Error::other("missing scalar"))?;
    Ok(row.try_get("", "value")?)
}

async fn created_events(db: &DatabaseConnection) -> TestResult<i64> {
    scalar(
        db,
        "SELECT COUNT(*) AS value FROM sys_events WHERE event_type = 'forum.topic.created'"
            .to_owned(),
    )
    .await
}

async fn replied_events(db: &DatabaseConnection) -> TestResult<i64> {
    scalar(
        db,
        "SELECT COUNT(*) AS value FROM sys_events WHERE event_type = 'forum.topic.replied'"
            .to_owned(),
    )
    .await
}

async fn category_count(
    db: &DatabaseConnection,
    tenant: Uuid,
    category_id: Uuid,
    field: &str,
) -> TestResult<i64> {
    assert!(matches!(field, "topic_count" | "reply_count"));
    scalar(
        db,
        format!(
            "SELECT {field} AS value FROM forum_categories WHERE tenant_id = X'{}' AND id = X'{}'",
            hex::encode(tenant.as_bytes()),
            hex::encode(category_id.as_bytes())
        ),
    )
    .await
}

async fn user_stat(
    db: &DatabaseConnection,
    tenant: Uuid,
    user: Uuid,
    field: &str,
) -> TestResult<i64> {
    assert!(matches!(
        field,
        "topic_count" | "reply_count" | "solution_count"
    ));
    scalar(
        db,
        format!(
            "SELECT {field} AS value FROM forum_user_stats WHERE tenant_id = X'{}' AND user_id = X'{}'",
            hex::encode(tenant.as_bytes()),
            hex::encode(user.as_bytes())
        ),
    )
    .await
}
#[tokio::test]
async fn pending_approval_emits_creation_only_at_first_publication() -> TestResult<()> {
    let (db, events) = setup().await?;
    let tenant = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let author = security(UserRole::Admin, author_id);
    let category_id = category(&db, tenant, author.clone(), false).await?;
    let id = topic(&db, &events, tenant, author.clone(), category_id, true).await?;
    let moderation = ModerationService::new(db.clone(), events);

    assert_eq!(created_events(&db).await?, 0);
    assert_eq!(
        category_count(&db, tenant, category_id, "topic_count").await?,
        1
    );

    moderation.reopen_topic(tenant, id, author.clone()).await?;
    assert_eq!(created_events(&db).await?, 1);

    moderation.archive_topic(tenant, id, author.clone()).await?;
    assert_eq!(
        category_count(&db, tenant, category_id, "topic_count").await?,
        0
    );

    moderation.reopen_topic(tenant, id, author).await?;
    assert_eq!(
        category_count(&db, tenant, category_id, "topic_count").await?,
        1
    );
    assert_eq!(created_events(&db).await?, 1);
    Ok(())
}
#[tokio::test]
async fn archived_thread_releases_and_restores_category_and_author_counters() -> TestResult<()> {
    let (db, events) = setup().await?;
    let tenant = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let reply_author_id = Uuid::new_v4();
    let author = security(UserRole::Admin, author_id);
    let reply_author = security(UserRole::Customer, reply_author_id);
    let category_id = category(&db, tenant, author.clone(), false).await?;
    let topic_id = topic(&db, &events, tenant, author.clone(), category_id, false).await?;
    let reply = ReplyService::new(db.clone(), events.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant,
            reply_author,
            topic_id,
            CreateReplyInput {
                locale: "en".to_owned(),
                content: rustok_api::RichTextDocument::single_paragraph("An answer"),
                parent_reply_id: None,
            },
        )
        .await?;
    let moderation = ModerationService::new(db.clone(), events);
    moderation
        .mark_solution(tenant, topic_id, reply.id, author.clone())
        .await?;

    assert_eq!(created_events(&db).await?, 1);
    assert_eq!(
        category_count(&db, tenant, category_id, "reply_count").await?,
        1
    );
    assert_eq!(
        user_stat(&db, tenant, reply_author_id, "reply_count").await?,
        1
    );
    assert_eq!(
        user_stat(&db, tenant, reply_author_id, "solution_count").await?,
        1
    );

    moderation
        .archive_topic(tenant, topic_id, author.clone())
        .await?;
    // Solution writes are forbidden on archived topics and cannot move
    // statistics while the topic is hidden.
    assert!(
        moderation
            .mark_solution(tenant, topic_id, reply.id, author.clone())
            .await
            .is_err()
    );
    assert!(
        moderation
            .clear_solution(tenant, topic_id, author.clone())
            .await
            .is_err()
    );
    assert_eq!(
        category_count(&db, tenant, category_id, "topic_count").await?,
        0
    );
    assert_eq!(
        category_count(&db, tenant, category_id, "reply_count").await?,
        0
    );
    assert_eq!(user_stat(&db, tenant, author_id, "topic_count").await?, 0);
    assert_eq!(
        user_stat(&db, tenant, reply_author_id, "reply_count").await?,
        0
    );
    assert_eq!(
        user_stat(&db, tenant, reply_author_id, "solution_count").await?,
        0
    );

    moderation.reopen_topic(tenant, topic_id, author).await?;
    assert_eq!(created_events(&db).await?, 1);
    assert_eq!(
        category_count(&db, tenant, category_id, "topic_count").await?,
        1
    );
    assert_eq!(
        category_count(&db, tenant, category_id, "reply_count").await?,
        1
    );
    assert_eq!(user_stat(&db, tenant, author_id, "topic_count").await?, 1);
    assert_eq!(
        user_stat(&db, tenant, reply_author_id, "reply_count").await?,
        1
    );
    assert_eq!(
        user_stat(&db, tenant, reply_author_id, "solution_count").await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn rejected_pending_topic_publishes_once_after_first_archive_restore() -> TestResult<()> {
    let (db, events) = setup().await?;
    let tenant = Uuid::new_v4();
    let admin = security(UserRole::Admin, Uuid::new_v4());
    let category_id = category(&db, tenant, admin.clone(), false).await?;
    let topic_id = topic(&db, &events, tenant, admin.clone(), category_id, true).await?;
    let moderation = ModerationService::new(db.clone(), events);

    moderation
        .archive_topic(tenant, topic_id, admin.clone())
        .await?;
    assert_eq!(created_events(&db).await?, 0);
    moderation
        .reopen_topic(tenant, topic_id, admin.clone())
        .await?;
    assert_eq!(created_events(&db).await?, 1);
    moderation
        .archive_topic(tenant, topic_id, admin.clone())
        .await?;
    moderation.reopen_topic(tenant, topic_id, admin).await?;
    assert_eq!(created_events(&db).await?, 1);
    Ok(())
}
#[tokio::test]
async fn reapproved_reply_does_not_emit_another_first_publication() -> TestResult<()> {
    let (db, events) = setup().await?;
    let tenant = Uuid::new_v4();
    let admin = security(UserRole::Admin, Uuid::new_v4());
    let replier = security(UserRole::Customer, Uuid::new_v4());
    let category_id = category(&db, tenant, admin.clone(), true).await?;
    let topic_id = topic(&db, &events, tenant, admin.clone(), category_id, false).await?;
    let reply = ReplyService::new(db.clone(), events.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant,
            replier,
            topic_id,
            CreateReplyInput {
                locale: "en".to_owned(),
                content: rustok_api::RichTextDocument::single_paragraph("Moderated answer"),
                parent_reply_id: None,
            },
        )
        .await?;
    assert_eq!(reply.status, "pending");

    let moderation = ModerationService::new(db.clone(), events);
    assert_eq!(replied_events(&db).await?, 0);
    moderation
        .reject_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    moderation
        .approve_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    assert_eq!(replied_events(&db).await?, 1);
    moderation
        .reject_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    moderation
        .approve_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    assert_eq!(replied_events(&db).await?, 1);
    moderation
        .hide_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    moderation
        .approve_reply(tenant, reply.id, topic_id, admin)
        .await?;
    assert_eq!(replied_events(&db).await?, 1);
    assert_eq!(
        category_count(&db, tenant, category_id, "reply_count").await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn hidden_before_first_approval_notifies_once() -> TestResult<()> {
    let (db, events) = setup().await?;
    let tenant = Uuid::new_v4();
    let admin = security(UserRole::Admin, Uuid::new_v4());
    let replier = security(UserRole::Customer, Uuid::new_v4());
    let category_id = category(&db, tenant, admin.clone(), true).await?;
    let topic_id = topic(&db, &events, tenant, admin.clone(), category_id, false).await?;
    let reply = ReplyService::new(db.clone(), events.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant,
            replier,
            topic_id,
            CreateReplyInput {
                locale: "en".to_owned(),
                content: rustok_api::RichTextDocument::single_paragraph("Held then hidden"),
                parent_reply_id: None,
            },
        )
        .await?;
    assert_eq!(reply.status, "pending");

    let moderation = ModerationService::new(db.clone(), events);
    moderation
        .hide_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    assert_eq!(replied_events(&db).await?, 0);
    moderation
        .approve_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    assert_eq!(replied_events(&db).await?, 1);
    moderation
        .hide_reply(tenant, reply.id, topic_id, admin.clone())
        .await?;
    moderation
        .approve_reply(tenant, reply.id, topic_id, admin)
        .await?;
    assert_eq!(replied_events(&db).await?, 1);
    Ok(())
}
