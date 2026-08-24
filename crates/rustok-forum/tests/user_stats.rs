use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ModerationService, ReplyService, TopicService, UserStatsService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_user_stats_{}?mode=memory&cache=shared",
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

async fn setup() -> (
    DatabaseConnection,
    TransactionalEventBus,
    Uuid,
) {
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

#[tokio::test]
async fn user_stats_track_topic_reply_and_solution_lifecycle() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let moderation_service = ModerationService::new(db.clone(), event_bus.clone());
    let stats_service = UserStatsService::new(db);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let topic_author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let reply_author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let topic = topic_service
        .create(
            tenant_id,
            topic_author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Stats topic".to_string(),
                slug: Some("stats-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    let topic_author_stats = stats_service
        .get(
            tenant_id,
            admin.clone(),
            topic_author.user_id.expect("topic author id"),
        )
        .await
        .expect("topic author stats should load");
    assert_eq!(topic_author_stats.topic_count, 1);
    assert_eq!(topic_author_stats.reply_count, 0);
    assert_eq!(topic_author_stats.solution_count, 0);

    let reply = reply_service
        .create(
            tenant_id,
            reply_author.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Solution"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let reply_author_stats = stats_service
        .get(
            tenant_id,
            admin.clone(),
            reply_author.user_id.expect("reply author id"),
        )
        .await
        .expect("reply author stats should load");
    assert_eq!(reply_author_stats.topic_count, 0);
    assert_eq!(reply_author_stats.reply_count, 1);
    assert_eq!(reply_author_stats.solution_count, 0);

    moderation_service
        .mark_solution(tenant_id, topic.id, reply.id, admin.clone())
        .await
        .expect("solution should be marked");

    let reply_author_after_solution = stats_service
        .get(
            tenant_id,
            admin.clone(),
            reply_author.user_id.expect("reply author id"),
        )
        .await
        .expect("reply author stats after solution should load");
    assert_eq!(reply_author_after_solution.solution_count, 1);

    topic_service
        .delete(tenant_id, topic.id, admin.clone())
        .await
        .expect("topic should be deleted");

    let topic_author_after_delete = stats_service
        .get(
            tenant_id,
            admin.clone(),
            topic_author.user_id.expect("topic author id"),
        )
        .await
        .expect("topic author stats after delete should load");
    assert_eq!(topic_author_after_delete.topic_count, 0);

    let reply_author_after_delete = stats_service
        .get(
            tenant_id,
            admin,
            reply_author.user_id.expect("reply author id"),
        )
        .await
        .expect("reply author stats after delete should load");
    assert_eq!(reply_author_after_delete.reply_count, 0);
    assert_eq!(reply_author_after_delete.solution_count, 0);
}

#[tokio::test]
async fn user_stats_return_zero_state_for_unknown_user() {
    let (db, _event_bus, tenant_id) = setup().await;
    let stats_service = UserStatsService::new(db);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let unknown_user_id = Uuid::new_v4();

    let stats = stats_service
        .get(tenant_id, admin, unknown_user_id)
        .await
        .expect("unknown user stats should still load");

    assert_eq!(stats.user_id, unknown_user_id);
    assert_eq!(stats.topic_count, 0);
    assert_eq!(stats.reply_count, 0);
    assert_eq!(stats.solution_count, 0);
}
