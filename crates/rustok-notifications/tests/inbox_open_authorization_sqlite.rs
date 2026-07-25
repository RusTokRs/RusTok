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
use rustok_notifications::entities::notification;
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    NotificationError, NotificationInboxOpenDecision, NotificationInboxOpenRequest,
    NotificationInboxOpenService, NotificationsModule,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection,
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
            .expect("inbox open call recorder should stay available")
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

#[tokio::test]
async fn exact_recipient_gets_fresh_route_without_cross_recipient_oracle() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let other_recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;

    let target_id = Uuid::new_v4();
    let notification_id = seed_notification(&db, tenant_id, recipient_id, target_id, SOURCE).await;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db,
        AuthorizationBehavior::Allowed,
        calls.clone(),
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
            assert_eq!(
                route.as_str(),
                format!("/modules/test?target={target_id}")
            );
        }
        NotificationInboxOpenDecision::Unavailable => {
            panic!("exact recipient target should be available")
        }
    }

    let recorded = calls
        .lock()
        .expect("inbox open call recorder should stay available")
        .clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].tenant_id, tenant_id);
    assert_eq!(recorded[0].recipient_id, recipient_id);
    assert_eq!(recorded[0].target.owner, source_slug());
    assert_eq!(recorded[0].target.kind.as_str(), TARGET_KIND);
    assert_eq!(recorded[0].target.id, target_id);

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
        calls
            .lock()
            .expect("inbox open call recorder should stay available")
            .len(),
        1,
        "foreign and missing rows must not invoke a source provider"
    );
}

#[tokio::test]
async fn stale_target_and_retryable_owner_failure_remain_distinct() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let notification_id =
        seed_notification(&db, tenant_id, recipient_id, Uuid::new_v4(), SOURCE).await;
    let request = NotificationInboxOpenRequest {
        tenant_id,
        recipient_id,
        notification_id,
    };

    let unavailable = service(
        db.clone(),
        AuthorizationBehavior::Unavailable,
        Arc::new(Mutex::new(Vec::new())),
    )
    .authorize_open(request.clone())
    .await
    .expect("stale target should fail closed without an error");
    assert_eq!(unavailable, NotificationInboxOpenDecision::Unavailable);

    let retryable = service(
        db,
        AuthorizationBehavior::Error(NotificationProviderError::CapabilityUnavailable {
            retryable: true,
        }),
        Arc::new(Mutex::new(Vec::new())),
    )
    .authorize_open(request)
    .await
    .expect_err("retryable source failure must not become a stale-target deny");
    assert!(matches!(
        retryable,
        NotificationError::ProviderFailure { retryable: true }
    ));
    assert_eq!(
        retryable.stable_code(),
        "NOTIFICATION_SOURCE_PROVIDER_FAILURE"
    );
    assert!(retryable.is_retryable());
}

#[tokio::test]
async fn invalid_stored_source_identity_fails_before_provider_invocation() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let notification_id = seed_notification(
        &db,
        tenant_id,
        recipient_id,
        Uuid::new_v4(),
        "Invalid Source",
    )
    .await;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let error = service(db, AuthorizationBehavior::Allowed, calls.clone())
        .authorize_open(NotificationInboxOpenRequest {
            tenant_id,
            recipient_id,
            notification_id,
        })
        .await
        .expect_err("invalid stored source identity must fail closed");

    assert!(matches!(error, NotificationError::InvalidDescriptor));
    assert!(calls
        .lock()
        .expect("inbox open call recorder should stay available")
        .is_empty());
}

fn service(
    db: DatabaseConnection,
    behavior: AuthorizationBehavior,
    calls: Arc<Mutex<Vec<AuthorizeNotificationTargetRequest>>>,
) -> NotificationInboxOpenService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(TestSourceProvider { behavior, calls })
        .expect("test source provider should register");
    NotificationInboxOpenService::new(db, Arc::new(registry))
}

async fn seed_notification(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    recipient_id: Uuid,
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
        actor_id: Set(None),
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
