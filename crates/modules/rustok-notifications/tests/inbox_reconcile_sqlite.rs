use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, Utc};
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
    NotificationError, NotificationInboxReconcileRequest, NotificationInboxReconcileService,
    NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
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

#[derive(Clone)]
struct SelectiveRecipientPolicy {
    suppressed_targets: Arc<BTreeSet<Uuid>>,
    retryable_targets: Arc<BTreeSet<Uuid>>,
    calls: Arc<Mutex<Vec<NotificationRecipientPolicyRequest>>>,
}

#[async_trait]
impl NotificationRecipientPolicy for SelectiveRecipientPolicy {
    async fn evaluate(
        &self,
        request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError> {
        self.calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .push(request.clone());
        if self.retryable_targets.contains(&request.target.id) {
            return Err(NotificationRecipientPolicyError::retryable());
        }
        if self.suppressed_targets.contains(&request.target.id) {
            return Ok(NotificationRecipientPolicyDecision::Suppress {
                reason: NotificationRecipientSuppression::Blocked,
            });
        }
        Ok(NotificationRecipientPolicyDecision::Allow)
    }
}

#[derive(Clone)]
struct SelectiveSourceProvider {
    unavailable_targets: Arc<BTreeSet<Uuid>>,
    retryable_targets: Arc<BTreeSet<Uuid>>,
    calls: Arc<Mutex<Vec<AuthorizeNotificationTargetRequest>>>,
}

#[async_trait]
impl NotificationSourceProvider for SelectiveSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Inbox reconciliation test source"
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
        if self.retryable_targets.contains(&request.target.id) {
            return Err(NotificationProviderError::CapabilityUnavailable { retryable: true });
        }
        if self.unavailable_targets.contains(&request.target.id) {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }
        Ok(NotificationOpenAuthorization::Allowed {
            route: NotificationTargetRoute::new(format!(
                "/modules/test?target={}",
                request.target.id
            ))
            .expect("test route should remain valid"),
        })
    }
}

#[tokio::test]
async fn bounded_pages_archive_only_currently_unavailable_rows() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let base = fixed_time();
    seed_notification(
        &db,
        Uuid::from_u128(1),
        tenant_id,
        recipient_id,
        Uuid::from_u128(101),
        NotificationState::Archived,
        base.to_owned() + Duration::seconds(6),
    )
    .await;
    for (notification_id, target_id, state, seconds) in [
        (
            Uuid::from_u128(5),
            Uuid::from_u128(105),
            NotificationState::Unread,
            5,
        ),
        (
            Uuid::from_u128(4),
            Uuid::from_u128(104),
            NotificationState::Seen,
            4,
        ),
        (
            Uuid::from_u128(3),
            Uuid::from_u128(103),
            NotificationState::Read,
            3,
        ),
        (
            Uuid::from_u128(2),
            Uuid::from_u128(102),
            NotificationState::Unread,
            2,
        ),
    ] {
        seed_notification(
            &db,
            notification_id,
            tenant_id,
            recipient_id,
            target_id,
            state,
            base.to_owned() + Duration::seconds(seconds),
        )
        .await;
    }

    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db.clone(),
        BTreeSet::from([Uuid::from_u128(104)]),
        BTreeSet::new(),
        BTreeSet::from([Uuid::from_u128(103)]),
        BTreeSet::new(),
        policy_calls.clone(),
        source_calls.clone(),
    );

    let first = service
        .reconcile_page(NotificationInboxReconcileRequest {
            tenant_id,
            recipient_id,
            cursor: None,
            limit: 2,
        })
        .await
        .expect("first reconciliation page should complete");
    assert_eq!(first.scanned, 2);
    assert_eq!(first.archived, 1);
    assert!(first.has_more);
    let cursor = first
        .next_cursor
        .clone()
        .expect("bounded first page should return a cursor");

    let second = service
        .reconcile_page(NotificationInboxReconcileRequest {
            tenant_id,
            recipient_id,
            cursor: Some(cursor),
            limit: 2,
        })
        .await
        .expect("second reconciliation page should complete");
    assert_eq!(second.scanned, 2);
    assert_eq!(second.archived, 1);
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());

    let allowed_newest = load(&db, Uuid::from_u128(5)).await;
    assert_eq!(allowed_newest.state, NotificationState::Unread);
    assert!(allowed_newest.archived_at.is_none());

    let policy_archived = load(&db, Uuid::from_u128(4)).await;
    assert_eq!(policy_archived.state, NotificationState::Archived);
    assert!(policy_archived.seen_at.is_some());
    assert!(policy_archived.read_at.is_none());
    assert!(policy_archived.archived_at.is_some());

    let source_archived = load(&db, Uuid::from_u128(3)).await;
    assert_eq!(source_archived.state, NotificationState::Archived);
    assert!(source_archived.seen_at.is_some());
    assert!(source_archived.read_at.is_some());
    assert!(source_archived.archived_at.is_some());

    let allowed_oldest = load(&db, Uuid::from_u128(2)).await;
    assert_eq!(allowed_oldest.state, NotificationState::Unread);
    assert!(allowed_oldest.archived_at.is_none());

    let already_archived = load(&db, Uuid::from_u128(1)).await;
    assert_eq!(already_archived.state, NotificationState::Archived);

    let policy_targets = policy_calls
        .lock()
        .expect("recipient policy call recorder should stay available")
        .iter()
        .map(|request| request.target.id)
        .collect::<Vec<_>>();
    assert_eq!(
        policy_targets,
        vec![
            Uuid::from_u128(105),
            Uuid::from_u128(104),
            Uuid::from_u128(103),
            Uuid::from_u128(102),
        ]
    );
    let source_targets = source_calls
        .lock()
        .expect("source authorization call recorder should stay available")
        .iter()
        .map(|request| request.target.id)
        .collect::<Vec<_>>();
    assert_eq!(
        source_targets,
        vec![
            Uuid::from_u128(105),
            Uuid::from_u128(103),
            Uuid::from_u128(102),
        ]
    );

    let replay = service
        .reconcile_page(NotificationInboxReconcileRequest {
            tenant_id,
            recipient_id,
            cursor: None,
            limit: 0,
        })
        .await
        .expect("reconciliation replay should remain idempotent");
    assert_eq!(replay.scanned, 2);
    assert_eq!(replay.archived, 0);
    assert!(!replay.has_more);

    assert_eq!(
        delivery_attempt::Entity::find()
            .filter(delivery_attempt::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("delivery attempt count should succeed"),
        0
    );
}

#[tokio::test]
async fn foreign_and_invalid_requests_fail_before_owner_authorization() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(10);
    let recipient_id = Uuid::from_u128(11);
    let other_recipient_id = Uuid::from_u128(12);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;
    seed_notification(
        &db,
        Uuid::from_u128(13),
        tenant_id,
        recipient_id,
        Uuid::from_u128(113),
        NotificationState::Unread,
        fixed_time(),
    )
    .await;

    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db,
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        policy_calls.clone(),
        source_calls.clone(),
    );

    let foreign = service
        .reconcile_page(NotificationInboxReconcileRequest {
            tenant_id,
            recipient_id: other_recipient_id,
            cursor: None,
            limit: 10,
        })
        .await
        .expect("foreign recipient reconciliation should be empty");
    assert_eq!(foreign.scanned, 0);
    assert_eq!(foreign.archived, 0);
    assert!(!foreign.has_more);

    for cursor in [
        "invalid-cursor".to_string(),
        format!("ir1:1:0:{}", Uuid::nil()),
        "x".repeat(129),
    ] {
        let error = service
            .reconcile_page(NotificationInboxReconcileRequest {
                tenant_id,
                recipient_id,
                cursor: Some(cursor),
                limit: 10,
            })
            .await
            .expect_err("invalid reconciliation cursor must fail before owner calls");
        assert!(matches!(error, NotificationError::Validation(_)));
    }

    let error = service
        .reconcile_page(NotificationInboxReconcileRequest {
            tenant_id: Uuid::nil(),
            recipient_id,
            cursor: None,
            limit: 10,
        })
        .await
        .expect_err("nil reconciliation identity must fail before owner calls");
    assert!(matches!(error, NotificationError::Validation(_)));

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

#[tokio::test]
async fn retryable_failure_stops_after_durable_idempotent_progress() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    let base = fixed_time();
    seed_notification(
        &db,
        Uuid::from_u128(22),
        tenant_id,
        recipient_id,
        Uuid::from_u128(122),
        NotificationState::Unread,
        base.to_owned() + Duration::seconds(2),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(23),
        tenant_id,
        recipient_id,
        Uuid::from_u128(123),
        NotificationState::Unread,
        base + Duration::seconds(1),
    )
    .await;

    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let failing = service(
        db.clone(),
        BTreeSet::from([Uuid::from_u128(122)]),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::from([Uuid::from_u128(123)]),
        policy_calls,
        source_calls,
    );
    let error = failing
        .reconcile_page(NotificationInboxReconcileRequest {
            tenant_id,
            recipient_id,
            cursor: None,
            limit: 2,
        })
        .await
        .expect_err("retryable source failure should stop the page");
    assert_eq!(error.stable_code(), "NOTIFICATION_SOURCE_PROVIDER_FAILURE");
    assert!(error.is_retryable());

    let archived = load(&db, Uuid::from_u128(22)).await;
    assert_eq!(archived.state, NotificationState::Archived);
    assert!(archived.archived_at.is_some());
    let pending = load(&db, Uuid::from_u128(23)).await;
    assert_eq!(pending.state, NotificationState::Unread);
    assert!(pending.archived_at.is_none());

    let replay = service(
        db.clone(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    )
    .reconcile_page(NotificationInboxReconcileRequest {
        tenant_id,
        recipient_id,
        cursor: None,
        limit: 2,
    })
    .await
    .expect("restart should skip the already archived row");
    assert_eq!(replay.scanned, 1);
    assert_eq!(replay.archived, 0);
    assert!(!replay.has_more);
}

#[allow(clippy::too_many_arguments)]
fn service(
    db: DatabaseConnection,
    suppressed_targets: BTreeSet<Uuid>,
    retryable_policy_targets: BTreeSet<Uuid>,
    unavailable_targets: BTreeSet<Uuid>,
    retryable_source_targets: BTreeSet<Uuid>,
    policy_calls: Arc<Mutex<Vec<NotificationRecipientPolicyRequest>>>,
    source_calls: Arc<Mutex<Vec<AuthorizeNotificationTargetRequest>>>,
) -> NotificationInboxReconcileService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(SelectiveSourceProvider {
            unavailable_targets: Arc::new(unavailable_targets),
            retryable_targets: Arc::new(retryable_source_targets),
            calls: source_calls,
        })
        .expect("test source provider should register");
    NotificationInboxReconcileService::new(
        db,
        Arc::new(registry),
        Arc::new(SelectiveRecipientPolicy {
            suppressed_targets: Arc::new(suppressed_targets),
            retryable_targets: Arc::new(retryable_policy_targets),
            calls: policy_calls,
        }),
    )
}

async fn seed_notification(
    db: &DatabaseConnection,
    notification_id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
    target_id: Uuid,
    state: NotificationState,
    created_at: DateTime<FixedOffset>,
) {
    let seen_at = matches!(state, NotificationState::Seen | NotificationState::Read)
        .then_some(created_at.to_owned());
    let read_at = matches!(state, NotificationState::Read).then_some(created_at.to_owned());
    let archived_at = matches!(state, NotificationState::Archived).then_some(created_at.to_owned());
    notification::ActiveModel {
        id: Set(notification_id),
        tenant_id: Set(tenant_id),
        recipient_id: Set(recipient_id),
        source_slug: Set(SOURCE.to_string()),
        source_event_id: Set(Uuid::new_v4()),
        source_revision: Set(1),
        notification_type: Set(NOTIFICATION_TYPE.to_string()),
        template_key: Set(NOTIFICATION_TYPE.to_string()),
        target_owner: Set(SOURCE.to_string()),
        target_kind: Set(TARGET_KIND.to_string()),
        target_id: Set(target_id),
        actor_id: Set(None),
        priority: Set(NotificationPriorityValue::Normal),
        state: Set(state),
        template_data_json: Set(serde_json::json!({"target_id": target_id})),
        group_key: Set(None),
        idempotency_key: Set(format!("notification:{notification_id}")),
        seen_at: Set(seen_at),
        read_at: Set(read_at),
        archived_at: Set(archived_at),
        created_at: Set(created_at.to_owned()),
        updated_at: Set(created_at),
    }
    .insert(db)
    .await
    .expect("notification fixture should persist");
}

async fn load(db: &DatabaseConnection, notification_id: Uuid) -> notification::Model {
    notification::Entity::find_by_id(notification_id)
        .one(db)
        .await
        .expect("notification lookup should succeed")
        .expect("notification fixture should remain stored")
}

fn fixed_time() -> DateTime<FixedOffset> {
    DateTime::<Utc>::from_timestamp(1_800_000_000, 123_456_789)
        .expect("test timestamp should be valid")
        .fixed_offset()
}

fn source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(SOURCE).expect("test source slug must remain valid")
}

fn notification_type() -> NotificationTypeKey {
    NotificationTypeKey::new(NOTIFICATION_TYPE).expect("test notification type must remain valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_inbox_reconcile_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox reconciliation sqlite database should connect");
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
