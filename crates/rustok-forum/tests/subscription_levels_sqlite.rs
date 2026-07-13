use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::entities::forum_domain_event;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumDigestMode,
    ForumModule, ForumSubscriptionLevel, ReplyService, SubscriptionService, TopicService,
    UpdateForumSubscriptionInput, UpdateForumSubscriptionPolicyInput,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_subscription_levels_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum subscription sqlite database should connect");
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    (db, event_bus, Uuid::new_v4())
}

#[tokio::test]
async fn subscription_levels_policy_auto_subscribe_and_events_are_consistent() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus);
    let subscriptions = SubscriptionService::new(db.clone());

    let author_id = Uuid::new_v4();
    let participant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(author_id));
    let participant = SecurityContext::new(UserRole::Customer, Some(participant_id));

    let category = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "General".into(),
                slug: "general".into(),
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
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Subscription levels".into(),
                slug: Some("subscription-levels".into()),
                body: "Body".into(),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    let author_settings = subscriptions
        .get_topic_subscription(tenant_id, topic.id, admin.clone())
        .await
        .expect("author settings should load");
    assert_eq!(author_settings.level, ForumSubscriptionLevel::Watching);
    assert!(author_settings.explicit);
    assert_eq!(author_settings.revision, 1);

    let muted = subscriptions
        .update_topic_subscription(
            tenant_id,
            topic.id,
            admin.clone(),
            UpdateForumSubscriptionInput {
                level: ForumSubscriptionLevel::Muted,
                notify_mentions: Some(true),
                notify_replies: Some(true),
                notify_new_topics: Some(true),
                digest_mode: Some(ForumDigestMode::Daily),
                expected_revision: Some(author_settings.revision),
            },
        )
        .await
        .expect("explicit mute should be stored");
    assert_eq!(muted.level, ForumSubscriptionLevel::Muted);
    assert!(!muted.notify_mentions);
    assert!(!muted.notify_replies);
    assert!(!muted.notify_new_topics);
    assert_eq!(muted.digest_mode, ForumDigestMode::Disabled);
    assert_eq!(muted.revision, 2);

    let stale = subscriptions
        .update_topic_subscription(
            tenant_id,
            topic.id,
            admin.clone(),
            UpdateForumSubscriptionInput {
                level: ForumSubscriptionLevel::Watching,
                notify_mentions: None,
                notify_replies: None,
                notify_new_topics: None,
                digest_mode: None,
                expected_revision: Some(1),
            },
        )
        .await;
    assert!(stale.is_err(), "stale revision must be rejected");

    reply_service
        .create(
            tenant_id,
            participant.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".into(),
                content: "Participating".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let participant_settings = subscriptions
        .get_topic_subscription(tenant_id, topic.id, participant.clone())
        .await
        .expect("participant settings should load");
    assert_eq!(participant_settings.level, ForumSubscriptionLevel::Tracking);
    assert!(participant_settings.explicit);

    reply_service
        .create(
            tenant_id,
            admin.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".into(),
                content: "Muted author reply".into(),
                content_format: "markdown".into(),
                content_json: None,
                parent_reply_id: None,
            },
        )
        .await
        .expect("muted author reply should be created");
    let still_muted = subscriptions
        .get_topic_subscription(tenant_id, topic.id, admin.clone())
        .await
        .expect("muted settings should remain");
    assert_eq!(still_muted.level, ForumSubscriptionLevel::Muted);
    assert_eq!(still_muted.revision, 2);

    let default_policy = subscriptions
        .get_policy(tenant_id, admin.clone())
        .await
        .expect("default policy should load");
    assert!(!default_policy.explicit);
    assert_eq!(default_policy.revision, 0);
    assert_eq!(
        default_policy.reply_participant_level,
        ForumSubscriptionLevel::Tracking
    );

    let policy = subscriptions
        .update_policy(
            tenant_id,
            admin.clone(),
            UpdateForumSubscriptionPolicyInput {
                auto_subscribe_topic_authors: true,
                topic_author_level: ForumSubscriptionLevel::Tracking,
                auto_subscribe_reply_participants: false,
                reply_participant_level: ForumSubscriptionLevel::Tracking,
                expected_revision: Some(0),
            },
        )
        .await
        .expect("policy should update");
    assert!(policy.explicit);
    assert_eq!(policy.revision, 1);
    assert!(!policy.auto_subscribe_reply_participants);

    subscriptions
        .clear_topic_subscription(tenant_id, topic.id, participant.clone())
        .await
        .expect("clear should restore normal");
    let normal = subscriptions
        .get_topic_subscription(tenant_id, topic.id, participant)
        .await
        .expect("normal settings should load");
    assert_eq!(normal.level, ForumSubscriptionLevel::Normal);
    assert_eq!(normal.revision, 2);
    assert!(normal.explicit);

    let events = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
        .filter(forum_domain_event::Column::EventType.eq("forum.subscription.changed.v1"))
        .order_by_asc(forum_domain_event::Column::SequenceNo)
        .all(&db)
        .await
        .expect("subscription events should load");
    assert!(events.len() >= 4, "auto, mute, participant and clear events expected");
    let topic_id_text = topic.id.to_string();
    let author_id_text = author_id.to_string();
    let mute_event = events
        .iter()
        .find(|event| {
            event.payload["target_id"].as_str() == Some(topic_id_text.as_str())
                && event.payload["user_id"].as_str() == Some(author_id_text.as_str())
                && event.payload["level"].as_str() == Some("muted")
        })
        .expect("mute event should be present");
    assert_eq!(mute_event.schema_version, 1);
    assert_eq!(mute_event.payload["revision"], 2);
}
