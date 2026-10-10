#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;

use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ListTopicsFilter, ModerationService, ReplyService, TopicService, UpdateReplyInput,
    UpdateTopicInput,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
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
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let reply_service = ReplyService::new(db, event_bus)
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let other_customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

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
            other_customer.clone(),
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
        .expect_err("another customer should not update the topic");
    assert!(matches!(denied_topic_update, ForumError::Forbidden(_)));

    topic_service
        .update(
            tenant_id,
            topic.id,
            customer.clone(),
            UpdateTopicInput {
                locale: "en".to_string(),
                title: Some("Edited by author".to_string()),
                body: None,
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("author should update own topic");

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
            other_customer,
            UpdateReplyInput {
                locale: "en".to_string(),
                content: Some(rustok_api::RichTextDocument::single_paragraph("Edited")),
            },
        )
        .await
        .expect_err("another customer should not update the reply");
    assert!(matches!(denied_reply_update, ForumError::Forbidden(_)));

    reply_service
        .update(
            tenant_id,
            reply.id,
            customer.clone(),
            UpdateReplyInput {
                locale: "en".to_string(),
                content: Some(rustok_api::RichTextDocument::single_paragraph("Edited by author")),
            },
        )
        .await
        .expect("author should update own reply");

    // Author deletion is off by default (`allow_user_content_deletion = false`).
    let denied_reply_delete = reply_service
        .delete(tenant_id, reply.id, customer.clone())
        .await
        .expect_err("author deletion should be disabled by default");
    assert!(matches!(denied_reply_delete, ForumError::Forbidden(_)));

    let page = topic_service
        .list(
            tenant_id,
            customer,
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                after: None,
                per_page: 20,
            },
        )
        .await
        .expect("customer list should still work");
    let topics = page.items;
    assert_eq!(topics.len(), 1);
    assert_eq!(topics.len(), 1);
}

#[tokio::test]
async fn moderation_requires_moderate_scope() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
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

async fn create_forum_topic(
    topic_service: &TopicService,
    tenant_id: Uuid,
    category_id: Uuid,
    author: SecurityContext,
    slug: &str,
) -> Uuid {
    topic_service
        .create(
            tenant_id,
            author,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: format!("Topic {slug}"),
                slug: Some(slug.to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

#[tokio::test]
async fn author_topic_deletion_follows_the_tenant_policy_and_staff_are_not_affected() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone()).await;
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let other_customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    // Policy off (default): the author is refused, another customer is refused.
    let closed = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let own_topic =
        create_forum_topic(&closed, tenant_id, category.id, author.clone(), "own-closed").await;
    let denied = closed
        .delete(tenant_id, own_topic, author.clone())
        .await
        .expect_err("author deletion should be disabled by default");
    assert!(matches!(denied, ForumError::Forbidden(_)));
    let denied_other = closed
        .delete(tenant_id, own_topic, other_customer)
        .await
        .expect_err("another customer must never delete the topic");
    assert!(matches!(denied_other, ForumError::Forbidden(_)));

    // Policy on: the author deletes own topic.
    let open = TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(
        posting_cooldown::zero_cooldown_providers_with(
            serde_json::json!({ "allow_user_content_deletion": true }),
        ),
    );
    open.delete(tenant_id, own_topic, author.clone())
        .await
        .expect("author should delete own topic when the policy allows it");

    // Policy off again: the admin still deletes through staff permission.
    let staff_topic =
        create_forum_topic(&closed, tenant_id, category.id, author, "staff-target").await;
    closed
        .delete(tenant_id, staff_topic, admin)
        .await
        .expect("administrators are not limited by the author policy");
}

#[tokio::test]
async fn author_reply_deletion_follows_the_tenant_policy() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone()).await;
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let topic =
        create_forum_topic(&topic_service, tenant_id, category.id, admin, "reply-policy").await;

    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let reply_service = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    let reply = reply_service
        .create(
            tenant_id,
            author.clone(),
            topic,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let denied = reply_service
        .delete(tenant_id, reply.id, author.clone())
        .await
        .expect_err("author reply deletion should be disabled by default");
    assert!(matches!(denied, ForumError::Forbidden(_)));

    let open = ReplyService::new(db, event_bus).with_settings_providers(
        posting_cooldown::zero_cooldown_providers_with(
            serde_json::json!({ "allow_user_content_deletion": true }),
        ),
    );
    open.delete(tenant_id, reply.id, author)
        .await
        .expect("author should delete own reply when the policy allows it");
}

#[tokio::test]
async fn update_only_role_edits_its_own_topic_and_never_deletes_it() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin.clone()).await;
    let author_id = Uuid::new_v4();
    let author = SecurityContext::new(UserRole::Customer, Some(author_id));
    // The tenant allows author deletion, so the refusal below comes from the missing permission.
    let topics = TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(
        posting_cooldown::zero_cooldown_providers_with(
            serde_json::json!({ "allow_user_content_deletion": true }),
        ),
    );
    let topic_id =
        create_forum_topic(&topics, tenant_id, category.id, author, "update-only").await;

    let update_only = SecurityContext::from_permissions(
        UserRole::Customer,
        Some(author_id),
        [Permission::new(Resource::ForumTopics, Action::Update)],
    );
    topics
        .update(
            tenant_id,
            topic_id,
            update_only.clone(),
            UpdateTopicInput {
                locale: "en".to_string(),
                title: Some("Edited by the author".to_string()),
                body: None,
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("a role with Update edits its own topic");

    let denied = topics
        .delete(tenant_id, topic_id, update_only)
        .await
        .expect_err("a role without Delete must not delete its own topic");
    assert!(matches!(denied, ForumError::Forbidden(_)));
}
