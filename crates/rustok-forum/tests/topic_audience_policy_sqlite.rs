use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumTopicAudiencePolicyService,
    SetForumCategoryAudiencePolicyInput, SetForumTopicAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_audience_policy_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic audience sqlite database should connect");
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
    parent_id: Option<Uuid>,
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
                parent_id,
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
    category_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> Uuid {
    TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: slug.replace('-', " "),
                slug: Some(slug.into()),
                body: rustok_api::RichTextDocument::single_paragraph("Topic audience fixture"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

#[tokio::test]
async fn topic_audience_layer_narrows_inherited_category_layers_and_remains_bounded() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let root = create_category(&db, tenant_id, admin.clone(), "root", None).await;
    let child = create_category(&db, tenant_id, admin.clone(), "child", Some(root)).await;
    let topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        child,
        admin.clone(),
        "restricted-topic",
    )
    .await;
    let empty_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        child,
        admin.clone(),
        "empty-storage-topic",
    )
    .await;

    let foreign_root =
        create_category(&db, foreign_tenant_id, admin.clone(), "foreign-root", None).await;
    let foreign_topic = create_topic(
        &db,
        &event_bus,
        foreign_tenant_id,
        foreign_root,
        admin.clone(),
        "foreign-topic",
    )
    .await;

    let category_policies = ForumCategoryAudiencePolicyService::new(db.clone());
    category_policies
        .set(
            tenant_id,
            root,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("root category audience layer should persist");
    category_policies
        .set(
            tenant_id,
            child,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    minimum_trust_level: Some(5),
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("child category audience layer should persist");

    let channel_slugs = (0..32)
        .map(|index| format!("topic-channel-{index:02}"))
        .collect::<Vec<_>>();
    let allowed_user_id = Uuid::new_v4();
    let denied_user_id = Uuid::new_v4();
    let topic_policies = ForumTopicAudiencePolicyService::new(db.clone());
    let persisted = topic_policies
        .set(
            tenant_id,
            topic,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    channel_members_any: channel_slugs.clone(),
                    allow_user_ids: vec![allowed_user_id],
                    deny_user_ids: vec![denied_user_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic audience layer should persist");

    assert_eq!(persisted.topic_id, topic);
    assert_eq!(persisted.category_id, child);
    assert_eq!(
        persisted
            .inherited_category_layers
            .iter()
            .map(|layer| layer.category_id)
            .collect::<Vec<_>>(),
        vec![root, child]
    );
    let configured = persisted
        .configured_constraints
        .expect("topic should expose its local narrowing layer");
    assert_eq!(configured.channel_members_any, channel_slugs);
    assert_eq!(configured.allow_user_ids, vec![allowed_user_id]);
    assert_eq!(configured.deny_user_ids, vec![denied_user_id]);

    assert!(matches!(
        topic_policies
            .get(tenant_id, topic, SecurityContext::public_read())
            .await,
        Err(ForumError::Forbidden(_))
    ));

    let cleared = topic_policies
        .set(
            tenant_id,
            topic,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints::default(),
            },
        )
        .await
        .expect("empty constraints should clear only the topic layer");
    assert!(cleared.configured_constraints.is_none());
    assert_eq!(
        cleared
            .inherited_category_layers
            .iter()
            .map(|layer| layer.category_id)
            .collect::<Vec<_>>(),
        vec![root, child]
    );

    topic_policies
        .set(
            tenant_id,
            topic,
            admin.clone(),
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    channel_members_any: (0..32)
                        .map(|index| format!("topic-channel-{index:02}"))
                        .collect(),
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("bounded topic channel layer should persist");

    assert!(matches!(
        topic_policies
            .get(tenant_id, foreign_topic, admin.clone())
            .await,
        Err(ForumError::TopicNotFound(id)) if id == foreign_topic
    ));

    let direct_overflow = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO forum_topic_audience_channels (tenant_id, topic_id, channel_slug) VALUES (?, ?, ?)",
            [
                tenant_id.into(),
                topic.into(),
                "topic-channel-32".into(),
            ],
        ))
        .await;
    assert!(
        direct_overflow.is_err(),
        "database must reject a thirty-third topic channel relation"
    );

    let direct_relation_update = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_topic_audience_channels SET channel_slug = ? WHERE tenant_id = ? AND topic_id = ? AND channel_slug = ?",
            [
                "changed".into(),
                tenant_id.into(),
                topic.into(),
                "topic-channel-00".into(),
            ],
        ))
        .await;
    assert!(
        direct_relation_update.is_err(),
        "database must reject mutable topic relation-row updates"
    );

    let direct_policy_update = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_topic_audience_policies SET minimum_trust_level = ? WHERE tenant_id = ? AND topic_id = ?",
            [
                9.into(),
                tenant_id.into(),
                topic.into(),
            ],
        ))
        .await;
    assert!(
        direct_policy_update.is_err(),
        "database must reject mutable topic policy-row updates"
    );

    let cross_tenant_policy = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO forum_topic_audience_policies (tenant_id, topic_id, minimum_trust_level, updated_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
            [
                foreign_tenant_id.into(),
                topic.into(),
                1.into(),
            ],
        ))
        .await;
    assert!(
        cross_tenant_policy.is_err(),
        "database must reject a cross-tenant topic policy relation"
    );

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO forum_topic_audience_policies (tenant_id, topic_id, minimum_trust_level, updated_at) VALUES (?, ?, NULL, CURRENT_TIMESTAMP)",
        [tenant_id.into(), empty_topic.into()],
    ))
    .await
    .expect("direct empty layer fixture should insert");
    assert!(matches!(
        topic_policies.get(tenant_id, empty_topic, admin).await,
        Err(ForumError::Validation(message)) if message.contains("empty local layer")
    ));
}
