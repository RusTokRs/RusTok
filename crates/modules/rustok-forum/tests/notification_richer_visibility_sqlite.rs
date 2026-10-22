#[path = "support/forum_public_settings.rs"]
mod forum_public_settings;

#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    HostRuntimeContext, PortActor, PortContext, PortError, SharedStaticModuleSettingsReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
};
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
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(recipient_id.to_string()),
                "en",
                "test-category-subscription",
            ),
        )
        .await
        .expect("category watcher should be stored");

    let topic_service = TopicService::new(db.clone(), event_bus)
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
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
    let host = extensions.apply_to_host_runtime(
        HostRuntimeContext::new(db.clone())
            .with_shared_value(forum_public_settings::default_settings_reader()),
    );
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

struct PrivateForumSettings;

#[async_trait]
impl StaticModuleSettingsReader for PrivateForumSettings {
    async fn settings(
        &self,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!(module_slug, "forum");
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: Some(true),
            settings: serde_json::json!({"allow_anonymous_reading": false}),
        }))
    }
}

#[tokio::test]
async fn notification_public_description_and_stale_audience_obey_private_tenant_setting() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let recipient_id = Uuid::new_v4();
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            author.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Privacy switch".into(),
                slug: "privacy-switch".into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category");
    SubscriptionService::new(db.clone())
        .set_category_subscription(
            tenant_id,
            category.id,
            SecurityContext::new(UserRole::Customer, Some(recipient_id)),
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(recipient_id.to_string()),
                "en",
                "private-settings-notification-subscription",
            ),
        )
        .await
        .expect("subscription");
    let topic = TopicService::new(db.clone(), event_bus)
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant_id,
            author,
            topic_input(category.id, "Privacy switch", "privacy-switch-topic"),
        )
        .await
        .expect("topic");
    let event_ref = source_event_ref(&topic_created_event(&db, tenant_id, topic.id).await);

    let registry = ModuleRegistry::new()
        .register(NotificationsModule)
        .register(ForumModule);
    let mut open_extensions = registry.build_runtime_extensions().expect("open registry");
    let open_host = open_extensions.apply_to_host_runtime(
        HostRuntimeContext::new(db.clone())
            .with_shared_value(forum_public_settings::default_settings_reader()),
    );
    let open_registry = materialize_notification_source_registry(&mut open_extensions, &open_host)
        .expect("open source");
    let open_source = open_registry
        .get_by_str("forum")
        .expect("open forum source");
    let descriptor = open_source
        .describe_event(DescribeNotificationRequest {
            event: event_ref.clone(),
        })
        .await
        .expect("public description")
        .expect("previously public topic is describable");
    assert_eq!(
        open_source
            .resolve_audience(ResolveNotificationAudienceRequest {
                event: event_ref.clone(),
                descriptor: descriptor.clone(),
                cursor: None,
                limit: 10,
            })
            .await
            .expect("public audience")
            .recipients()
            .len(),
        1,
    );

    // Simulate a saved settings change without changing the event or its old
    // descriptor. The notification owner must not reuse that descriptor to leak
    // the public audience after the tenant closes anonymous reads.
    let mut closed_extensions = registry
        .build_runtime_extensions()
        .expect("closed registry");
    let closed_host =
        closed_extensions.apply_to_host_runtime(HostRuntimeContext::new(db).with_shared_value(
            SharedStaticModuleSettingsReader(Arc::new(PrivateForumSettings)),
        ));
    let closed_registry =
        materialize_notification_source_registry(&mut closed_extensions, &closed_host)
            .expect("closed source");
    let closed_source = closed_registry
        .get_by_str("forum")
        .expect("closed forum source");
    assert!(
        closed_source
            .describe_event(DescribeNotificationRequest {
                event: event_ref.clone()
            })
            .await
            .expect("private description")
            .is_none()
    );
    let page = closed_source
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref,
            descriptor,
            cursor: None,
            limit: 10,
        })
        .await
        .expect("private audience");
    assert!(page.recipients().is_empty());
    assert!(page.is_complete());
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
