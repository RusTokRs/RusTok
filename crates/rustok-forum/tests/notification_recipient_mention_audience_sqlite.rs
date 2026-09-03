use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{Permission, PortActor, PortCallPolicy, PortContext, PortError};
use rustok_core::{MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::entities::{forum_domain_event, forum_relation_revision, forum_user_mention};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumAudienceConstraints, ForumModule, ForumNotificationRecipientContextPort,
    ForumNotificationRecipientContextRequest, ForumTopicAudiencePolicyService, ReplyService,
    SetForumTopicAudiencePolicyInput, SharedForumNotificationRecipientContextPort, TopicService,
};
use rustok_notifications::NotificationsModule;
use rustok_notifications_api::{
    DescribeNotificationRequest, NotificationSourceEventRef, ResolveNotificationAudienceRequest,
    materialize_notification_source_registry,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Clone)]
struct StaticRecipientContextPort {
    customer_id: Uuid,
    manager_id: Uuid,
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
                "forum.notification_recipient_context.test_tenant_mismatch",
                "Forum notification recipient tenant does not match",
            ));
        }
        let role = if request.recipient_id == self.customer_id {
            "customer"
        } else if request.recipient_id == self.manager_id {
            "manager"
        } else {
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
        .with_role(role)
        .with_claim(Permission::FORUM_TOPICS_READ.to_string())
        .with_claim(Permission::FORUM_REPLIES_READ.to_string());
        recipient.causation_id = context.causation_id;
        recipient.traceparent = context.traceparent;
        recipient.deadline_ms = context.deadline_ms;
        Ok(recipient)
    }
}

#[tokio::test]
async fn mention_description_and_audience_use_the_exact_recipient_for_topics_and_replies() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let manager_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(author_id));

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Exact mention audience".into(),
                slug: "exact-mention-audience".into(),
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
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Recipient-specific mention".into(),
                slug: Some("recipient-specific-mention".into()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Mention notification visibility must follow the exact recipient.",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = ReplyService::new(db.clone(), event_bus)
        .create(
            tenant_id,
            admin.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".into(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply mention target"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic.id,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: role_only(UserRole::Customer),
            },
        )
        .await
        .expect("topic audience should narrow to customers");

    let topic_customer_event =
        seed_user_mention_event(&db, tenant_id, author_id, "topic", topic.id, customer_id).await;
    let reply_customer_event =
        seed_user_mention_event(&db, tenant_id, author_id, "reply", reply.id, customer_id).await;
    let topic_manager_event =
        seed_user_mention_event(&db, tenant_id, author_id, "topic", topic.id, manager_id).await;

    let registry = ModuleRegistry::new()
        .register(NotificationsModule)
        .register(ForumModule);
    let mut extensions = registry
        .build_runtime_extensions()
        .expect("Notifications and Forum runtime extensions should initialize");
    let recipient_port: SharedForumNotificationRecipientContextPort =
        Arc::new(StaticRecipientContextPort {
            customer_id,
            manager_id,
        });
    extensions.insert(recipient_port);
    let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db.clone()));
    let providers = materialize_notification_source_registry(&mut extensions, &host)
        .expect("Forum source factory should consume the recipient capability");
    let provider = providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");

    let topic_customer_ref = source_event_ref(&topic_customer_event);
    let topic_descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: topic_customer_ref.clone(),
        })
        .await
        .expect("customer topic mention description should complete")
        .expect("customer topic mention should be describable");
    assert_eq!(topic_descriptor.target.kind.as_str(), "forum.topic");
    let topic_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: topic_customer_ref.clone(),
            descriptor: topic_descriptor.clone(),
            cursor: None,
            limit: 1,
        })
        .await
        .expect("customer topic mention audience should resolve");
    assert_eq!(topic_page.recipients().len(), 1);
    assert_eq!(topic_page.recipients()[0].recipient_id, customer_id);
    assert!(topic_page.is_complete());

    let reply_customer_ref = source_event_ref(&reply_customer_event);
    let reply_descriptor = provider
        .describe_event(DescribeNotificationRequest {
            event: reply_customer_ref.clone(),
        })
        .await
        .expect("customer reply mention description should complete")
        .expect("customer reply mention should be describable");
    assert_eq!(reply_descriptor.target.kind.as_str(), "forum.reply");
    let reply_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: reply_customer_ref,
            descriptor: reply_descriptor,
            cursor: None,
            limit: 1,
        })
        .await
        .expect("customer reply mention audience should resolve");
    assert_eq!(reply_page.recipients().len(), 1);
    assert_eq!(reply_page.recipients()[0].recipient_id, customer_id);
    assert!(reply_page.is_complete());

    assert!(
        provider
            .describe_event(DescribeNotificationRequest {
                event: source_event_ref(&topic_manager_event),
            })
            .await
            .expect("manager topic mention description should fail closed")
            .is_none()
    );

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic.id,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: role_only(UserRole::Manager),
            },
        )
        .await
        .expect("topic audience should narrow away from the customer");
    let stale_page = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: topic_customer_ref,
            descriptor: topic_descriptor,
            cursor: None,
            limit: 1,
        })
        .await
        .expect("stale customer mention descriptor should be rechecked");
    assert!(stale_page.recipients().is_empty());
    assert!(stale_page.is_complete());
}

fn role_only(role: UserRole) -> ForumAudienceConstraints {
    ForumAudienceConstraints {
        roles_any: vec![role],
        ..ForumAudienceConstraints::default()
    }
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

async fn seed_user_mention_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    source_kind: &str,
    source_id: Uuid,
    mentioned_user_id: Uuid,
) -> forum_domain_event::Model {
    let revision = ensure_relation_revision(db, tenant_id, source_kind, source_id).await;
    forum_user_mention::ActiveModel {
        tenant_id: Set(tenant_id),
        source_kind: Set(source_kind.to_string()),
        source_id: Set(source_id),
        source_locale: Set("en".to_string()),
        source_revision_id: Set(revision.revision_id),
        mentioned_user_id: Set(mentioned_user_id),
        handle_snapshot: Set(format!(
            "member_{}",
            &mentioned_user_id.simple().to_string()[..12]
        )),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("user mention relation should persist");

    forum_domain_event::ActiveModel {
        sequence_no: NotSet,
        event_id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        aggregate_type: Set(source_kind.to_string()),
        aggregate_id: Set(source_id),
        event_type: Set("forum.mention.user_added".to_string()),
        schema_version: Set(1),
        actor_id: Set(Some(actor_id)),
        payload: Set(serde_json::json!({
            "source_kind": source_kind,
            "source_id": source_id,
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

async fn ensure_relation_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_kind: &str,
    source_id: Uuid,
) -> forum_relation_revision::Model {
    if let Some(revision) = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq(source_kind))
        .filter(forum_relation_revision::Column::TargetId.eq(source_id))
        .filter(forum_relation_revision::Column::Locale.eq("en"))
        .order_by_desc(forum_relation_revision::Column::RevisionId)
        .one(db)
        .await
        .expect("relation revision query should succeed")
    {
        return revision;
    }

    forum_relation_revision::ActiveModel {
        revision_id: NotSet,
        tenant_id: Set(tenant_id),
        target_kind: Set(source_kind.to_string()),
        target_id: Set(source_id),
        locale: Set("en".to_string()),
        projection_fingerprint: Set("notification-recipient-mention-test".to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("relation revision fixture should persist")
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let url = format!(
        "sqlite:file:forum_notification_recipient_mention_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification recipient mention database should connect");
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
