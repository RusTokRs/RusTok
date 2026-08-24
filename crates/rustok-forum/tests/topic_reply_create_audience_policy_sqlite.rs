use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumAudienceConstraints, ForumCategoryReplyCreateAudiencePolicyService, ForumError,
    ForumModule, ForumReplyCreateAudienceAuthorizationService, ForumTopicAudiencePolicyService,
    ForumTopicReplyCreateAudiencePolicyService, ReplyService,
    SetForumCategoryReplyCreateAudiencePolicyInput, SetForumTopicAudiencePolicyInput,
    SetForumTopicReplyCreateAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    IntoActiveModel, PaginatorTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_reply_create_audience_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic reply-create audience sqlite database should connect");
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
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus)
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> Uuid {
    CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".into(),
                name: slug.replace('-', " "),
                slug: slug.into(),
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
        .id
}

async fn create_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: SecurityContext,
    category_id: Uuid,
    suffix: &str,
) -> Uuid {
    TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: format!("Topic reply-create audience {suffix}"),
                slug: Some(format!("topic-reply-create-audience-{suffix}")),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Topic reply-create audience fixture",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

fn reply_input(suffix: &str) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".into(),
        content: rustok_api::RichTextDocument::single_paragraph(format!(
            "Topic reply-create audience reply {suffix}"
        )),
        parent_reply_id: None,
    }
}

async fn reply_counts(db: &DatabaseConnection) -> (u64, u64) {
    let replies = rustok_forum::entities::forum_reply::Entity::find()
        .count(db)
        .await
        .expect("reply count should resolve");
    let bodies = rustok_forum::entities::forum_reply_body::Entity::find()
        .count(db)
        .await
        .expect("reply body count should resolve");
    (replies, bodies)
}

#[tokio::test]
async fn topic_reply_create_layer_narrows_categories_and_clears_locally() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let policy_admin_id = Uuid::new_v4();
    let allowed_admin_id = Uuid::new_v4();
    let other_admin_id = Uuid::new_v4();
    let manager_id = Uuid::new_v4();
    let policy_admin = SecurityContext::new(UserRole::Admin, Some(policy_admin_id));
    let allowed_admin = SecurityContext::new(UserRole::Admin, Some(allowed_admin_id));
    let other_admin = SecurityContext::new(UserRole::Admin, Some(other_admin_id));
    let manager = SecurityContext::new(UserRole::Manager, Some(manager_id));

    let category = create_category(
        &db,
        tenant_id,
        policy_admin.clone(),
        "topic-reply-create-narrowing",
    )
    .await;
    let topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        policy_admin.clone(),
        category,
        "narrowing",
    )
    .await;

    ForumCategoryReplyCreateAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            category,
            policy_admin.clone(),
            SetForumCategoryReplyCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("category reply-create layer should persist");

    let topic_policies = ForumTopicReplyCreateAudiencePolicyService::new(db.clone());
    let policy = topic_policies
        .set(
            tenant_id,
            topic,
            policy_admin.clone(),
            SetForumTopicReplyCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    allow_user_ids: vec![allowed_admin_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic reply-create narrowing should persist");
    assert_eq!(policy.inherited_category_layers.len(), 1);
    assert_eq!(
        policy
            .configured_constraints
            .as_ref()
            .expect("topic layer should exist")
            .allow_user_ids,
        vec![allowed_admin_id]
    );

    let authorization =
        ForumReplyCreateAudienceAuthorizationService::without_facts_provider(db.clone());
    let topic_denial = authorization
        .evaluate(tenant_id, topic, &other_admin, None)
        .await
        .expect("topic-local denial should evaluate");
    assert!(!topic_denial.allowed);
    assert_eq!(topic_denial.topic_id, topic);
    assert_eq!(topic_denial.denied_by_category_id, None);
    assert_eq!(topic_denial.evaluated_layers, 2);

    let category_denial = authorization
        .evaluate(tenant_id, topic, &manager, None)
        .await
        .expect("category denial should evaluate");
    assert!(!category_denial.allowed);
    assert_eq!(category_denial.topic_id, topic);
    assert_eq!(category_denial.denied_by_category_id, Some(category));
    assert_eq!(category_denial.evaluated_layers, 1);

    let replies = ReplyService::new(db.clone(), event_bus.clone());
    replies
        .create(
            tenant_id,
            allowed_admin,
            topic,
            reply_input("allowed-admin"),
        )
        .await
        .expect("actor matching category and topic layers should reply");

    let before_denial = reply_counts(&db).await;
    assert!(matches!(
        replies
            .create(
                tenant_id,
                other_admin.clone(),
                topic,
                reply_input("topic-denied"),
            )
            .await,
        Err(ForumError::Forbidden(message))
            if message == "Forum reply creation is unavailable for the current audience"
    ));
    assert_eq!(
        reply_counts(&db).await,
        before_denial,
        "topic-local denial must occur before reply and body writes"
    );

    let cleared = topic_policies
        .set(
            tenant_id,
            topic,
            policy_admin,
            SetForumTopicReplyCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints::default(),
            },
        )
        .await
        .expect("empty topic constraints should clear only the local layer");
    assert!(cleared.configured_constraints.is_none());
    assert_eq!(cleared.inherited_category_layers.len(), 1);

    replies
        .create(
            tenant_id,
            other_admin,
            topic,
            reply_input("after-topic-clear"),
        )
        .await
        .expect("clearing topic layer should restore category-only authorization");
}

#[tokio::test]
async fn topic_reply_create_policy_is_separate_and_database_bounded() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category =
        create_category(&db, tenant_id, admin.clone(), "topic-reply-create-storage").await;
    let topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        admin.clone(),
        category,
        "storage",
    )
    .await;

    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Manager],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic visibility audience should persist independently");

    let channels = (0..32)
        .map(|index| format!("reply-channel-{index:02}"))
        .collect::<Vec<_>>();
    let reply_policy = ForumTopicReplyCreateAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic,
            admin.clone(),
            SetForumTopicReplyCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    channel_members_any: channels,
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("bounded topic reply-create audience should persist");
    assert_eq!(
        reply_policy
            .configured_constraints
            .as_ref()
            .expect("reply-create topic layer should exist")
            .channel_members_any
            .len(),
        32
    );

    let visibility_policy = ForumTopicAudiencePolicyService::new(db.clone())
        .get(tenant_id, topic, admin.clone())
        .await
        .expect("topic visibility policy should remain readable");
    assert_eq!(
        visibility_policy
            .configured_constraints
            .expect("visibility topic layer should remain configured")
            .roles_any,
        vec![UserRole::Manager]
    );

    let extra_channel =
        rustok_forum::entities::forum_topic_reply_create_audience_channel::ActiveModel {
            tenant_id: Set(tenant_id),
            topic_id: Set(topic),
            channel_slug: Set("reply-channel-overflow".to_string()),
        };
    assert!(
        extra_channel.insert(&db).await.is_err(),
        "database must reject a thirty-third topic reply-create channel"
    );

    let policy_row =
        rustok_forum::entities::forum_topic_reply_create_audience_policy::Entity::find_by_id((
            tenant_id, topic,
        ))
        .one(&db)
        .await
        .expect("topic reply-create policy row lookup should succeed")
        .expect("topic reply-create policy row should exist");
    let mut mutable_policy = policy_row.into_active_model();
    mutable_policy.minimum_trust_level = Set(Some(7));
    assert!(
        mutable_policy.update(&db).await.is_err(),
        "database must reject mutable topic reply-create policy updates"
    );

    let foreign_policy =
        rustok_forum::entities::forum_topic_reply_create_audience_policy::ActiveModel {
            tenant_id: Set(Uuid::new_v4()),
            topic_id: Set(topic),
            minimum_trust_level: Set(None),
            updated_at: Set(chrono::Utc::now().into()),
        };
    assert!(
        foreign_policy.insert(&db).await.is_err(),
        "database must reject a cross-tenant topic reply-create policy"
    );
}
