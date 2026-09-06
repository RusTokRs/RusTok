use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumTopicAudiencePolicyService,
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
    SetForumCategoryAudiencePolicyInput, SetForumTopicAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Clone)]
struct RecordingFactsPort {
    channel_member_id: Uuid,
    requests: Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
}

#[async_trait]
impl ForumAudienceFactsPort for RecordingFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        self.requests
            .lock()
            .expect("facts request record should lock")
            .push(request.clone());
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: request.include_trust_level.then_some(8),
            channel_memberships: if request.user_id == self.channel_member_id {
                request.channel_slugs.clone()
            } else {
                Vec::new()
            },
            group_memberships: Vec::new(),
        })
    }
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let db_url = format!(
        "sqlite:file:forum_topic_audience_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum topic audience visibility sqlite database should connect");
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

async fn create_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    category_id: Uuid,
    security: SecurityContext,
) -> Uuid {
    TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: "Exact richer audience visibility".into(),
                slug: Some("exact-richer-audience-visibility".into()),
                body: rustok_api::RichTextDocument::single_paragraph("Audience visibility fixture"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: Some(vec!["web".into()]),
            },
        )
        .await
        .expect("topic should be created")
        .id
}

#[tokio::test]
async fn exact_topic_visibility_conjoins_base_category_and_topic_audience_layers() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let allowed_user_id = Uuid::new_v4();
    let denied_user_id = Uuid::new_v4();
    let no_membership_user_id = Uuid::new_v4();
    let manager_user_id = Uuid::new_v4();

    let root = create_category(&db, tenant_id, admin.clone(), "root", None).await;
    let child = create_category(&db, tenant_id, admin.clone(), "child", Some(root)).await;
    let topic = create_topic(&db, &event_bus, tenant_id, child, admin.clone()).await;

    let category_policies = ForumCategoryAudiencePolicyService::new(db.clone());
    category_policies
        .set(
            tenant_id,
            root,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("root role layer should persist");
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
        .expect("child trust layer should persist");
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            topic,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    channel_members_any: vec!["members".into()],
                    deny_user_ids: vec![denied_user_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic membership layer should persist");

    let requests = Arc::new(Mutex::new(Vec::new()));
    let visibility = ForumTopicAudienceVisibilityService::new(
        db.clone(),
        Some(Arc::new(RecordingFactsPort {
            channel_member_id: allowed_user_id,
            requests: requests.clone(),
        })),
    );
    let allowed_viewer = ForumTopicAudienceViewer::authenticated(
        SecurityContext::new(UserRole::Customer, Some(allowed_user_id)),
        read_context(tenant_id, allowed_user_id, "allowed-viewer"),
    )
    .expect("authenticated viewer should validate");

    assert!(
        visibility
            .is_topic_visible(tenant_id, topic, Some("WEB"), &allowed_viewer)
            .await
            .expect("allowed exact visibility should resolve")
    );
    let recorded = requests
        .lock()
        .expect("facts request record should lock")
        .clone();
    assert_eq!(recorded.len(), 2);
    assert!(recorded[0].include_trust_level);
    assert!(recorded[0].channel_slugs.is_empty());
    assert_eq!(recorded[1].channel_slugs, vec!["members".to_string()]);

    let calls_before_base_rejection = recorded.len();
    assert!(
        !visibility
            .is_topic_visible(tenant_id, topic, None, &allowed_viewer)
            .await
            .expect("route-channel rejection should resolve as absent")
    );
    assert_eq!(
        requests
            .lock()
            .expect("facts request record should lock")
            .len(),
        calls_before_base_rejection,
        "base visibility must fail before richer owner facts are requested"
    );

    assert!(
        !visibility
            .is_topic_visible(
                tenant_id,
                topic,
                Some("web"),
                &ForumTopicAudienceViewer::public(),
            )
            .await
            .expect("public richer visibility should fail closed")
    );

    let manager_viewer = ForumTopicAudienceViewer::authenticated(
        SecurityContext::new(UserRole::Manager, Some(manager_user_id)),
        read_context(tenant_id, manager_user_id, "manager-viewer"),
    )
    .expect("manager viewer should validate");
    assert!(
        !visibility
            .is_topic_visible(tenant_id, topic, Some("web"), &manager_viewer)
            .await
            .expect("nonmatching local role should resolve as denied")
    );

    let denied_viewer = ForumTopicAudienceViewer::authenticated(
        SecurityContext::new(UserRole::Customer, Some(denied_user_id)),
        read_context(tenant_id, denied_user_id, "denied-viewer"),
    )
    .expect("denied viewer should validate");
    assert!(
        !visibility
            .is_topic_visible(tenant_id, topic, Some("web"), &denied_viewer)
            .await
            .expect("topic explicit deny should win")
    );

    let no_membership_viewer = ForumTopicAudienceViewer::authenticated(
        SecurityContext::new(UserRole::Customer, Some(no_membership_user_id)),
        read_context(tenant_id, no_membership_user_id, "no-membership-viewer"),
    )
    .expect("no-membership viewer should validate");
    assert!(
        !visibility
            .is_topic_visible(tenant_id, topic, Some("web"), &no_membership_viewer)
            .await
            .expect("missing exact membership should deny")
    );

    assert!(matches!(
        ForumTopicAudienceVisibilityService::without_facts_provider(db.clone())
            .is_topic_visible(tenant_id, topic, Some("web"), &allowed_viewer)
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));

    let foreign_context_viewer = ForumTopicAudienceViewer::authenticated(
        SecurityContext::new(UserRole::Customer, Some(allowed_user_id)),
        read_context(foreign_tenant_id, allowed_user_id, "foreign-context-viewer"),
    )
    .expect("foreign context viewer should be structurally valid");
    assert!(matches!(
        visibility
            .is_topic_visible(tenant_id, topic, Some("web"), &foreign_context_viewer)
            .await,
        Err(ForumError::Validation(message)) if message.contains("tenant does not match")
    ));

    let foreign_request_viewer = ForumTopicAudienceViewer::authenticated(
        SecurityContext::new(UserRole::Customer, Some(allowed_user_id)),
        read_context(foreign_tenant_id, allowed_user_id, "foreign-request-viewer"),
    )
    .expect("foreign request viewer should validate");
    assert!(
        !visibility
            .is_topic_visible(
                foreign_tenant_id,
                topic,
                Some("web"),
                &foreign_request_viewer,
            )
            .await
            .expect("cross-tenant topic should resolve as absent")
    );

    assert!(matches!(
        ForumTopicAudienceViewer::authenticated(
            SecurityContext::new(UserRole::Customer, Some(allowed_user_id)),
            read_context(tenant_id, Uuid::new_v4(), "wrong-actor-viewer"),
        ),
        Err(ForumError::Validation(message)) if message.contains("actor does not match")
    ));
}
