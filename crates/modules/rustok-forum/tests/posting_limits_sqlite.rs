//! Tenant length limits, per-author posting rate limits, reply pre-moderation, the author edit
//! window and locked topics in storefront lists on SQLite.
//!
//! Each test writes the Forum settings row, then exercises a user write path. Imports are
//! deliberately not covered here; see `DECISIONS/2026-10-09-forum-content-length-enforcement.md`
//! `DECISIONS/2026-10-09-forum-posting-rate-limits.md`, and
//! `DECISIONS/2026-10-09-forum-wire-author-edit-window-and-locked-list-settings.md`.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot, StaticModuleSettingsTransactionReader,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ListRepliesFilter, ListTopicsFilter, ModerationService, ReplyService,
    ReplyStatus, TopicService, TopicStatus, UpdateReplyInput, UpdateTopicInput,
};
use rustok_forum::entities::{forum_reply, forum_topic};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

struct TestForumSettingsReader {
    db: DatabaseConnection,
}

#[async_trait]
impl StaticModuleSettingsReader for TestForumSettingsReader {
    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        let module = rustok_tenant::entities::tenant_module::Entity::find()
            .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
            .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq(module_slug))
            .one(&self.db)
            .await
            .map_err(|e| PortError::unavailable("database_error", e.to_string()))?;

        if let Some(m) = module {
            return Ok(Some(StaticModuleSettingsSnapshot {
                enabled: m.enabled,
                settings: m.settings,
            }));
        }

        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == "forum",
            settings: serde_json::json!({ "use_reactions": false }),
        }))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for TestForumSettingsReader {
    async fn settings_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        let module = rustok_tenant::entities::tenant_module::Entity::find()
            .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
            .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq(module_slug))
            .one(txn)
            .await
            .map_err(|e| PortError::unavailable("database_error", e.to_string()))?;

        if let Some(m) = module {
            return Ok(Some(StaticModuleSettingsSnapshot {
                enabled: m.enabled,
                settings: m.settings,
            }));
        }

        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == "forum",
            settings: serde_json::json!({ "use_reactions": false }),
        }))
    }
}

fn test_settings_providers(db: DatabaseConnection) -> rustok_forum::ForumSettingsProviders {
    let reader = Arc::new(TestForumSettingsReader { db });
    rustok_forum::ForumSettingsProviders::default().with_static_readers(
        SharedStaticModuleSettingsReader(reader.clone()),
        SharedStaticModuleSettingsTransactionReader(reader),
    )
}

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_votes_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect forum sqlite database")
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db = setup_forum_test_db().await;
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
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

    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenant_modules (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .await
    .expect("tenant modules table fixture should apply");
    let module = ForumModule;
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus, Uuid::new_v4())
}

async fn create_category(
    service: &CategoryService,
    tenant_id: Uuid,
    security: SecurityContext,
    moderated: bool,
) -> rustok_forum::CategoryResponse {
    service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated,
            },
        )
        .await
        .expect("category should be created")
}

async fn set_forum_module_settings(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    settings: serde_json::Value,
) {
    let existing = rustok_tenant::entities::tenant_module::Entity::find()
        .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
        .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq("forum"))
        .one(db)
        .await
        .expect("load forum module");
    match existing {
        Some(row) => {
            let mut active: rustok_tenant::entities::tenant_module::ActiveModel = row.into();
            active.settings = Set(settings);
            active
                .update(db)
                .await
                .expect("forum settings should update");
        }
        None => {
            let now = chrono::Utc::now();
            rustok_tenant::entities::tenant_module::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                module_slug: Set("forum".to_string()),
                enabled: Set(true),
                settings: Set(settings),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await
            .expect("forum settings should be created");
        }
    }
}


fn repeat_char(ch: char, count: usize) -> String {
    ch.to_string().repeat(count)
}

fn topic_input(category_id: Uuid, title: &str, slug: &str, body: &str) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".to_string(),
        category_id,
        title: title.to_string(),
        slug: Some(slug.to_string()),
        body: rustok_api::RichTextDocument::single_paragraph(body),
        metadata: serde_json::json!({}),
        tags: vec![],
        channel_slugs: None,
    }
}

/// A customer. The built-in Customer role holds the topic and reply update permissions, with
/// scope `Own`, so the author may edit the author's own items.
fn author_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(user_id))
}

async fn backdate_topic(db: &DatabaseConnection, topic_id: Uuid, minutes_ago: i64) {
    forum_topic::ActiveModel {
        id: Set(topic_id),
        created_at: Set((Utc::now() - Duration::minutes(minutes_ago)).fixed_offset()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("topic creation time should move into the past");
}

async fn backdate_reply(db: &DatabaseConnection, reply_id: Uuid, minutes_ago: i64) {
    forum_reply::ActiveModel {
        id: Set(reply_id),
        created_at: Set((Utc::now() - Duration::minutes(minutes_ago)).fixed_offset()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("reply creation time should move into the past");
}

async fn lock_topic(db: &DatabaseConnection, topic_id: Uuid) {
    forum_topic::ActiveModel {
        id: Set(topic_id),
        is_locked: Set(true),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("topic should be locked");
}

fn assert_validation<T: std::fmt::Debug>(result: Result<T, ForumError>, what: &str) {
    match result {
        Err(ForumError::Validation(_)) => {}
        other => panic!("{what} should fail with Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn topic_title_and_body_follow_tenant_limits_on_create() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "min_topic_title_length": 3,
            "max_topic_title_length": 10,
            "min_post_body_length": 2,
            "max_post_body_length": 20,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    let too_short = topics
        .create(tenant_id, author.clone(), topic_input(category.id, "ab", "short", "Body"))
        .await;
    assert_validation(too_short, "title below the minimum");

    let too_long = topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(category.id, &repeat_char('a', 11), "long", "Body"),
        )
        .await;
    assert_validation(too_long, "title above the maximum");

    let body_too_short = topics
        .create(tenant_id, author.clone(), topic_input(category.id, "Valid", "body-short", "a"))
        .await;
    assert_validation(body_too_short, "body below the minimum");

    let body_too_long = topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(category.id, "Valid", "body-long", &repeat_char('b', 21)),
        )
        .await;
    assert_validation(body_too_long, "body above the maximum");

    // Surrounding whitespace is not counted, so "  abc  " is three characters.
    topics
        .create(tenant_id, author, topic_input(category.id, "  abc  ", "trimmed", "ok"))
        .await
        .expect("title at the minimum after trimming and a body at the minimum should be accepted");
}

#[tokio::test]
async fn reply_body_follows_tenant_limits_on_create_and_update() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = author_security(Uuid::new_v4());
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "min_post_body_length": 2,
            "max_post_body_length": 20,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .create(tenant_id, author.clone(), topic_input(category.id, "Topic", "topic", "Body"))
        .await
        .expect("topic should be created with the default title limits");
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    let short_reply = replies
        .create(
            tenant_id,
            author.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("a"),
                parent_reply_id: None,
            },
        )
        .await;
    assert_validation(short_reply, "reply body below the minimum");

    let reply = replies
        .create(
            tenant_id,
            author.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("ok"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply at the minimum should be accepted");

    let long_update = replies
        .update(
            tenant_id,
            reply.id,
            author.clone(),
            UpdateReplyInput {
                locale: "en".to_string(),
                content: Some(rustok_api::RichTextDocument::single_paragraph(
                    &repeat_char('c', 21),
                )),
            },
        )
        .await;
    assert_validation(long_update, "reply body update above the maximum");

    replies
        .update(
            tenant_id,
            reply.id,
            author,
            UpdateReplyInput {
                locale: "en".to_string(),
                content: Some(rustok_api::RichTextDocument::single_paragraph("fixed")),
            },
        )
        .await
        .expect("reply update within the limits should be accepted");
}

#[tokio::test]
async fn topic_update_checks_only_the_title_and_body_it_supplies() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = author_security(Uuid::new_v4());
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "min_topic_title_length": 3,
            "max_topic_title_length": 10,
            "min_post_body_length": 2,
            "max_post_body_length": 20,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let topic = topics
        .create(tenant_id, author.clone(), topic_input(category.id, "Topic", "update-me", "Body"))
        .await
        .expect("topic should be created");

    let update = |title: Option<String>,
                  body: Option<rustok_api::RichTextDocument>| UpdateTopicInput {
        locale: "en".to_string(),
        title,
        body,
        metadata: None,
        tags: None,
        channel_slugs: None,
    };

    let short_title = topics
        .update(tenant_id, topic.id, author.clone(), update(Some("x".to_string()), None))
        .await;
    assert_validation(short_title, "title update below the minimum");

    let long_body = topics
        .update(
            tenant_id,
            topic.id,
            author.clone(),
            update(
                None,
                Some(rustok_api::RichTextDocument::single_paragraph(&repeat_char('d', 21))),
            ),
        )
        .await;
    assert_validation(long_body, "body update above the maximum");

    topics
        .update(tenant_id, topic.id, author, update(Some("Renamed".to_string()), None))
        .await
        .expect("a title update within the limits should be accepted without re-checking the body");
}

#[tokio::test]
async fn zero_maximum_is_unlimited_and_inconsistent_limits_fail() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "max_post_body_length": 0,
            "max_topic_title_length": 0,
            "rate_limit_new_topic_seconds": 0,
        }),
    )
    .await;
    topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(
                category.id,
                &repeat_char('t', 1000),
                "unlimited",
                &repeat_char('b', 5000),
            ),
        )
        .await
        .expect("a zero maximum should not limit the title or body");

    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "min_topic_title_length": 50,
            "max_topic_title_length": 10,
            "rate_limit_new_topic_seconds": 0,
        }),
    )
    .await;
    let inconsistent = topics
        .create(tenant_id, author, topic_input(category.id, "Valid", "inconsistent", "Body"))
        .await;
    assert_validation(inconsistent, "a minimum above the maximum");
}

fn assert_rate_limited<T: std::fmt::Debug>(result: Result<T, ForumError>, what: &str) -> u64 {
    match result {
        Err(ForumError::RateLimited {
            retry_after_seconds,
        }) => retry_after_seconds,
        other => panic!("{what} should be rate limited, got {other:?}"),
    }
}

#[tokio::test]
async fn topic_rate_limit_blocks_the_same_author_within_the_interval() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let other = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "rate_limit_new_topic_seconds": 3600,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    topics
        .create(tenant_id, author.clone(), topic_input(category.id, "First", "first", "Body"))
        .await
        .expect("the first topic should be accepted");

    let retry = assert_rate_limited(
        topics
            .create(tenant_id, author, topic_input(category.id, "Second", "second", "Body"))
            .await,
        "a second topic inside the interval",
    );
    assert!(
        (3591..=3600).contains(&retry),
        "retry should be close to the full interval, got {retry}"
    );

    topics
        .create(tenant_id, other, topic_input(category.id, "Other", "other", "Body"))
        .await
        .expect("another author is not limited by this author's posts");
}

#[tokio::test]
async fn reply_rate_limit_is_independent_of_the_topic_limit() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let other = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 3600,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    let topic = topics
        .create(tenant_id, author.clone(), topic_input(category.id, "Topic", "topic", "Body"))
        .await
        .expect("a disabled topic limit should accept consecutive topics");
    topics
        .create(tenant_id, author.clone(), topic_input(category.id, "Again", "again", "Body"))
        .await
        .expect("a zero topic interval should not limit topic creation");

    let reply_input = |text: &str| CreateReplyInput {
        locale: "en".to_string(),
        content: rustok_api::RichTextDocument::single_paragraph(text),
        parent_reply_id: None,
    };
    replies
        .create(tenant_id, author.clone(), topic.id, reply_input("First reply"))
        .await
        .expect("the first reply should be accepted");

    let retry = assert_rate_limited(
        replies
            .create(tenant_id, author, topic.id, reply_input("Second reply"))
            .await,
        "a second reply inside the interval",
    );
    assert!((3591..=3600).contains(&retry), "got {retry}");

    replies
        .create(tenant_id, other, topic.id, reply_input("Other reply"))
        .await
        .expect("another author is not limited by this author's replies");
}

fn topic_title_update(title: &str) -> UpdateTopicInput {
    UpdateTopicInput {
        locale: "en".to_string(),
        title: Some(title.to_string()),
        body: None,
        metadata: None,
        tags: None,
        channel_slugs: None,
    }
}

fn reply_content_update(text: &str) -> UpdateReplyInput {
    UpdateReplyInput {
        locale: "en".to_string(),
        content: Some(rustok_api::RichTextDocument::single_paragraph(text)),
    }
}

#[tokio::test]
async fn author_edit_window_closes_for_topic_and_reply_updates_but_not_for_moderators() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let manager = SecurityContext::new(UserRole::Manager, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "max_edit_window_minutes": 10,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone(), false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    let author = author_security(Uuid::new_v4());
    let topic = topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(category.id, "Topic", "window-topic", "Body"),
        )
        .await
        .expect("topic should be created");
    let reply = replies
        .create(
            tenant_id,
            author.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("First"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    topics
        .update(tenant_id, topic.id, author.clone(), topic_title_update("Edited inside"))
        .await
        .expect("an author update inside the window should be accepted");
    replies
        .update(tenant_id, reply.id, author.clone(), reply_content_update("Edited inside"))
        .await
        .expect("an author reply update inside the window should be accepted");

    backdate_topic(&db, topic.id, 11).await;
    backdate_reply(&db, reply.id, 11).await;

    assert_validation(
        topics
            .update(tenant_id, topic.id, author.clone(), topic_title_update("Too late"))
            .await,
        "topic update after the author window",
    );
    assert_validation(
        replies
            .update(tenant_id, reply.id, author.clone(), reply_content_update("Too late"))
            .await,
        "reply update after the author window",
    );
    let stored = topics
        .get(tenant_id, admin.clone(), topic.id, "en")
        .await
        .expect("topic should be readable");
    assert_eq!(stored.title, "Edited inside", "a rejected update must write nothing");

    topics
        .update(tenant_id, topic.id, manager.clone(), topic_title_update("Moderated"))
        .await
        .expect("a moderator is not limited by the author window");
    replies
        .update(tenant_id, reply.id, manager, reply_content_update("Moderated"))
        .await
        .expect("a moderator reply update is not limited by the author window");
}

#[tokio::test]
async fn zero_edit_window_leaves_author_edits_open() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "max_edit_window_minutes": 0,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    let author = author_security(Uuid::new_v4());
    let topic = topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(category.id, "Topic", "unlimited-topic", "Body"),
        )
        .await
        .expect("topic should be created");
    let reply = replies
        .create(
            tenant_id,
            author.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("First"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    backdate_topic(&db, topic.id, 30 * 24 * 60).await;
    backdate_reply(&db, reply.id, 30 * 24 * 60).await;

    topics
        .update(tenant_id, topic.id, author.clone(), topic_title_update("Still open"))
        .await
        .expect("a zero window must not close the author's topic edit");
    replies
        .update(tenant_id, reply.id, author, reply_content_update("Still open"))
        .await
        .expect("a zero window must not close the author's reply edit");
}

async fn storefront_ids(
    topics: &TopicService,
    tenant_id: Uuid,
    category_id: Uuid,
    reader: &SecurityContext,
) -> Vec<Uuid> {
    topics
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            reader.clone(),
            ListTopicsFilter {
                category_id: Some(category_id),
                status: None,
                locale: Some("en".to_string()),
                after: None,
                per_page: 20,
            },
            Some("en"),
            None,
        )
        .await
        .expect("storefront topic list should resolve")
        .items
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[tokio::test]
async fn locked_topics_are_hidden_from_storefront_lists_only_when_the_setting_is_off() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));

    let author = author_security(Uuid::new_v4());
    let open_topic = topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(category.id, "Open", "open-topic", "Body"),
        )
        .await
        .expect("open topic should be created");
    let locked_topic = topics
        .create(
            tenant_id,
            author,
            topic_input(category.id, "Locked", "locked-topic", "Body"),
        )
        .await
        .expect("locked topic should be created");
    lock_topic(&db, locked_topic.id).await;

    let reader = SecurityContext::public_read();
    let category_id = category.id;
    let shown_by_default = storefront_ids(&topics, tenant_id, category_id, &reader).await;
    assert!(shown_by_default.contains(&open_topic.id));
    assert!(
        shown_by_default.contains(&locked_topic.id),
        "locked topics are shown while show_locked_topics_in_lists is true"
    );

    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "show_locked_topics_in_lists": false,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;

    let shown_when_hidden = storefront_ids(&topics, tenant_id, category_id, &reader).await;
    assert!(shown_when_hidden.contains(&open_topic.id));
    assert!(
        !shown_when_hidden.contains(&locked_topic.id),
        "locked topics must be hidden from the storefront list when the setting is false"
    );
}

async fn stored_reply_status(db: &DatabaseConnection, reply_id: Uuid) -> ReplyStatus {
    forum_reply::Entity::find_by_id(reply_id)
        .one(db)
        .await
        .expect("reply lookup should succeed")
        .expect("reply row should exist")
        .status
}

#[tokio::test]
async fn pre_moderation_holds_new_replies_until_a_moderator_approves_them() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone(), false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    // Create the topic before pre-moderation is on: a held topic accepts no replies.
    let topic = topics
        .create(
            tenant_id,
            admin.clone(),
            topic_input(category.id, "Held replies", "held-replies", "Body"),
        )
        .await
        .expect("topic should be created");
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "pre_moderation_enabled": true,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;

    let reply = replies
        .create(
            tenant_id,
            author_security(Uuid::new_v4()),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Held"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");
    assert_eq!(
        stored_reply_status(&db, reply.id).await,
        ReplyStatus::Pending,
        "a reply under tenant pre-moderation must be held"
    );

    ModerationService::new(db.clone(), event_bus.clone())
        .approve_reply(tenant_id, reply.id, topic.id, admin)
        .await
        .expect("moderator should approve the held reply");
    assert_eq!(
        stored_reply_status(&db, reply.id).await,
        ReplyStatus::Approved
    );
}

async fn stored_topic_status(db: &DatabaseConnection, topic_id: Uuid) -> TopicStatus {
    forum_topic::Entity::find_by_id(topic_id)
        .one(db)
        .await
        .expect("topic lookup should succeed")
        .expect("topic row should exist")
        .status
}

#[tokio::test]
async fn pre_moderation_holds_new_topics_until_a_moderator_approves_them() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "pre_moderation_enabled": true,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone(), false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let author = author_security(Uuid::new_v4());

    let topic = topics
        .create(
            tenant_id,
            author.clone(),
            topic_input(category.id, "Held topic", "held-topic", "Body"),
        )
        .await
        .expect("topic should be created");
    assert_eq!(
        stored_topic_status(&db, topic.id).await,
        TopicStatus::Pending,
        "a topic under tenant pre-moderation must be held"
    );

    let public = SecurityContext::public_read();
    assert!(
        !storefront_ids(&topics, tenant_id, category.id, &public)
            .await
            .contains(&topic.id),
        "a held topic must not appear in the storefront list"
    );

    let reply = replies
        .create(
            tenant_id,
            author_security(Uuid::new_v4()),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Too early"),
                parent_reply_id: None,
            },
        )
        .await;
    assert!(
        matches!(reply, Err(ForumError::TopicAwaitingModeration)),
        "a held topic must accept no replies"
    );

    ModerationService::new(db.clone(), event_bus.clone())
        .reopen_topic(tenant_id, topic.id, admin)
        .await
        .expect("moderator should approve the held topic");
    assert_eq!(stored_topic_status(&db, topic.id).await, TopicStatus::Open);
    assert!(
        storefront_ids(&topics, tenant_id, category.id, &public)
            .await
            .contains(&topic.id),
        "an approved topic must appear in the storefront list"
    );
}

async fn listed_reply_ids(
    replies: &ReplyService,
    tenant_id: Uuid,
    topic_id: Uuid,
    security: SecurityContext,
) -> Vec<Uuid> {
    replies
        .list_for_topic_with_locale_fallback(
            tenant_id,
            security,
            topic_id,
            ListRepliesFilter {
                locale: Some("en".to_string()),
                after: None,
                per_page: 20,
            },
            Some("en"),
        )
        .await
        .expect("reply list should resolve")
        .items
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[tokio::test]
async fn held_replies_are_listed_only_to_their_author_and_moderators() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone(), false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    // The topic is created while pre-moderation is off, so only the reply is held.
    let topic = topics
        .create(
            tenant_id,
            admin.clone(),
            topic_input(category.id, "Held reply list", "held-reply-list", "Body"),
        )
        .await
        .expect("topic should be created");
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "pre_moderation_enabled": true,
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let author = author_security(Uuid::new_v4());
    let other_customer = author_security(Uuid::new_v4());
    let reply = replies
        .create(
            tenant_id,
            author.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Held answer"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");
    assert_eq!(stored_reply_status(&db, reply.id).await, ReplyStatus::Pending);

    assert!(
        !listed_reply_ids(&replies, tenant_id, topic.id, other_customer)
            .await
            .contains(&reply.id),
        "another author must not see a held reply"
    );
    assert!(
        listed_reply_ids(&replies, tenant_id, topic.id, author)
            .await
            .contains(&reply.id),
        "the author must see their own held reply"
    );
    assert!(
        listed_reply_ids(&replies, tenant_id, topic.id, admin)
            .await
            .contains(&reply.id),
        "a moderator must see held replies"
    );
}

#[tokio::test]
async fn new_replies_are_published_immediately_when_pre_moderation_is_off() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({
            "rate_limit_new_topic_seconds": 0,
            "rate_limit_new_reply_seconds": 0,
        }),
    )
    .await;
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone(), false).await;
    let topics = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let replies = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let topic = topics
        .create(
            tenant_id,
            admin,
            topic_input(category.id, "Open replies", "open-replies", "Body"),
        )
        .await
        .expect("topic should be created");

    let reply = replies
        .create(
            tenant_id,
            author_security(Uuid::new_v4()),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Visible"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");
    assert_eq!(
        stored_reply_status(&db, reply.id).await,
        ReplyStatus::Approved
    );
}
