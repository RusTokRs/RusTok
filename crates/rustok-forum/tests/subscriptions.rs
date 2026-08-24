use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumModule,
    ListTopicsFilter, SubscriptionService, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_subscriptions_{}?mode=memory&cache=shared",
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
async fn category_and_topic_subscriptions_round_trip_through_read_paths() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let subscription_service = SubscriptionService::new(db);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let viewer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let other_viewer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

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
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Subscribed topic".to_string(),
                slug: Some("subscribed-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    subscription_service
        .set_category_subscription(tenant_id, category.id, viewer.clone())
        .await
        .expect("category subscription should be set");
    subscription_service
        .set_topic_subscription(tenant_id, topic.id, viewer.clone())
        .await
        .expect("topic subscription should be set");

    let category_after_subscribe = category_service
        .get(tenant_id, viewer.clone(), category.id, "en")
        .await
        .expect("category should load for viewer");
    assert!(category_after_subscribe.is_subscribed);

    let (categories, total_categories) = category_service
        .list_paginated_with_locale_fallback(tenant_id, viewer.clone(), "en", 1, 20, Some("en"))
        .await
        .expect("category list should load");
    assert_eq!(total_categories, 1);
    assert!(categories[0].is_subscribed);

    let topic_after_subscribe = topic_service
        .get(tenant_id, viewer.clone(), topic.id, "en")
        .await
        .expect("topic should load for viewer");
    assert!(topic_after_subscribe.is_subscribed);

    let (topics, total_topics) = topic_service
        .list(
            tenant_id,
            viewer.clone(),
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
    assert_eq!(total_topics, 1);
    assert!(topics[0].is_subscribed);

    let category_for_other = category_service
        .get(tenant_id, other_viewer.clone(), category.id, "en")
        .await
        .expect("category should load for another viewer");
    assert!(!category_for_other.is_subscribed);

    let topic_for_other = topic_service
        .get(tenant_id, other_viewer, topic.id, "en")
        .await
        .expect("topic should load for another viewer");
    assert!(!topic_for_other.is_subscribed);

    subscription_service
        .clear_category_subscription(tenant_id, category.id, viewer.clone())
        .await
        .expect("category subscription should clear");
    subscription_service
        .clear_topic_subscription(tenant_id, topic.id, viewer.clone())
        .await
        .expect("topic subscription should clear");

    let category_after_clear = category_service
        .get(tenant_id, viewer.clone(), category.id, "en")
        .await
        .expect("category should load after clear");
    assert!(!category_after_clear.is_subscribed);

    let topic_after_clear = topic_service
        .get(tenant_id, viewer, topic.id, "en")
        .await
        .expect("topic should load after clear");
    assert!(!topic_after_clear.is_subscribed);
}

#[tokio::test]
async fn subscriptions_require_authenticated_user_context() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let subscription_service = SubscriptionService::new(db);

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
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
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Topic".to_string(),
                slug: Some("topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    let category_err = subscription_service
        .set_category_subscription(tenant_id, category.id, SecurityContext::system())
        .await
        .expect_err("subscription without user should fail");
    assert!(matches!(category_err, ForumError::Forbidden(_)));

    let topic_err = subscription_service
        .set_topic_subscription(tenant_id, topic.id, SecurityContext::system())
        .await
        .expect_err("subscription without user should fail");
    assert!(matches!(topic_err, ForumError::Forbidden(_)));
}
