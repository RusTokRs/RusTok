use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::Utc;
use rustok_api::HostRuntimeContext;
use rustok_core::{MemoryTransport, MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::entities::{forum_domain_event, forum_relation_revision, forum_user_mention};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule, ModerationService,
    SubscriptionService, TopicService,
};
use rustok_notifications::NotificationsModule;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationOpenAuthorization,
    NotificationProviderError, NotificationSourceEventRef, NotificationSourceSlug,
    NotificationTypeKey, ResolveNotificationAudienceRequest,
    materialize_notification_source_registry, notification_source_factory_registry_from_extensions,
    notification_source_registry_from_extensions,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn forum_topic_and_user_mention_sources_support_notifications_profiles() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let first_recipient = Uuid::new_v4();
    let second_recipient = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(author_id));

    let off_registry = ModuleRegistry::new().register(ForumModule);
    let off_extensions = off_registry
        .build_runtime_extensions()
        .expect("Forum runtime extensions should initialize");
    assert_eq!(
        notification_source_factory_registry_from_extensions(&off_extensions)
            .expect("Forum should publish its source factory")
            .len(),
        1
    );
    assert!(notification_source_registry_from_extensions(&off_extensions).is_none());

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Notifications".into(),
                slug: "notifications".into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created while notifications owner is absent");

    let subscriptions = SubscriptionService::new(db.clone());
    for recipient_id in [first_recipient, second_recipient] {
        subscriptions
            .set_category_subscription(
                tenant_id,
                category.id,
                SecurityContext::new(UserRole::Customer, Some(recipient_id)),
            )
            .await
            .expect("category watcher should be stored");
    }

    let topic = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Provider proof".into(),
                slug: Some("provider-proof".into()),
                body: "The Forum command remains independent from notifications.".into(),
                body_format: "markdown".into(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .expect("topic creation must succeed in notifications-off profile");

    let topic_event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
        .filter(forum_domain_event::Column::EventType.eq("forum.topic.created"))
        .order_by_desc(forum_domain_event::Column::SequenceNo)
        .one(&db)
        .await
        .expect("forum event query should succeed")
        .expect("topic-created event should be journaled");
    let topic_source_event = source_event_ref(&topic_event);

    let on_registry = ModuleRegistry::new()
        .register(NotificationsModule)
        .register(ForumModule);
    let mut on_extensions = on_registry
        .build_runtime_extensions()
        .expect("Notifications and Forum runtime extensions should initialize");
    let host = on_extensions.apply_to_host_runtime(HostRuntimeContext::new(db.clone()));
    let providers = materialize_notification_source_registry(&mut on_extensions, &host)
        .expect("Forum source factory should materialize");
    let provider = providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");
    let supported_types = provider
        .supported_types()
        .into_iter()
        .map(|event_type| event_type.into_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        supported_types,
        BTreeSet::from([
            "forum.mention.user_added".to_string(),
            "forum.topic.created".to_string(),
        ])
    );

    let descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: topic_source_event.clone(),
        })
        .await
        .expect("topic event should be described")
        .expect("open public topic should be notifiable");
    assert_eq!(descriptor.notification_type.as_str(), "forum.topic.created");
    assert_eq!(descriptor.target.id, topic.id);
    let topic_id = topic.id.to_string();
    assert_eq!(
        descriptor.template_data.get("topic_id"),
        Some(topic_id.as_str())
    );

    let first_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: topic_source_event.clone(),
            descriptor: descriptor.clone(),
            cursor: None,
            limit: 1,
        })
        .await
        .expect("first audience page should resolve");
    assert_eq!(first_page.recipients().len(), 1);
    let cursor = first_page
        .next_cursor()
        .cloned()
        .expect("bounded first page should expose a cursor");
    let second_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: topic_source_event.clone(),
            descriptor: descriptor.clone(),
            cursor: Some(cursor),
            limit: 1,
        })
        .await
        .expect("second audience page should resolve");
    assert_eq!(second_page.recipients().len(), 1);
    assert!(second_page.is_complete());

    let recipients = first_page
        .recipients()
        .iter()
        .chain(second_page.recipients())
        .map(|candidate| candidate.recipient_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        recipients,
        BTreeSet::from([first_recipient, second_recipient])
    );
    assert!(!recipients.contains(&author_id));

    let authorization = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id: first_recipient,
            target: descriptor.target.clone(),
        })
        .await
        .expect("target authorization should complete");
    match authorization {
        NotificationOpenAuthorization::Allowed { route } => assert_eq!(
            route.as_str(),
            format!("/modules/forum?category={}&topic={}", category.id, topic.id)
        ),
        NotificationOpenAuthorization::Unavailable => panic!("open topic should be available"),
    }

    let mention_event =
        seed_user_mention_event(&db, tenant_id, author_id, topic.id, first_recipient).await;
    let mention_source_event = source_event_ref(&mention_event);
    let mention_descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: mention_source_event.clone(),
        })
        .await
        .expect("user mention event should be described")
        .expect("visible user mention should be notifiable");
    assert_eq!(
        mention_descriptor.notification_type.as_str(),
        "forum.mention.user_added"
    );
    assert_eq!(mention_descriptor.target.id, topic.id);
    assert_eq!(
        mention_descriptor.template_data.get("source_kind"),
        Some("topic")
    );
    let mention_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: mention_source_event.clone(),
            descriptor: mention_descriptor.clone(),
            cursor: None,
            limit: 1,
        })
        .await
        .expect("user mention audience should resolve");
    assert_eq!(mention_page.recipients().len(), 1);
    assert_eq!(mention_page.recipients()[0].recipient_id, first_recipient);
    assert!(mention_page.is_complete());

    let cross_tenant = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id: Uuid::new_v4(),
            recipient_id: first_recipient,
            target: descriptor.target.clone(),
        })
        .await
        .expect("cross-tenant authorization should fail closed");
    assert_eq!(cross_tenant, NotificationOpenAuthorization::Unavailable);

    ModerationService::new(db.clone(), event_bus)
        .close_topic(tenant_id, topic.id, admin)
        .await
        .expect("topic should close through owner workflow");
    let closed = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id: first_recipient,
            target: descriptor.target,
        })
        .await
        .expect("closed target authorization should fail closed");
    assert_eq!(closed, NotificationOpenAuthorization::Unavailable);
    let closed_mention_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: mention_source_event,
            descriptor: mention_descriptor,
            cursor: None,
            limit: 1,
        })
        .await
        .expect("closed mention source should fail closed without provider error");
    assert!(closed_mention_page.recipients().is_empty());
    assert!(closed_mention_page.is_complete());

    db.execute_unprepared("DROP TABLE forum_domain_events")
        .await
        .expect("test should remove the source journal");
    let error = provider
        .describe_event(DescribeNotificationRequest {
            event: topic_source_event,
        })
        .await
        .expect_err("database failure should be classified");
    assert_eq!(
        error,
        NotificationProviderError::Internal { retryable: true }
    );
}

fn source_event_ref(event: &forum_domain_event::Model) -> NotificationSourceEventRef {
    NotificationSourceEventRef::new(
        event.tenant_id,
        event.event_id,
        NotificationSourceSlug::new("forum").expect("source slug"),
        NotificationTypeKey::new(event.event_type.clone()).expect("event type"),
        u64::try_from(event.sequence_no).expect("event sequence should be positive"),
    )
    .expect("source event reference")
}

async fn seed_user_mention_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    topic_id: Uuid,
    mentioned_user_id: Uuid,
) -> forum_domain_event::Model {
    let revision = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq("topic"))
        .filter(forum_relation_revision::Column::TargetId.eq(topic_id))
        .filter(forum_relation_revision::Column::Locale.eq("en"))
        .order_by_desc(forum_relation_revision::Column::RevisionId)
        .one(db)
        .await
        .expect("relation revision query should succeed")
        .expect("topic relation revision should exist");

    forum_user_mention::ActiveModel {
        tenant_id: Set(tenant_id),
        source_kind: Set("topic".to_string()),
        source_id: Set(topic_id),
        source_locale: Set("en".to_string()),
        source_revision_id: Set(revision.revision_id),
        mentioned_user_id: Set(mentioned_user_id),
        handle_snapshot: Set("member-one".to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("user mention relation should persist");

    forum_domain_event::ActiveModel {
        sequence_no: NotSet,
        event_id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        aggregate_type: Set("topic".to_string()),
        aggregate_id: Set(topic_id),
        event_type: Set("forum.mention.user_added".to_string()),
        schema_version: Set(1),
        actor_id: Set(Some(actor_id)),
        payload: Set(serde_json::json!({
            "source_kind": "topic",
            "source_id": topic_id,
            "source_revision_id": revision.revision_id,
            "source_locale": "en",
            "mentioned_user_id": mentioned_user_id,
        })),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("user mention event should persist")
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let url = format!(
        "sqlite:file:forum_notification_source_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification source sqlite database should connect");
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    (db, event_bus)
}
