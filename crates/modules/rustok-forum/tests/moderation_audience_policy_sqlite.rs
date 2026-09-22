use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumAudienceConstraints, ForumAudienceFacts, ForumAudienceFactsPort,
    ForumAudienceFactsRequest, ForumCategoryModerationAudiencePolicyService, ForumError,
    ForumModule, ModerationService, ReplyService, ReplyStatus,
    SetForumCategoryModerationAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, IntoActiveModel,
};
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
            .expect("moderation facts recorder should lock")
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
        "sqlite:file:forum_moderation_audience_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum moderation audience sqlite database should connect");
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
    moderated: bool,
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
                moderated,
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
                title: format!("Moderation audience {suffix}"),
                slug: Some(format!("moderation-audience-{suffix}")),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Moderation audience topic fixture",
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

async fn create_reply(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: SecurityContext,
    topic_id: Uuid,
    suffix: &str,
) -> Uuid {
    ReplyService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            topic_id,
            CreateReplyInput {
                locale: "en".into(),
                content: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Moderation audience reply {suffix}"
                )),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created")
        .id
}

async fn topic_model(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> rustok_forum::entities::forum_topic::Model {
    rustok_forum::entities::forum_topic::Entity::find_by_id(topic_id)
        .one(db)
        .await
        .expect("topic lookup should succeed")
        .filter(|topic| topic.tenant_id == tenant_id)
        .expect("tenant topic should exist")
}

async fn reply_model(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> rustok_forum::entities::forum_reply::Model {
    rustok_forum::entities::forum_reply::Entity::find_by_id(reply_id)
        .one(db)
        .await
        .expect("reply lookup should succeed")
        .filter(|reply| reply.tenant_id == tenant_id)
        .expect("tenant reply should exist")
}

#[tokio::test]
async fn moderation_audience_gates_topic_reply_and_solution_owner_paths() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let policy_admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let allowed_admin_id = Uuid::new_v4();
    let allowed_admin = SecurityContext::new(UserRole::Admin, Some(allowed_admin_id));
    let denied_super_id = Uuid::new_v4();
    let denied_super = SecurityContext::new(UserRole::SuperAdmin, Some(denied_super_id));
    let author_id = Uuid::new_v4();
    let author = SecurityContext::new(UserRole::Customer, Some(author_id));

    let role_category = create_category(
        &db,
        tenant_id,
        policy_admin.clone(),
        "moderation-role",
        None,
        true,
    )
    .await;
    let role_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        author.clone(),
        role_category,
        "role",
    )
    .await;
    let allowed_reply = create_reply(
        &db,
        &event_bus,
        tenant_id,
        author.clone(),
        role_topic,
        "allowed",
    )
    .await;
    let denied_reply = create_reply(
        &db,
        &event_bus,
        tenant_id,
        author.clone(),
        role_topic,
        "denied",
    )
    .await;

    let policies = ForumCategoryModerationAudiencePolicyService::new(db.clone());
    policies
        .set(
            tenant_id,
            role_category,
            policy_admin.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("role moderation layer should persist");

    let moderation = ModerationService::new(db.clone(), event_bus.clone());
    moderation
        .approve_reply(tenant_id, allowed_reply, role_topic, allowed_admin.clone())
        .await
        .expect("matching moderation role should approve a reply");
    assert_eq!(
        reply_model(&db, tenant_id, allowed_reply).await.status,
        ReplyStatus::Approved
    );

    assert!(matches!(
        moderation
            .approve_reply(
                tenant_id,
                denied_reply,
                role_topic,
                denied_super.clone(),
            )
            .await,
        Err(ForumError::Forbidden(message))
            if message == "Forum moderation is unavailable for the current audience"
    ));
    assert_eq!(
        reply_model(&db, tenant_id, denied_reply).await.status,
        ReplyStatus::Pending,
        "denied moderation must not mutate reply status"
    );

    assert!(matches!(
        moderation
            .pin_topic(tenant_id, role_topic, denied_super.clone())
            .await,
        Err(ForumError::Forbidden(_))
    ));
    assert!(!topic_model(&db, tenant_id, role_topic).await.is_pinned);
    moderation
        .pin_topic(tenant_id, role_topic, allowed_admin.clone())
        .await
        .expect("matching moderation role should pin a topic");
    assert!(topic_model(&db, tenant_id, role_topic).await.is_pinned);

    let explicit_deny_category = create_category(
        &db,
        tenant_id,
        policy_admin.clone(),
        "moderation-explicit-deny",
        None,
        false,
    )
    .await;
    let explicit_deny_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        author.clone(),
        explicit_deny_category,
        "explicit-deny",
    )
    .await;
    policies
        .set(
            tenant_id,
            explicit_deny_category,
            policy_admin.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    deny_user_ids: vec![allowed_admin_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("explicit deny moderation layer should persist");
    assert!(matches!(
        moderation
            .lock_topic(tenant_id, explicit_deny_topic, allowed_admin.clone())
            .await,
        Err(ForumError::Forbidden(_))
    ));
    assert!(
        !topic_model(&db, tenant_id, explicit_deny_topic)
            .await
            .is_locked
    );

    let group_category = create_category(
        &db,
        tenant_id,
        policy_admin.clone(),
        "moderation-group",
        None,
        false,
    )
    .await;
    let group_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        author.clone(),
        group_category,
        "group",
    )
    .await;
    let required_group_id = Uuid::new_v4();
    policies
        .set(
            tenant_id,
            group_category,
            policy_admin.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    group_members_any: vec![required_group_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("group moderation layer should persist");
    assert!(matches!(
        moderation
            .lock_topic(tenant_id, group_topic, allowed_admin.clone())
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));
    assert!(!topic_model(&db, tenant_id, group_topic).await.is_locked);

    let requests = Arc::new(Mutex::new(Vec::new()));
    let facts = Arc::new(RecordingGroupFactsPort {
        active_user_id: allowed_admin_id,
        requests: requests.clone(),
    });
    ModerationService::with_audience_facts(db.clone(), event_bus.clone(), facts)
        .lock_topic_with_audience_context(
            tenant_id,
            group_topic,
            allowed_admin.clone(),
            read_context(tenant_id, allowed_admin_id, "moderation-group-lock"),
        )
        .await
        .expect("matching exact group facts should allow moderation");
    assert!(topic_model(&db, tenant_id, group_topic).await.is_locked);
    {
        let recorded = requests
            .lock()
            .expect("moderation facts requests should lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].tenant_id, tenant_id);
        assert_eq!(recorded[0].user_id, allowed_admin_id);
        assert_eq!(recorded[0].group_ids, vec![required_group_id]);
    }

    let solution_category = create_category(
        &db,
        tenant_id,
        policy_admin.clone(),
        "moderation-solution",
        None,
        false,
    )
    .await;
    let solution_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        author.clone(),
        solution_category,
        "solution",
    )
    .await;
    let solution_reply = create_reply(
        &db,
        &event_bus,
        tenant_id,
        SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4())),
        solution_topic,
        "solution",
    )
    .await;
    policies
        .set(
            tenant_id,
            solution_category,
            policy_admin,
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("solution moderation layer should persist");

    assert!(matches!(
        moderation
            .mark_solution(tenant_id, solution_topic, solution_reply, denied_super,)
            .await,
        Err(ForumError::Forbidden(_))
    ));
    assert!(
        rustok_forum::entities::forum_solution::Entity::find_by_id((solution_topic, tenant_id,))
            .one(&db)
            .await
            .expect("solution lookup should succeed")
            .is_none(),
        "denied moderator must not write solution state"
    );

    moderation
        .mark_solution(tenant_id, solution_topic, solution_reply, author)
        .await
        .expect("topic author owner scope should not be narrowed by moderator audience");
    assert!(
        rustok_forum::entities::forum_solution::Entity::find_by_id((solution_topic, tenant_id,))
            .one(&db)
            .await
            .expect("solution lookup should succeed")
            .is_some()
    );
}

#[tokio::test]
async fn moderation_audience_inherits_clears_and_enforces_database_bounds() {
    let (db, _event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let root = create_category(
        &db,
        tenant_id,
        admin.clone(),
        "moderation-root",
        None,
        false,
    )
    .await;
    let child = create_category(
        &db,
        tenant_id,
        admin.clone(),
        "moderation-child",
        Some(root),
        false,
    )
    .await;

    let policies = ForumCategoryModerationAudiencePolicyService::new(db.clone());
    policies
        .set(
            tenant_id,
            root,
            admin.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("root moderation layer should persist");
    let channels = (0..32)
        .map(|index| format!("moderation-channel-{index:02}"))
        .collect::<Vec<_>>();
    let child_policy = policies
        .set(
            tenant_id,
            child,
            admin.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    channel_members_any: channels,
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("bounded child moderation layer should persist");
    assert_eq!(child_policy.effective_layers.len(), 2);
    assert_eq!(
        child_policy
            .configured_constraints
            .as_ref()
            .expect("child moderation layer should exist")
            .channel_members_any
            .len(),
        32
    );

    let extra_channel =
        rustok_forum::entities::forum_category_moderation_audience_channel::ActiveModel {
            tenant_id: Set(tenant_id),
            category_id: Set(child),
            channel_slug: Set("moderation-channel-overflow".to_string()),
        };
    assert!(
        extra_channel.insert(&db).await.is_err(),
        "database must reject a thirty-third moderation channel"
    );

    let policy_row =
        rustok_forum::entities::forum_category_moderation_audience_policy::Entity::find_by_id((
            tenant_id, child,
        ))
        .one(&db)
        .await
        .expect("moderation policy lookup should succeed")
        .expect("moderation policy row should exist");
    let mut mutable_policy = policy_row.into_active_model();
    mutable_policy.minimum_trust_level = Set(Some(7));
    assert!(
        mutable_policy.update(&db).await.is_err(),
        "database must reject mutable moderation policy updates"
    );

    let cleared = policies
        .set(
            tenant_id,
            child,
            admin,
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: ForumAudienceConstraints::default(),
            },
        )
        .await
        .expect("empty moderation constraints should clear only the child layer");
    assert!(cleared.configured_constraints.is_none());
    assert_eq!(cleared.effective_layers.len(), 1);
}
