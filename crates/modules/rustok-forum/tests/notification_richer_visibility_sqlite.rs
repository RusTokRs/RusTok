use std::sync::Arc;

use rustok_api::HostRuntimeContext;
use rustok_core::{MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::entities::forum_domain_event;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumModule, ForumTopicAudiencePolicyService,
    SetForumCategoryAudiencePolicyInput, SetForumTopicAudiencePolicyInput, SubscriptionService,
    TopicService,
};
use rustok_notifications::NotificationsModule;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationOpenAuthorization,
    NotificationSourceEventRef, ResolveNotificationAudienceRequest,
    materialize_notification_source_registry,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn notification_source_rechecks_category_and_topic_richer_visibility() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(author_id));

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Richer notifications".into(),
                slug: "richer-notifications".into(),
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

    SubscriptionService::new(db.clone())
        .set_category_subscription(
            tenant_id,
            category.id,
            SecurityContext::new(UserRole::Customer, Some(recipient_id)),
        )
        .await
        .expect("category watcher should be stored");

    let topic_service = TopicService::new(db.clone(), event_bus);
    let topic_narrowed = topic_service
        .create(
            tenant_id,
            admin.clone(),
            topic_input(category.id, "Topic narrowing", "topic-narrowing"),
        )
        .await
        .expect("topic-narrowing fixture should be created");
    let category_narrowed = topic_service
        .create(
            tenant_id,
            admin.clone(),
            topic_input(category.id, "Category narrowing", "category-narrowing"),
        )
        .await
        .expect("category-narrowing fixture should be created");

    let topic_event = topic_created_event(&db, tenant_id, topic_narrowed.id).await;
    let category_event = topic_created_event(&db, tenant_id, category_narrowed.id).await;
    let topic_source_event = source_event_ref(&topic_event);
    let category_source_event = source_event_ref(&category_event);

    let registry = ModuleRegistry::new()
        .register(NotificationsModule)
        .register(ForumModule);
    let mut extensions = registry
        .build_runtime_extensions()
        .expect("Notifications and Forum runtime extensions should initialize");
    let host = extensions.apply_to_host_runtime(HostRuntimeContext::new(db.clone()));
    let providers = materialize_notification_source_registry(&mut extensions, &host)
        .expect("Forum source factory should materialize");
    let provider = providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");

    let topic_descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: topic_source_event.clone(),
        })
        .await
        .expect("public topic event should be described")
        .expect("public topic should initially be notifiable");
    let category_descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: category_source_event.clone(),
        })
        .await
        .expect("public category topic event should be described")
        .expect("public category topic should initially be notifiable");

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic_narrowed.id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: customer_only(),
            },
        )
        .await
        .expect("topic audience should narrow to authenticated customers");

    assert!(
        provider
            .describe_event(DescribeNotificationRequest {
                event: topic_source_event.clone(),
            })
            .await
            .expect("topic-level richer visibility should fail closed")
            .is_none()
    );
    let topic_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: topic_source_event,
            descriptor: topic_descriptor.clone(),
            cursor: None,
            limit: 10,
        })
        .await
        .expect("stale public descriptor should be rechecked");
    assert!(topic_page.recipients().is_empty());
    assert!(topic_page.is_complete());
    let topic_open = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id,
            target: topic_descriptor.target,
        })
        .await
        .expect("topic open authorization should fail closed");
    assert_eq!(topic_open, NotificationOpenAuthorization::Unavailable);

    let category_open_before_narrowing = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id,
            target: category_descriptor.target.clone(),
        })
        .await
        .expect("public category topic should still authorize");
    assert!(matches!(
        category_open_before_narrowing,
        NotificationOpenAuthorization::Allowed { .. }
    ));

    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            category.id,
            admin,
            SetForumCategoryAudiencePolicyInput {
                constraints: customer_only(),
            },
        )
        .await
        .expect("category audience should narrow to authenticated customers");

    assert!(
        provider
            .describe_event(DescribeNotificationRequest {
                event: category_source_event.clone(),
            })
            .await
            .expect("category-level richer visibility should fail closed")
            .is_none()
    );
    let category_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: category_source_event,
            descriptor: category_descriptor.clone(),
            cursor: None,
            limit: 10,
        })
        .await
        .expect("category policy change should invalidate the old public descriptor");
    assert!(category_page.recipients().is_empty());
    assert!(category_page.is_complete());
    let category_open = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id,
            target: category_descriptor.target,
        })
        .await
        .expect("category-narrowed target authorization should fail closed");
    assert_eq!(category_open, NotificationOpenAuthorization::Unavailable);
}

fn topic_input(category_id: Uuid, title: &str, slug: &str) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".into(),
        category_id,
        title: title.into(),
        slug: Some(slug.into()),
        body: rustok_api::RichTextDocument::single_paragraph(
            "Notification visibility must follow the current exact Forum owner.",
        ),
        metadata: serde_json::json!({}),
        tags: Vec::new(),
        channel_slugs: None,
    }
}

fn customer_only() -> ForumAudienceConstraints {
    ForumAudienceConstraints {
        roles_any: vec![UserRole::Customer],
        ..ForumAudienceConstraints::default()
    }
}

async fn topic_created_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> forum_domain_event::Model {
    forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
        .filter(forum_domain_event::Column::AggregateType.eq("topic"))
        .filter(forum_domain_event::Column::AggregateId.eq(topic_id))
        .filter(forum_domain_event::Column::EventType.eq("forum.topic.created"))
        .one(db)
        .await
        .expect("topic-created event query should succeed")
        .expect("topic-created event should be journaled")
}

fn source_event_ref(event: &forum_domain_event::Model) -> NotificationSourceEventRef {
    NotificationSourceEventRef::new(
        event.tenant_id,
        event.event_id,
        "forum".try_into().expect("source slug"),
        event.event_type.clone().try_into().expect("event type"),
        u64::try_from(event.sequence_no).expect("event sequence should be positive"),
    )
    .expect("source event reference")
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let url = format!(
        "sqlite:file:forum_notification_richer_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification richer visibility sqlite database should connect");
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
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
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus)
}
