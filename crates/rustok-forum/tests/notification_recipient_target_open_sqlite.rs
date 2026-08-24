use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortCallPolicy, PortContext, PortError};
use rustok_core::{MigrationSource, ModuleRegistry, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumAudienceConstraints, ForumModule, ForumNotificationRecipientContextPort,
    ForumNotificationRecipientContextRequest, ForumTopicAudiencePolicyService, ReplyService,
    SetForumTopicAudiencePolicyInput, SharedForumNotificationRecipientContextPort, TopicService,
};
use rustok_notifications::NotificationsModule;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, NotificationOpenAuthorization, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRef, materialize_notification_source_registry,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
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
async fn notification_target_open_uses_exact_recipient_role_for_topics_and_replies() {
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
                name: "Recipient target open".into(),
                slug: "recipient-target-open".into(),
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
                title: "Recipient-specific target".into(),
                slug: Some("recipient-specific-target".into()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Notification target-open authorization must use the exact recipient.",
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
                content: rustok_api::RichTextDocument::single_paragraph(
                    "Recipient-specific reply target",
                ),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic.id,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic audience should narrow to customers");

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
    let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
    let providers = materialize_notification_source_registry(&mut extensions, &host)
        .expect("Forum source factory should consume the recipient capability");
    let provider = providers
        .get_by_str("forum")
        .expect("Forum source should be discoverable");

    let topic_target = NotificationTargetRef {
        owner: NotificationSourceSlug::new("forum").expect("source slug"),
        kind: NotificationTargetKind::new("forum.topic").expect("topic target kind"),
        id: topic.id,
    };
    let reply_target = NotificationTargetRef {
        owner: NotificationSourceSlug::new("forum").expect("source slug"),
        kind: NotificationTargetKind::new("forum.reply").expect("reply target kind"),
        id: reply.id,
    };

    for target in [topic_target.clone(), reply_target.clone()] {
        let allowed = provider
            .authorize_target_open(AuthorizeNotificationTargetRequest {
                tenant_id,
                recipient_id: customer_id,
                target,
            })
            .await
            .expect("customer target-open authorization should complete");
        assert!(matches!(
            allowed,
            NotificationOpenAuthorization::Allowed { .. }
        ));
    }

    for target in [topic_target, reply_target] {
        let denied = provider
            .authorize_target_open(AuthorizeNotificationTargetRequest {
                tenant_id,
                recipient_id: manager_id,
                target,
            })
            .await
            .expect("manager target-open authorization should fail closed");
        assert_eq!(denied, NotificationOpenAuthorization::Unavailable);
    }

    let unavailable = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id: Uuid::new_v4(),
            target: NotificationTargetRef {
                owner: NotificationSourceSlug::new("forum").expect("source slug"),
                kind: NotificationTargetKind::new("forum.topic").expect("topic target kind"),
                id: topic.id,
            },
        })
        .await
        .expect("unavailable recipient should fail closed without an existence oracle");
    assert_eq!(unavailable, NotificationOpenAuthorization::Unavailable);
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let url = format!(
        "sqlite:file:forum_notification_recipient_target_open_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification recipient target-open database should connect");
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
