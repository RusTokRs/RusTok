#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Schema};
use rustok_api::{AuthContext, Permission, PortActor, PortContext, TenantContext};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumMutation, ForumQuery,
    ListTopicsFilter, SetForumCategoryAudiencePolicyInput, SetForumTopicAudiencePolicyInput,
    SubscriptionService, TopicService, ForumTopicAudiencePolicyService,
    UpdateForumSubscriptionInput,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
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
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
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
        .set_category_subscription(tenant_id, category.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await
        .expect("category subscription should be set");
    subscription_service
        .set_topic_subscription(
            tenant_id,
            topic.id,
            viewer.clone(),
            write_context(tenant_id, &viewer),
        )
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

    let page = topic_service
        .list(
            tenant_id,
            viewer.clone(),
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                after: None,
                per_page: 20,
            },
        )
        .await
        .expect("topic list should load");
    let topics = page.items;
    assert_eq!(topics.len(), 1);
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
        .clear_category_subscription(tenant_id, category.id, viewer.clone(), write_context(tenant_id, &viewer))
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
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
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
        .set_category_subscription(tenant_id, category.id, SecurityContext::system(), write_context(tenant_id, &SecurityContext::system()))
        .await
        .expect_err("subscription without user should fail");
    assert!(matches!(category_err, ForumError::Forbidden(_)));

    let topic_err = subscription_service
        .set_topic_subscription(
            tenant_id,
            topic.id,
            SecurityContext::system(),
            write_context(tenant_id, &SecurityContext::system()),
        )
        .await
        .expect_err("subscription without user should fail");
    assert!(matches!(topic_err, ForumError::Forbidden(_)));
}

#[tokio::test]
async fn category_subscription_reads_and_writes_do_not_disclose_or_change_hidden_categories() {
    let (db, _event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let viewer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Restricted category".to_string(),
                slug: "restricted-category".to_string(),
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
    let subscriptions = SubscriptionService::new(db.clone());
    let policies = ForumCategoryAudiencePolicyService::new(db);
    subscriptions
        .set_category_subscription(tenant_id, category.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await
        .expect("viewer may subscribe while category is public");

    policies
        .set(
            tenant_id,
            category.id,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    allow_user_ids: vec![admin.user_id.expect("admin id")],
                    ..Default::default()
                },
            },
        )
        .await
        .expect("category audience should narrow");

    let absent_id = Uuid::new_v4();
    for id in [category.id, absent_id] {
        assert!(matches!(
            subscriptions.get_category_subscription(tenant_id, id, viewer.clone(), write_context(tenant_id, &viewer)).await,
            Err(ForumError::CategoryNotFound(not_found)) if not_found == id
        ));
        assert!(matches!(
            subscriptions.update_category_subscription(
                tenant_id,
                id,
                viewer.clone(),
                write_context(tenant_id, &viewer),
                UpdateForumSubscriptionInput::watching(),
            ).await,
            Err(ForumError::CategoryNotFound(not_found)) if not_found == id
        ));
        assert!(matches!(
            subscriptions.clear_category_subscription(tenant_id, id, viewer.clone(), write_context(tenant_id, &viewer)).await,
            Err(ForumError::CategoryNotFound(not_found)) if not_found == id
        ));
    }
    assert!(matches!(
        subscriptions.get_category_subscription(
            Uuid::new_v4(), category.id, viewer.clone(), write_context(tenant_id, &viewer)
        ).await,
        Err(ForumError::Validation(_))
    ));

    policies
        .set(
            tenant_id,
            category.id,
            admin,
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints::default(),
            },
        )
        .await
        .expect("category audience should widen again");
    let unchanged = subscriptions
        .get_category_subscription(tenant_id, category.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await
        .expect("original subscription should still be present");
    assert_eq!(unchanged.level, rustok_forum::ForumSubscriptionLevel::Watching);
    assert_eq!(unchanged.revision, 1);
}

#[tokio::test]
async fn topic_subscription_get_hides_denied_and_absent_topics_equally() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let viewer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Subscription owner".to_string(),
                slug: "subscription-owner".to_string(),
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
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Restricted subscription".to_string(),
                slug: Some("restricted-subscription".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let subscriptions = SubscriptionService::new(db.clone());
    assert!(subscriptions
        .get_topic_subscription(tenant_id, topic.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await
        .is_ok());
    subscriptions
        .set_topic_subscription(tenant_id, topic.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await
        .expect("viewer can subscribe before the audience is restricted");
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic.id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    allow_user_ids: vec![Uuid::new_v4()],
                    ..Default::default()
                },
            },
        )
        .await
        .expect("topic audience should narrow");
    for id in [topic.id, Uuid::new_v4()] {
        assert!(matches!(
            subscriptions.get_topic_subscription(tenant_id, id, viewer.clone(), write_context(tenant_id, &viewer)).await,
            Err(ForumError::TopicNotFound(not_found)) if not_found == id
        ));
    }

    db.execute_unprepared(
        "CREATE TABLE tenant_modules (tenant_id TEXT, module_slug TEXT, enabled INTEGER)"
    ).await.expect("GraphQL module enablement table");
    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules VALUES ('{tenant_id}', 'forum', 1)"
    )).await.expect("GraphQL module enablement row");
    let schema = Schema::build(ForumQuery::default(), ForumMutation::default(), EmptySubscription)
        .data(db.clone())
        .data(event_bus)
        .data(TenantContext {
            id: tenant_id, name: "Forum test".into(), slug: "forum".into(),
            domain: None, settings: serde_json::json!({}), default_locale: "en".into(),
            is_active: true,
        })
        .data(AuthContext {
            user_id: viewer.user_id.expect("viewer id"), session_id: Uuid::new_v4(), tenant_id,
            permissions: vec![Permission::FORUM_TOPICS_READ],
            client_id: None, scopes: Vec::new(), grant_type: "user".into(),
        })
        .finish();
    // The canonical clear acknowledges the same outcome for hidden and absent
    // IDs, without returning (or probing for) topic content.
    for id in [topic.id, Uuid::new_v4()] {
        let result = schema
            .execute(format!(
                "mutation {{ clearForumTopicSubscription(tenantId: \"{tenant_id}\", topicId: \"{id}\") }}"
            ))
            .await;
        assert!(
            result.errors.is_empty(),
            "blind clear errors: {:?}",
            result.errors
        );
        let data = result.data.into_json().expect("blind clear data");
        assert_eq!(data["clearForumTopicSubscription"], true);
    }
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(tenant_id, topic.id, admin, SetForumTopicAudiencePolicyInput {
            constraints: ForumAudienceConstraints::default(),
        }).await.expect("restore topic audience to inspect state");
    let cleared = subscriptions
        .get_topic_subscription(tenant_id, topic.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await.expect("read cleared subscription");
    assert!(!cleared.level.is_explicitly_subscribed());
    assert_eq!(cleared.revision, 2, "blind clear updates only the caller's existing subscription");
}

#[tokio::test]
async fn graphql_category_subscription_returns_post_write_flag_and_denies_hidden_targets() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let viewer_id = Uuid::new_v4();
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id, admin.clone(), CreateCategoryInput {
                locale: "en".into(), name: "Visible category".into(),
                slug: "graphql-visible-category".into(), description: None,
                icon: None, color: None, parent_id: None, position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("visible category");
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (tenant_id TEXT, module_slug TEXT, enabled INTEGER)"
    ).await.expect("GraphQL module enablement table");
    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules VALUES ('{tenant_id}', 'forum', 1)"
    )).await.expect("GraphQL module enablement row");

    let schema = Schema::build(ForumQuery::default(), ForumMutation::default(), EmptySubscription)
        .data(db.clone())
        .data(event_bus)
        .data(TenantContext {
            id: tenant_id, name: "Forum test".into(), slug: "forum".into(),
            domain: None, settings: serde_json::json!({}), default_locale: "en".into(),
            is_active: true,
        })
        .data(AuthContext {
            user_id: viewer_id, session_id: Uuid::new_v4(), tenant_id,
            permissions: vec![Permission::FORUM_CATEGORIES_READ],
            client_id: None, scopes: Vec::new(), grant_type: "user".into(),
        })
        .finish();

    for (operation, subscribed) in [
        ("setForumCategorySubscription", true),
        ("clearForumCategorySubscription", false),
    ] {
        let result = schema.execute(format!(
            "mutation {{ {operation}(tenantId: \"{tenant_id}\", categoryId: \"{}\") \
             {{ id name isSubscribed }} }}", category.id,
        )).await;
        assert!(result.errors.is_empty(), "{operation} errors: {:?}", result.errors);
        let data = result.data.into_json().expect("GraphQL mutation data");
        assert_eq!(data[operation]["isSubscribed"], subscribed);
        assert_eq!(data[operation]["name"], "Visible category");
    }

    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(tenant_id, category.id, admin.clone(), SetForumCategoryAudiencePolicyInput {
            constraints: ForumAudienceConstraints {
                allow_user_ids: vec![admin.user_id.expect("admin id")],
                ..Default::default()
            },
        }).await.expect("hide category from viewer");
    let result = schema.execute(format!(
        "mutation {{ setForumCategorySubscription(tenantId: \"{tenant_id}\", \
         categoryId: \"{}\") {{ id name isSubscribed }} }}", category.id,
    )).await;
    assert!(!result.errors.is_empty(), "hidden target must fail before a write");
    assert_eq!(result.data, async_graphql::Value::Null);
    assert!(result.errors[0].message.contains("not found"));
    // Widen the audience just to inspect state; a denied mutation must not
    // have written even if the caller knew the category UUID.
    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(tenant_id, category.id, admin, SetForumCategoryAudiencePolicyInput {
            constraints: ForumAudienceConstraints::default(),
        }).await.expect("reopen category");
    let viewer = SecurityContext::new(UserRole::Customer, Some(viewer_id));
    let stored = SubscriptionService::new(db)
        .get_category_subscription(tenant_id, category.id, viewer.clone(), write_context(tenant_id, &viewer))
        .await.expect("inspected subscription");
    assert!(!stored.level.is_explicitly_subscribed());
    assert_eq!(stored.revision, 2, "only set and clear may update the row");
}

/// Builds the exact user port context used by vote and subscription writes in tests.
fn write_context(tenant_id: Uuid, security: &SecurityContext) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(security.user_id.unwrap_or_else(Uuid::nil).to_string()),
        "en",
        "test-audience-write",
    )
}
