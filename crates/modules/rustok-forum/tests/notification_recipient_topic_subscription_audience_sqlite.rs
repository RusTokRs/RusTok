use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortCallPolicy, PortContext, PortError};
use rustok_core::{MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::entities::forum_domain_event;
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints, ForumModule,
    ForumNotificationRecipientContextPort, ForumNotificationRecipientContextRequest,
    ForumTopicAudiencePolicyService, SetForumTopicAudiencePolicyInput,
    SharedForumNotificationRecipientContextPort, SubscriptionService, TopicService,
};
use rustok_notifications::NotificationsModule;
use rustok_notifications_api::{
    DescribeNotificationRequest, NotificationAudienceCursor, NotificationSourceEventRef,
    ResolveNotificationAudienceRequest, materialize_notification_source_registry,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Clone)]
struct RecordingRecipientContextPort {
    roles: BTreeMap<Uuid, &'static str>,
    calls: Arc<Mutex<Vec<Uuid>>>,
}

#[async_trait]
impl ForumNotificationRecipientContextPort for RecordingRecipientContextPort {
    async fn resolve_forum_notification_recipient_context(
        &self,
        context: PortContext,
        request: ForumNotificationRecipientContextRequest,
    ) -> Result<PortContext, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if context.tenant_id != request.tenant_id.to_string() {
            return Err(PortError::validation(
                "forum.notification_recipient_context.test_tenant_mismatch",
                "Forum notification recipient tenant does not match",
            ));
        }
        self.calls
            .lock()
            .expect("recipient call recorder should stay available")
            .push(request.recipient_id);
        let Some(role) = self.roles.get(&request.recipient_id) else {
            return Err(PortError::not_found(
                "forum.notification_recipient_context.test_recipient_unavailable",
                "Forum notification recipient is unavailable",
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
async fn topic_subscription_audience_filters_exact_recipients_before_cursor_progress() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::from_u128(100);
    let denied_first = Uuid::from_u128(1);
    let unavailable_second = Uuid::from_u128(2);
    let allowed_third = Uuid::from_u128(3);
    let denied_fourth = Uuid::from_u128(4);
    let allowed_fifth = Uuid::from_u128(5);
    let unavailable_cursor = unavailable_second.to_string();
    let denied_cursor = denied_fourth.to_string();
    let admin = SecurityContext::new(UserRole::Admin, Some(author_id));

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Recipient-filtered subscriptions".into(),
                slug: "recipient-filtered-subscriptions".into(),
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
    for recipient_id in [
        denied_first,
        unavailable_second,
        allowed_third,
        denied_fourth,
        allowed_fifth,
    ] {
        subscriptions
            .set_category_subscription(
                tenant_id,
                category.id,
                SecurityContext::new(UserRole::Customer, Some(recipient_id)),
            )
            .await
            .expect("category subscription should persist");
    }

    let topic = TopicService::new(db.clone(), event_bus)
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Bounded recipient filtering".into(),
                slug: Some("bounded-recipient-filtering".into()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Topic-created fanout must scan raw subscriptions without skipping denied recipients.",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
        .filter(forum_domain_event::Column::AggregateType.eq("topic"))
        .filter(forum_domain_event::Column::AggregateId.eq(topic.id))
        .filter(forum_domain_event::Column::EventType.eq("forum.topic.created"))
        .order_by_desc(forum_domain_event::Column::SequenceNo)
        .one(&db)
        .await
        .expect("topic event query should succeed")
        .expect("topic-created event should exist");
    let event_ref = source_event_ref(&event);

    let calls = Arc::new(Mutex::new(Vec::new()));
    let recipient_port: SharedForumNotificationRecipientContextPort =
        Arc::new(RecordingRecipientContextPort {
            roles: BTreeMap::from([
                (denied_first, "customer"),
                (allowed_third, "customer"),
                (denied_fourth, "customer"),
                (allowed_fifth, "customer"),
            ]),
            calls: calls.clone(),
        });
    let registry = ModuleRegistry::new()
        .register(NotificationsModule)
        .register(ForumModule);
    let mut extensions = registry
        .build_runtime_extensions()
        .expect("Notifications and Forum runtime extensions should initialize");
    extensions.insert(recipient_port);
    let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db.clone()));
    let providers = materialize_notification_source_registry(&mut extensions, &host)
        .expect("Forum source factory should consume the recipient capability");
    let provider = providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");

    let descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: event_ref.clone(),
        })
        .await
        .expect("public topic description should complete")
        .expect("public topic should materialize a descriptor");

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic.id,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    deny_user_ids: vec![denied_first, denied_fourth],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic audience should become non-public and deny exact recipients");

    let first_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref.clone(),
            descriptor: descriptor.clone(),
            cursor: None,
            limit: 2,
        })
        .await
        .expect("first exact subscription page should resolve");
    assert!(first_page.recipients().is_empty());
    assert!(!first_page.is_complete());
    assert_eq!(
        first_page
            .next_cursor()
            .map(NotificationAudienceCursor::as_str),
        Some(unavailable_cursor.as_str())
    );
    assert_eq!(
        recorded_calls(&calls),
        vec![denied_first, unavailable_second]
    );

    let second_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref.clone(),
            descriptor: descriptor.clone(),
            cursor: first_page.next_cursor().cloned(),
            limit: 2,
        })
        .await
        .expect("second exact subscription page should resolve");
    assert_eq!(second_page.recipients().len(), 1);
    assert_eq!(second_page.recipients()[0].recipient_id, allowed_third);
    assert!(!second_page.is_complete());
    assert_eq!(
        second_page
            .next_cursor()
            .map(NotificationAudienceCursor::as_str),
        Some(denied_cursor.as_str())
    );
    assert_eq!(
        recorded_calls(&calls),
        vec![
            denied_first,
            unavailable_second,
            allowed_third,
            denied_fourth
        ]
    );

    let third_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref,
            descriptor,
            cursor: second_page.next_cursor().cloned(),
            limit: 2,
        })
        .await
        .expect("terminal exact subscription page should resolve");
    assert_eq!(third_page.recipients().len(), 1);
    assert_eq!(third_page.recipients()[0].recipient_id, allowed_fifth);
    assert!(third_page.is_complete());
    assert_eq!(
        recorded_calls(&calls),
        vec![
            denied_first,
            unavailable_second,
            allowed_third,
            denied_fourth,
            allowed_fifth,
        ]
    );

    let recipients = second_page
        .recipients()
        .iter()
        .chain(third_page.recipients())
        .map(|candidate| candidate.recipient_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(recipients, BTreeSet::from([allowed_third, allowed_fifth]));
}

fn recorded_calls(calls: &Arc<Mutex<Vec<Uuid>>>) -> Vec<Uuid> {
    calls
        .lock()
        .expect("recipient call recorder should stay available")
        .clone()
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
        "sqlite:file:forum_notification_topic_subscription_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification topic subscription database should connect");
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
