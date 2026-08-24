use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumTopicAudiencePolicyService,
    ForumTopicAudienceReadService, SetForumCategoryAudiencePolicyInput,
    SetForumTopicAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Clone)]
struct RecordingFactsPort {
    low_trust_user_id: Uuid,
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
            trust_level: request.include_trust_level.then_some(
                if request.user_id == self.low_trust_user_id {
                    1
                } else {
                    8
                },
            ),
            channel_memberships: Vec::new(),
            group_memberships: Vec::new(),
        })
    }
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let database_url = format!(
        "sqlite:file:forum_topic_audience_exact_read_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("exact topic audience read SQLite database should connect");

    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("SQLite platform user fixture should be created");

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

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?1, ?2)",
        vec![user_id.into(), tenant_id.into()],
    ))
    .await
    .expect("platform user fixture should insert");
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

fn read_context(
    tenant_id: Uuid,
    user_id: Uuid,
    channel: Option<&str>,
    correlation: &str,
) -> PortContext {
    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        correlation,
    )
    .with_deadline(Duration::from_secs(1));
    if let Some(channel) = channel {
        context = context.with_channel(channel.to_string());
    }
    context
}

#[tokio::test]
async fn exact_topic_read_enforces_inherited_and_topic_audience_before_hydration() {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let admin_user_id = Uuid::new_v4();
    let allowed_user_id = Uuid::new_v4();
    let low_trust_user_id = Uuid::new_v4();
    let explicitly_denied_user_id = Uuid::new_v4();

    for user_id in [
        admin_user_id,
        allowed_user_id,
        low_trust_user_id,
        explicitly_denied_user_id,
    ] {
        insert_user(&db, tenant_id, user_id).await;
    }

    let admin = SecurityContext::new(UserRole::Admin, Some(admin_user_id));
    let root = create_category(&db, tenant_id, admin.clone(), "members", None).await;
    let child = create_category(&db, tenant_id, admin.clone(), "trusted", Some(root)).await;
    let topic_id = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: child,
                title: "Exact audience read".into(),
                slug: Some("exact-audience-read".into()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Exact audience owner fixture",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: Some(vec!["web".into()]),
            },
        )
        .await
        .expect("topic should be created")
        .id;

    let category_policy = ForumCategoryAudiencePolicyService::new(db.clone());
    category_policy
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
    category_policy
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
            topic_id,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    deny_user_ids: vec![explicitly_denied_user_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic deny layer should persist");

    let requests = Arc::new(Mutex::new(Vec::new()));
    let service = ForumTopicAudienceReadService::with_audience_facts(
        db.clone(),
        event_bus.clone(),
        Arc::new(RecordingFactsPort {
            low_trust_user_id,
            requests: requests.clone(),
        }),
    );

    assert!(
        service
            .get_public_storefront_visible_with_locale_fallback(
                tenant_id,
                topic_id,
                "en",
                Some("en"),
                Some("web"),
            )
            .await
            .expect("public denied topic should resolve as absent")
            .is_none()
    );
    assert!(
        requests
            .lock()
            .expect("facts request record should lock")
            .is_empty(),
        "public rejection must not call optional owner facts"
    );

    let allowed_security = SecurityContext::new(UserRole::Customer, Some(allowed_user_id));
    let allowed = service
        .get_authenticated_storefront_visible_with_audience_context(
            tenant_id,
            allowed_security.clone(),
            read_context(tenant_id, allowed_user_id, Some("web"), "allowed-read"),
            topic_id,
            Some("en"),
        )
        .await
        .expect("trusted exact read should resolve")
        .expect("trusted exact read should hydrate the topic");
    assert_eq!(allowed.id, topic_id);
    assert_eq!(allowed.effective_locale, "en");
    assert_eq!(
        requests
            .lock()
            .expect("facts request record should lock")
            .as_slice(),
        &[ForumAudienceFactsRequest {
            tenant_id,
            user_id: allowed_user_id,
            include_trust_level: true,
            channel_slugs: Vec::new(),
            group_ids: Vec::new(),
        }]
    );

    assert!(
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                SecurityContext::new(UserRole::Customer, Some(low_trust_user_id)),
                read_context(tenant_id, low_trust_user_id, Some("web"), "low-trust-read"),
                topic_id,
                Some("en"),
            )
            .await
            .expect("low-trust exact read should resolve as absent")
            .is_none()
    );
    assert!(
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                SecurityContext::new(UserRole::Customer, Some(explicitly_denied_user_id)),
                read_context(
                    tenant_id,
                    explicitly_denied_user_id,
                    Some("web"),
                    "explicit-deny-read",
                ),
                topic_id,
                Some("en"),
            )
            .await
            .expect("explicitly denied exact read should resolve as absent")
            .is_none()
    );

    let calls_before_base_rejection = requests
        .lock()
        .expect("facts request record should lock")
        .len();
    assert!(
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                allowed_security.clone(),
                read_context(tenant_id, allowed_user_id, None, "route-channel-miss"),
                topic_id,
                Some("en"),
            )
            .await
            .expect("route-channel miss should resolve as absent")
            .is_none()
    );
    assert_eq!(
        requests
            .lock()
            .expect("facts request record should lock")
            .len(),
        calls_before_base_rejection,
        "base route visibility must reject before richer owner facts"
    );

    assert!(matches!(
        ForumTopicAudienceReadService::new(db.clone(), event_bus)
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                allowed_security.clone(),
                read_context(tenant_id, allowed_user_id, Some("web"), "missing-provider"),
                topic_id,
                Some("en"),
            )
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));

    let calls_before_context_rejection = requests
        .lock()
        .expect("facts request record should lock")
        .len();
    assert!(matches!(
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                allowed_security.clone(),
                read_context(
                    foreign_tenant_id,
                    allowed_user_id,
                    Some("web"),
                    "foreign-tenant-context",
                ),
                topic_id,
                Some("en"),
            )
            .await,
        Err(ForumError::Validation(message)) if message.contains("tenant does not match")
    ));
    assert!(matches!(
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                allowed_security,
                read_context(
                    tenant_id,
                    Uuid::new_v4(),
                    Some("web"),
                    "foreign-actor-context",
                ),
                topic_id,
                Some("en"),
            )
            .await,
        Err(ForumError::Validation(message)) if message.contains("actor does not match")
    ));
    assert_eq!(
        requests
            .lock()
            .expect("facts request record should lock")
            .len(),
        calls_before_context_rejection,
        "invalid exact context must fail before optional facts access"
    );

    assert!(
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                SecurityContext::new(UserRole::Customer, Some(allowed_user_id)),
                read_context(tenant_id, allowed_user_id, Some("web"), "missing-topic"),
                Uuid::new_v4(),
                Some("en"),
            )
            .await
            .expect("missing topic should resolve as absent")
            .is_none()
    );
}
