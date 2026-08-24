use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryTopicCreateAudiencePolicyService, ForumError, ForumModule,
    SetForumCategoryTopicCreateAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectionTrait,ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Clone)]
struct RecordingGroupFactsPort {
    active_user_id: Uuid,
    requests: Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
}

#[async_trait]
impl ForumAudienceFactsPort for RecordingGroupFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        self.requests
            .lock()
            .expect("topic-create facts recorder should lock")
            .push(request.clone());
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: None,
            channel_memberships: Vec::new(),
            group_memberships: if request.user_id == self.active_user_id {
                request.group_ids.clone()
            } else {
                Vec::new()
            },
        })
    }
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_create_audience_enforcement_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic-create audience sqlite database should connect");
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

fn read_context(tenant_id: Uuid, user_id: Uuid, correlation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        correlation,
    )
    .with_deadline(Duration::from_secs(1))
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

fn topic_input(category_id: Uuid, suffix: &str) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".into(),
        category_id,
        title: format!("Topic create audience {suffix}"),
        slug: Some(format!("topic-create-audience-{suffix}")),
        body: rustok_api::RichTextDocument::single_paragraph("Topic create audience fixture"),
        metadata: serde_json::json!({}),
        tags: Vec::new(),
        channel_slugs: None,
    }
}

async fn topic_count(db: &DatabaseConnection) -> u64 {
    rustok_forum::entities::forum_topic::Entity::find()
        .count(db)
        .await
        .expect("topic count should resolve")
}

#[tokio::test]
async fn topic_create_command_enforces_inherited_audience_before_writes() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let policy_admin_id = Uuid::new_v4();
    let allowed_admin_id = Uuid::new_v4();
    let denied_admin_id = Uuid::new_v4();
    let manager_id = Uuid::new_v4();
    let policy_admin = SecurityContext::new(UserRole::Admin, Some(policy_admin_id));
    let allowed_admin = SecurityContext::new(UserRole::Admin, Some(allowed_admin_id));
    let denied_admin = SecurityContext::new(UserRole::Admin, Some(denied_admin_id));
    let manager = SecurityContext::new(UserRole::Manager, Some(manager_id));

    let unrestricted =
        create_category(&db, tenant_id, policy_admin.clone(), "unrestricted", None).await;
    let role_only = create_category(&db, tenant_id, policy_admin.clone(), "role-only", None).await;
    let root = create_category(&db, tenant_id, policy_admin.clone(), "root", None).await;
    let group_child = create_category(
        &db,
        tenant_id,
        policy_admin.clone(),
        "group-child",
        Some(root),
    )
    .await;
    let explicit_allow =
        create_category(&db, tenant_id, policy_admin.clone(), "explicit-allow", None).await;
    let explicit_deny =
        create_category(&db, tenant_id, policy_admin.clone(), "explicit-deny", None).await;

    let policies = ForumCategoryTopicCreateAudiencePolicyService::new(db.clone());
    policies
        .set(
            tenant_id,
            role_only,
            policy_admin.clone(),
            SetForumCategoryTopicCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("role-only topic-create layer should persist");
    policies
        .set(
            tenant_id,
            root,
            policy_admin.clone(),
            SetForumCategoryTopicCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("root topic-create layer should persist");
    let required_group_id = Uuid::new_v4();
    policies
        .set(
            tenant_id,
            group_child,
            policy_admin.clone(),
            SetForumCategoryTopicCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    group_members_any: vec![required_group_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("group child topic-create layer should persist");
    policies
        .set(
            tenant_id,
            explicit_allow,
            policy_admin.clone(),
            SetForumCategoryTopicCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    group_members_any: vec![Uuid::new_v4()],
                    allow_user_ids: vec![manager_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("explicit allow topic-create layer should persist");
    policies
        .set(
            tenant_id,
            explicit_deny,
            policy_admin.clone(),
            SetForumCategoryTopicCreateAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    deny_user_ids: vec![allowed_admin_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("explicit deny topic-create layer should persist");

    let ordinary = TopicService::new(db.clone(), event_bus.clone());
    ordinary
        .create(
            tenant_id,
            manager.clone(),
            topic_input(unrestricted, "unrestricted"),
        )
        .await
        .expect("category without topic-create audience should preserve compatibility");
    ordinary
        .create(
            tenant_id,
            allowed_admin.clone(),
            topic_input(role_only, "role-allowed"),
        )
        .await
        .expect("matching local role should not require owner facts or caller context");
    ordinary
        .create(
            tenant_id,
            manager.clone(),
            topic_input(explicit_allow, "explicit-allowed"),
        )
        .await
        .expect("explicit allow should short-circuit unresolved owner facts");

    let count_before_explicit_deny = topic_count(&db).await;
    assert!(matches!(
        ordinary
            .create(
                tenant_id,
                allowed_admin.clone(),
                topic_input(explicit_deny, "explicit-denied"),
            )
            .await,
        Err(ForumError::Forbidden(_))
    ));
    assert_eq!(topic_count(&db).await, count_before_explicit_deny);

    let count_before_denials = topic_count(&db).await;
    assert!(matches!(
        ordinary
            .create(
                tenant_id,
                manager.clone(),
                topic_input(role_only, "role-denied"),
            )
            .await,
        Err(ForumError::Forbidden(message))
            if message == "Forum topic creation is unavailable for the current audience"
    ));
    assert_eq!(topic_count(&db).await, count_before_denials);

    assert!(matches!(
        ordinary
            .create(
                tenant_id,
                allowed_admin.clone(),
                topic_input(group_child, "missing-capability"),
            )
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));
    assert_eq!(topic_count(&db).await, count_before_denials);

    let requests = Arc::new(Mutex::new(Vec::new()));
    let facts_port = Arc::new(RecordingGroupFactsPort {
        active_user_id: allowed_admin_id,
        requests: requests.clone(),
    });
    let composed = TopicService::with_audience_facts(db.clone(), event_bus.clone(), facts_port);

    assert!(matches!(
        composed
            .create(
                tenant_id,
                allowed_admin.clone(),
                topic_input(group_child, "missing-context"),
            )
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));
    assert!(
        requests
            .lock()
            .expect("facts requests should lock")
            .is_empty()
    );

    composed
        .create_with_audience_context(
            tenant_id,
            allowed_admin.clone(),
            read_context(tenant_id, allowed_admin_id, "allowed-group-create"),
            topic_input(group_child, "group-allowed"),
        )
        .await
        .expect("matching exact group facts should allow topic creation");
    let recorded = requests.lock().expect("facts requests should lock").clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].tenant_id, tenant_id);
    assert_eq!(recorded[0].user_id, allowed_admin_id);
    assert_eq!(recorded[0].group_ids, vec![required_group_id]);

    let count_before_group_denial = topic_count(&db).await;
    assert!(matches!(
        composed
            .create_with_audience_context(
                tenant_id,
                denied_admin.clone(),
                read_context(tenant_id, denied_admin_id, "denied-group-create"),
                topic_input(group_child, "group-denied"),
            )
            .await,
        Err(ForumError::Forbidden(_))
    ));
    assert_eq!(topic_count(&db).await, count_before_group_denial);

    let calls_before_wrong_actor = requests.lock().expect("facts requests should lock").len();
    assert!(matches!(
        composed
            .create_with_audience_context(
                tenant_id,
                allowed_admin.clone(),
                read_context(tenant_id, Uuid::new_v4(), "wrong-actor-create"),
                topic_input(group_child, "wrong-actor"),
            )
            .await,
        Err(ForumError::Validation(message)) if message.contains("actor does not match")
    ));
    assert_eq!(
        requests.lock().expect("facts requests should lock").len(),
        calls_before_wrong_actor,
        "invalid exact caller context must fail before owner facts"
    );

    assert!(matches!(
        composed
            .create_with_audience_context(
                tenant_id,
                allowed_admin,
                read_context(foreign_tenant_id, allowed_admin_id, "foreign-tenant-create"),
                topic_input(group_child, "foreign-context"),
            )
            .await,
        Err(ForumError::Validation(message)) if message.contains("tenant does not match")
    ));
    assert_eq!(topic_count(&db).await, count_before_group_denial);
}
