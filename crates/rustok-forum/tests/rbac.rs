use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ListTopicsFilter, ModerationService, ReplyService, TopicService, UpdateReplyInput,
    UpdateTopicInput,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_rbac_{}?mode=memory&cache=shared",
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
                moderated: false,
            },
        )
        .await
        .expect("category should be created")
}

#[tokio::test]
async fn customer_permissions_are_enforced_in_forum_services() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db, event_bus);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let denied_category = category_service
        .create(
            tenant_id,
            customer.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Denied".to_string(),
                slug: "denied".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect_err("customer should not create categories");
    assert!(matches!(denied_category, ForumError::Forbidden(_)));

    let category = create_category(&category_service, tenant_id, admin.clone()).await;

    let topic = topic_service
        .create(
            tenant_id,
            customer.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Customer topic".to_string(),
                slug: Some("customer-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("customer should create topics");

    let denied_topic_update = topic_service
        .update(
            tenant_id,
            topic.id,
            customer.clone(),
            UpdateTopicInput {
                locale: "en".to_string(),
                title: Some("Edited".to_string()),
                body: None,
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect_err("customer should not update topics");
    assert!(matches!(denied_topic_update, ForumError::Forbidden(_)));

    let reply = reply_service
        .create(
            tenant_id,
            customer.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("customer should create replies");

    let denied_reply_update = reply_service
        .update(
            tenant_id,
            reply.id,
            customer.clone(),
            UpdateReplyInput {
                locale: "en".to_string(),
                content: Some(rustok_api::RichTextDocument::single_paragraph("Edited")),
            },
        )
        .await
        .expect_err("customer should not update replies");
    assert!(matches!(denied_reply_update, ForumError::Forbidden(_)));

    let denied_reply_delete = reply_service
        .delete(tenant_id, reply.id, customer.clone())
        .await
        .expect_err("customer should not delete replies");
    assert!(matches!(denied_reply_delete, ForumError::Forbidden(_)));

    let (topics, total) = topic_service
        .list(
            tenant_id,
            customer,
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await
        .expect("customer list should still work");
    assert_eq!(total, 1);
    assert_eq!(topics.len(), 1);
}

#[tokio::test]
async fn moderation_requires_moderate_scope() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let moderation_service = ModerationService::new(db, event_bus);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let manager = SecurityContext::new(UserRole::Manager, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin.clone()).await;
    let topic = topic_service
        .create(
            tenant_id,
            customer.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Moderated topic".to_string(),
                slug: Some("moderated-topic".to_string()),
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
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let denied = moderation_service
        .hide_reply(
            tenant_id,
            reply.id,
            topic.id,
            SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4())),
        )
        .await
        .expect_err("customer should not moderate replies");
    assert!(matches!(denied, ForumError::Forbidden(_)));

    let denied_solution = moderation_service
        .mark_solution(
            tenant_id,
            topic.id,
            reply.id,
            SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4())),
        )
        .await
        .expect_err("customer should not mark solutions");
    assert!(matches!(denied_solution, ForumError::Forbidden(_)));

    moderation_service
        .mark_solution(
            tenant_id,
            topic.id,
            reply.id,
            SecurityContext::new(UserRole::Manager, Some(Uuid::new_v4())),
        )
        .await
        .expect("manager should mark solutions");

    moderation_service
        .hide_reply(tenant_id, reply.id, topic.id, manager)
        .await
        .expect("manager should moderate replies");
}
