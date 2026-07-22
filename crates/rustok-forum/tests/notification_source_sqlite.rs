use std::collections::BTreeSet;
use std::sync::Arc;

use rustok_api::HostRuntimeContext;
use rustok_core::{MemoryTransport, MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::entities::forum_domain_event;
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
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn forum_topic_source_supports_notifications_off_and_on_profiles() {
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

    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
        .filter(forum_domain_event::Column::EventType.eq("forum.topic.created"))
        .order_by_desc(forum_domain_event::Column::SequenceNo)
        .one(&db)
        .await
        .expect("forum event query should succeed")
        .expect("topic-created event should be journaled");
    let revision = u64::try_from(event.sequence_no).expect("event sequence should be positive");
    let source_event = NotificationSourceEventRef::new(
        tenant_id,
        event.event_id,
        NotificationSourceSlug::new("forum").expect("source slug"),
        NotificationTypeKey::new("forum.topic.created").expect("event type"),
        revision,
    )
    .expect("source event reference");

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

    let descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: source_event.clone(),
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
            event: source_event.clone(),
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
            event: source_event.clone(),
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

    db.execute_unprepared("DROP TABLE forum_domain_events")
        .await
        .expect("test should remove the source journal");
    let error = provider
        .describe_event(DescribeNotificationRequest {
            event: source_event,
        })
        .await
        .expect_err("database failure should be classified");
    assert_eq!(
        error,
        NotificationProviderError::Internal { retryable: true }
    );
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
