use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_notifications::api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudiencePage,
    NotificationOpenAuthorization, NotificationProviderError, NotificationProviderResult,
    NotificationSemanticDescriptor, NotificationSourceProvider, NotificationSourceRegistry,
    NotificationSourceSlug, NotificationTargetRoute, NotificationTypeKey,
    ResolveNotificationAudienceRequest,
};
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    NotificationError, NotificationInboxOpenDecision, NotificationInboxOpenRequest,
    NotificationInboxOpenService, NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
    NotificationRecipientPolicyError, NotificationRecipientPolicyRequest,
    NotificationRecipientSuppression, NotificationsModule,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "test-source";
const NOTIFICATION_TYPE: &str = "test.notification";
const TARGET_KIND: &str = "test.target";

#[derive(Clone, Copy)]
enum AuthorizationBehavior {
    Allowed,
    Unavailable,
    Error(NotificationProviderError),
}

#[derive(Clone)]
struct TestSourceProvider {
    behavior: AuthorizationBehavior,
    calls: Arc<Mutex<Vec<AuthorizeNotificationTargetRequest>>>,
}

#[async_trait]
impl NotificationSourceProvider for TestSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Inbox open test source"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![notification_type()]
    }

    async fn describe_event(
        &self,
        _request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        Err(NotificationProviderError::Rejected)
    }

    async fn resolve_audience(
        &self,
        _request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        Err(NotificationProviderError::Rejected)
    }

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        self.calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .push(request.clone());
        match self.behavior {
            AuthorizationBehavior::Allowed => Ok(NotificationOpenAuthorization::Allowed {
                route: NotificationTargetRoute::new(format!(
                    "/modules/test?target={}",
                    request.target.id
                ))
                .expect("test route should remain valid"),
            }),
            AuthorizationBehavior::Unavailable => Ok(NotificationOpenAuthorization::Unavailable),
            AuthorizationBehavior::Error(error) => Err(error),
        }
    }
}

#[derive(Clone)]
struct StaticRecipientPolicy {
    result: Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError>,
    calls: Arc<Mutex<Vec<NotificationRecipientPolicyRequest>>>,
}

#[async_trait]
impl NotificationRecipientPolicy for StaticRecipientPolicy {
    async fn evaluate(
        &self,
        request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError> {
        self.calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .push(request);
        self.result
    }
}

#[tokio::test]
async fn exact_recipient_passes_privacy_then_gets_fresh_route_without_oracle() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let other_recipient_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;

    let target_id = Uuid::new_v4();
    let notification_id = seed_notification(
        &db,
        tenant_id,
        recipient_id,
        Some(actor_id),
        target_id,
        SOURCE,
    )
    .await;
    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db.clone(),
        Ok(NotificationRecipientPolicyDecision::Allow),
        AuthorizationBehavior::Allowed,
        policy_calls.clone(),
        source_calls.clone(),
    );

    let decision = service
        .authorize_open(NotificationInboxOpenRequest {
            tenant_id,
            recipient_id,
            notification_id,
        })
        .await
        .expect("exact recipient target should authorize");
    match decision {
        NotificationInboxOpenDecision::Allowed { route } => {
            assert_eq!(route.as_str(), format!("/modules/test?target={target_id}"));
        }
        NotificationInboxOpenDecision::Unavailable => {
            panic!("exact recipient target should be available")
        }
    }

    let recorded_policy = policy_calls
        .lock()
        .expect("recipient policy call recorder should stay available")
        .clone();
    assert_eq!(recorded_policy.len(), 1);
    assert_eq!(recorded_policy[0].tenant_id, tenant_id);
    assert_eq!(recorded_policy[0].recipient_id, recipient_id);
    assert_eq!(recorded_policy[0].actor_id, Some(actor_id));
    assert_eq!(recorded_policy[0].source_slug, SOURCE);
    assert_eq!(recorded_policy[0].notification_type, NOTIFICATION_TYPE);
    assert_eq!(recorded_policy[0].target.id, target_id);

    let recorded_source = source_calls
        .lock()
        .expect("source authorization call recorder should stay available")
        .clone();
    assert_eq!(recorded_source.len(), 1);
    assert_eq!(recorded_source[0].tenant_id, tenant_id);
    assert_eq!(recorded_source[0].recipient_id, recipient_id);
    assert_eq!(recorded_source[0].target.id, target_id);

    for foreign_request in [
        NotificationInboxOpenRequest {
            tenant_id,
            recipient_id: other_recipient_id,
            notification_id,
        },
        NotificationInboxOpenRequest {
            tenant_id: other_tenant_id,
            recipient_id,
            notification_id,
        },
        NotificationInboxOpenRequest {
            tenant_id,
            recipient_id,
            notification_id: Uuid::new_v4(),
        },
    ] {
        assert_eq!(
            service
                .authorize_open(foreign_request)
                .await
                .expect("foreign or missing notification should fail closed"),
            NotificationInboxOpenDecision::Unavailable
        );
    }
    assert_eq!(
        policy_calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .len(),
        1,
        "foreign and missing rows must not invoke recipient policy"
    );
    assert_eq!(
        source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .len(),
        1,
        "foreign and missing rows must not invoke a source provider"
    );

    let stored = notification::Entity::find_by_id(notification_id)
        .one(&db)
        .await
        .expect("notification read should succeed")
        .expect("notification should remain stored");
    assert_eq!(stored.state, NotificationState::Unread);
    assert!(stored.seen_at.is_none());
    assert!(stored.read_at.is_none());
    assert!(stored.archived_at.is_none());
    assert_eq!(
        delivery_attempt::Entity::find()
            .filter(delivery_attempt::Column::NotificationId.eq(notification_id))
            .count(&db)
            .await
            .expect("delivery attempt count should succeed"),
        0
    );
}

#[tokio::test]
async fn privacy_suppression_and_retryable_failure_stop_before_source_authorization() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    let notification_id =
        seed_notification(&db, tenant_id, recipient_id, None, Uuid::new_v4(), SOURCE).await;
    let request = NotificationInboxOpenRequest {
        tenant_id,
        recipient_id,
        notification_id,
    };

    let suppressed_source_calls = Arc::new(Mutex::new(Vec::new()));
    let suppressed = service(
        db.clone(),
        Ok(NotificationRecipientPolicyDecision::Suppress {
            reason: NotificationRecipientSuppression::Blocked,
        }),
        AuthorizationBehavior::Allowed,
        Arc::new(Mutex::new(Vec::new())),
        suppressed_source_calls.clone(),
    )
    .authorize_open(request.clone())
    .await
    .expect("privacy suppression should fail closed without a source error");
    assert_eq!(suppressed, NotificationInboxOpenDecision::Unavailable);
    assert!(
        suppressed_source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .is_empty(),
        "suppressed recipient must not reach source authorization"
    );

    let retryable_source_calls = Arc::new(Mutex::new(Vec::new()));
    let retryable = service(
        db,
        Err(NotificationRecipientPolicyError::retryable()),
        AuthorizationBehavior::Allowed,
        Arc::new(Mutex::new(Vec::new())),
        retryable_source_calls.clone(),
    )
    .authorize_open(request)
    .await
    .expect_err("retryable privacy failure must not become a permanent deny");
    assert_eq!(
        retryable.stable_code(),
        "NOTIFICATION_RECIPIENT_POLICY_FAILURE"
    );
    assert!(retryable.is_retryable());
    assert!(
        retryable_source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .is_empty(),
        "failed recipient policy must not reach source authorization"
    );
}

#[tokio::test]
async fn stale_target_provider_failure_and_invalid_source_remain_distinct() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let notification_id =
        seed_notification(&db, tenant_id, recipient_id, None, Uuid::new_v4(), SOURCE).await;
    let request = NotificationInboxOpenRequest {
        tenant_id,
        recipient_id,
        notification_id,
    };

    let unavailable = service(
        db.clone(),
        Ok(NotificationRecipientPolicyDecision::Allow),
        AuthorizationBehavior::Unavailable,
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .authorize_open(request.clone())
    .await
    .expect("stale target should fail closed without an error");
    assert_eq!(unavailable, NotificationInboxOpenDecision::Unavailable);

    let provider_error = service(
        db.clone(),
        Ok(NotificationRecipientPolicyDecision::Allow),
        AuthorizationBehavior::Error(NotificationProviderError::CapabilityUnavailable {
            retryable: true,
        }),
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .authorize_open(request)
    .await
    .expect_err("retryable source failure must not become a stale-target deny");
    assert_eq!(
        provider_error.stable_code(),
        "NOTIFICATION_SOURCE_PROVIDER_FAILURE"
    );
    assert!(provider_error.is_retryable());

    let invalid_notification_id = seed_notification(
        &db,
        tenant_id,
        recipient_id,
        None,
        Uuid::new_v4(),
        "Invalid Source",
    )
    .await;
    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let invalid = service(
        db,
        Ok(NotificationRecipientPolicyDecision::Allow),
        AuthorizationBehavior::Allowed,
        policy_calls.clone(),
        source_calls.clone(),
    )
    .authorize_open(NotificationInboxOpenRequest {
        tenant_id,
        recipient_id,
        notification_id: invalid_notification_id,
    })
    .await
    .expect_err("invalid stored source identity must fail closed");
    assert!(matches!(invalid, NotificationError::InvalidDescriptor));
    assert!(
        policy_calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .is_empty()
    );
    assert!(
        source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .is_empty()
    );
}

fn service(
    db: DatabaseConnection,
    policy_result: Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError>,
    behavior: AuthorizationBehavior,
    policy_calls: Arc<Mutex<Vec<NotificationRecipientPolicyRequest>>>,
    source_calls: Arc<Mutex<Vec<AuthorizeNotificationTargetRequest>>>,
) -> NotificationInboxOpenService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(TestSourceProvider {
            behavior,
            calls: source_calls,
        })
        .expect("test source provider should register");
    NotificationInboxOpenService::new(
        db,
        Arc::new(registry),
        Arc::new(StaticRecipientPolicy {
            result: policy_result,
            calls: policy_calls,
        }),
    )
}

async fn seed_notification(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    recipient_id: Uuid,
    actor_id: Option<Uuid>,
    target_id: Uuid,
    source_slug: &str,
) -> Uuid {
    let now = Utc::now().fixed_offset();
    let notification_id = Uuid::new_v4();
    notification::ActiveModel {
        id: Set(notification_id),
        tenant_id: Set(tenant_id),
        recipient_id: Set(recipient_id),
        source_slug: Set(source_slug.to_string()),
        source_event_id: Set(Uuid::new_v4()),
        source_revision: Set(1),
        notification_type: Set(NOTIFICATION_TYPE.to_string()),
        template_key: Set(NOTIFICATION_TYPE.to_string()),
        target_owner: Set(SOURCE.to_string()),
        target_kind: Set(TARGET_KIND.to_string()),
        target_id: Set(target_id),
        actor_id: Set(actor_id),
        priority: Set(NotificationPriorityValue::Normal),
        state: Set(NotificationState::Unread),
        template_data_json: Set(serde_json::json!({"target_id": target_id})),
        group_key: Set(None),
        idempotency_key: Set(format!("notification:{notification_id}")),
        seen_at: Set(None),
        read_at: Set(None),
        archived_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("notification fixture should persist");
    notification_id
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("test source slug must remain valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE).expect("test notification type must remain valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_inbox_open_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox open sqlite database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should enable");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL
        );
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    )
    .await
    .expect("platform identity fixture should apply");
    let manager = SchemaManager::new(&db);
    for migration in NotificationsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("notification migration should apply");
    }
    db
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
        .await
        .expect("tenant fixture should persist");
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute_unprepared(&format!(
        "INSERT INTO users (id, tenant_id) VALUES ('{user_id}', '{tenant_id}')"
    ))
    .await
    .expect("user fixture should persist");
}
