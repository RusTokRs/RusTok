use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    HostRuntimeContext, Permission, PortActor, PortCallPolicy, PortContext, PortError,
};
use rustok_core::{MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::entities::forum_domain_event;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumModule, ForumNotificationRecipientContextPort,
    ForumNotificationRecipientContextRequest, ModerationService,
    SetForumCategoryAudiencePolicyInput, SharedForumNotificationRecipientContextPort,
    SubscriptionService, TopicService,
};
use rustok_notifications::NotificationsModule;
use rustok_notifications_api::{
    DescribeNotificationRequest, NotificationSourceEventRef, ResolveNotificationAudienceRequest,
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

#[derive(Clone)]
struct StaticRecipientContextPort {
    roles: BTreeMap<Uuid, &'static str>,
}

#[async_trait]
impl ForumNotificationRecipientContextPort for StaticRecipientContextPort {
    async fn resolve_forum_notification_recipient_context(
        &self,
        context: PortContext,
        request: ForumNotificationRecipientContextRequest,
    ) -> Result<PortContext, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if context.tenant_id != request.tenant_id.to_string() {
            return Err(PortError::validation(
                "forum.notification_descriptor.test_tenant_mismatch",
                "Forum notification descriptor recipient tenant does not match",
            ));
        }
        let Some(role) = self.roles.get(&request.recipient_id) else {
            return Err(PortError::not_found(
                "forum.notification_descriptor.test_recipient_missing",
                "Forum notification descriptor recipient is unavailable",
            ));
        };
        let mut recipient = PortContext::new(
            request.tenant_id.to_string(),
            PortActor::user(request.recipient_id.to_string()),
            context.locale.clone(),
            context.correlation_id.clone(),
        )
        .with_role(*role)
        .with_claim(Permission::FORUM_TOPICS_READ.to_string());
        recipient.causation_id = context.causation_id;
        recipient.traceparent = context.traceparent;
        recipient.deadline_ms = context.deadline_ms;
        Ok(recipient)
    }
}

#[tokio::test]
async fn initially_non_public_topic_descriptor_requires_recipient_capability_and_reauthorizes() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let allowed_recipient = Uuid::from_u128(1);
    let denied_recipient = Uuid::from_u128(2);
    let admin = SecurityContext::new(UserRole::Admin, Some(author_id));

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Initially private notifications".into(),
                slug: "initially-private-notifications".into(),
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

    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            category.id,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("category should be non-public before topic creation");

    let subscriptions = SubscriptionService::new(db.clone());
    for recipient_id in [allowed_recipient, denied_recipient] {
        subscriptions
            .set_category_subscription(
                tenant_id,
                category.id,
                SecurityContext::new(UserRole::Customer, Some(recipient_id)),
            )
            .await
            .expect("category subscription should persist");
    }

    let topic = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Private from creation".into(),
                slug: Some("private-from-creation".into()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Descriptor creation must not leak this body.",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let event = topic_created_event(&db, tenant_id, topic.id).await;
    let event_ref = source_event_ref(&event);

    let registry = ModuleRegistry::new()
        .register(NotificationsModule)
        .register(ForumModule);
    let mut public_extensions = registry
        .build_runtime_extensions()
        .expect("public fallback extensions should initialize");
    let public_host = public_extensions.apply_to_host_runtime(HostRuntimeContext::new(db.clone()));
    let public_providers =
        materialize_notification_source_registry(&mut public_extensions, &public_host)
            .expect("public fallback source registry should materialize");
    let public_provider = public_providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");
    assert!(
        public_provider
            .describe_event(DescribeNotificationRequest {
                event: event_ref.clone(),
            })
            .await
            .expect("public fallback description should complete")
            .is_none(),
        "without recipient capability an initially non-public topic must remain absent"
    );

    let recipient_port: SharedForumNotificationRecipientContextPort =
        Arc::new(StaticRecipientContextPort {
            roles: BTreeMap::from([
                (allowed_recipient, "customer"),
                (denied_recipient, "moderator"),
            ]),
        });
    let mut recipient_extensions = registry
        .build_runtime_extensions()
        .expect("recipient-aware extensions should initialize");
    recipient_extensions.insert(recipient_port);
    let recipient_host =
        recipient_extensions.apply_to_host_runtime(HostRuntimeContext::new(db.clone()));
    let recipient_providers =
        materialize_notification_source_registry(&mut recipient_extensions, &recipient_host)
            .expect("recipient-aware source registry should materialize");
    let recipient_provider = recipient_providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");

    let descriptor = recipient_provider
        .describe_event(DescribeNotificationRequest {
            event: event_ref.clone(),
        })
        .await
        .expect("recipient-aware descriptor materialization should complete")
        .expect("active initially non-public topic should materialize a descriptor");
    let topic_id = topic.id.to_string();
    let category_id = category.id.to_string();
    assert_eq!(descriptor.target.id, topic.id);
    assert_eq!(descriptor.template_data.len(), 2);
    assert_eq!(
        descriptor.template_data.get("topic_id"),
        Some(topic_id.as_str())
    );
    assert_eq!(
        descriptor.template_data.get("category_id"),
        Some(category_id.as_str())
    );
    for forbidden in ["title", "body", "route", "recipient_id", "audience"] {
        assert!(descriptor.template_data.get(forbidden).is_none());
    }

    let page = recipient_provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref.clone(),
            descriptor: descriptor.clone(),
            cursor: None,
            limit: 10,
        })
        .await
        .expect("exact subscription audience should resolve");
    assert!(page.is_complete());
    assert_eq!(page.recipients().len(), 1);
    assert_eq!(page.recipients()[0].recipient_id, allowed_recipient);

    ModerationService::new(db.clone(), event_bus)
        .close_topic(tenant_id, topic.id, admin)
        .await
        .expect("topic should close");
    assert!(
        recipient_provider
            .describe_event(DescribeNotificationRequest {
                event: event_ref.clone(),
            })
            .await
            .expect("closed descriptor recheck should complete")
            .is_none(),
        "closed initially non-public topic must not materialize a descriptor"
    );
    let closed_page = recipient_provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref,
            descriptor,
            cursor: None,
            limit: 10,
        })
        .await
        .expect("closed stale descriptor should be rechecked");
    assert!(closed_page.recipients().is_empty());
    assert!(closed_page.is_complete());
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
        .expect("topic-created event should exist")
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
        "sqlite:file:forum_notification_topic_descriptor_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification topic descriptor database should connect");
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
