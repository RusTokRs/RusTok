use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ListRepliesFilter, ListTopicsFilter, ModerationService, ReplyService,
    TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_solutions_{}?mode=memory&cache=shared",
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

#[tokio::test]
async fn mark_and_clear_solution_updates_topic_and_reply_read_paths() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let moderation_service = ModerationService::new(db, event_bus);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let manager = SecurityContext::new(UserRole::Manager, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin, false).await;
    let topic = topic_service
        .create(
            tenant_id,
            customer.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Solved topic".to_string(),
                slug: Some("solved-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = reply_service
        .create(
            tenant_id,
            customer.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Accepted answer"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    moderation_service
        .mark_solution(tenant_id, topic.id, reply.id, manager.clone())
        .await
        .expect("manager should mark solution");

    let topic_after_mark = topic_service
        .get(tenant_id, customer.clone(), topic.id, "en")
        .await
        .expect("topic should load");
    assert_eq!(topic_after_mark.solution_reply_id, Some(reply.id));

    let (topics, total) = topic_service
        .list(
            tenant_id,
            customer.clone(),
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await
        .expect("topic list should load");
    assert_eq!(total, 1);
    assert_eq!(topics[0].solution_reply_id, Some(reply.id));

    let reply_after_mark = reply_service
        .get(tenant_id, customer.clone(), reply.id, "en")
        .await
        .expect("reply should load");
    assert!(reply_after_mark.is_solution);

    let (replies, replies_total) = reply_service
        .list_response_for_topic_with_locale_fallback(
            tenant_id,
            customer.clone(),
            topic.id,
            ListRepliesFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
            Some("en"),
        )
        .await
        .expect("reply list should load");
    assert_eq!(replies_total, 1);
    assert!(replies[0].is_solution);

    moderation_service
        .clear_solution(tenant_id, topic.id, manager)
        .await
        .expect("manager should clear solution");

    let topic_after_clear = topic_service
        .get(tenant_id, customer.clone(), topic.id, "en")
        .await
        .expect("topic should load after clear");
    assert_eq!(topic_after_clear.solution_reply_id, None);

    let reply_after_clear = reply_service
        .get(tenant_id, customer, reply.id, "en")
        .await
        .expect("reply should load after clear");
    assert!(!reply_after_clear.is_solution);
}

#[tokio::test]
async fn pending_reply_cannot_be_marked_as_solution() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let moderation_service = ModerationService::new(db, event_bus);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let manager = SecurityContext::new(UserRole::Manager, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin, true).await;
    let topic = topic_service
        .create(
            tenant_id,
            customer.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Pending reply topic".to_string(),
                slug: Some("pending-reply-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = reply_service
        .create(
            tenant_id,
            customer,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Pending reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let err = moderation_service
        .mark_solution(tenant_id, topic.id, reply.id, manager)
        .await
        .expect_err("pending reply must not become a solution");

    assert!(matches!(err, ForumError::Validation(_)));
}
