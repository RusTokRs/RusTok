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
    NotificationError, NotificationInboxListRequest, NotificationInboxListService,
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
        "Inbox listing test source"
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
async fn sparse_pages_advance_by_raw_rows_and_return_only_currently_authorized_items() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let base = fixed_time();
    let rows = [
        (Uuid::from_u128(5), Uuid::from_u128(105), 5),
        (Uuid::from_u128(4), Uuid::from_u128(104), 4),
        (Uuid::from_u128(3), Uuid::from_u128(103), 3),
        (Uuid::from_u128(2), Uuid::from_u128(102), 2),
        (Uuid::from_u128(1), Uuid::from_u128(101), 1),
    ];
    for (notification_id, target_id, seconds) in rows {
        seed_notification(
            &db,
            notification_id,
            tenant_id,
            recipient_id,
            target_id,
            NotificationState::Unread,
            base.to_owned() + Duration::seconds(seconds),
        )
        .await;
    }

    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db.clone(),
        BTreeSet::from([Uuid::from_u128(105)]),
        BTreeSet::new(),
        BTreeSet::from([Uuid::from_u128(104), Uuid::from_u128(103)]),
        BTreeSet::new(),
        policy_calls.clone(),
        source_calls.clone(),
    );

    let page_one = service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: None,
            cursor: None,
            limit: 2,
        })
        .await
        .expect("first sparse inbox page should load");
    assert!(page_one.items.is_empty());
    assert!(page_one.has_more);
    let cursor_one = page_one
        .next_cursor
        .clone()
        .expect("empty raw page should still advance its cursor");

    let page_two = service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: None,
            cursor: Some(cursor_one.clone()),
            limit: 2,
        })
        .await
        .expect("second sparse inbox page should load");
    assert_eq!(
        page_two.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![Uuid::from_u128(2)]
    );
    assert!(page_two.has_more);
    let cursor_two = page_two
        .next_cursor
        .clone()
        .expect("second raw page should advance its cursor");
    assert_ne!(cursor_one, cursor_two);

    let page_three = service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: None,
            cursor: Some(cursor_two),
            limit: 2,
        })
        .await
        .expect("terminal inbox page should load");
    assert_eq!(
        page_three
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![Uuid::from_u128(1)]
    );
    assert!(!page_three.has_more);
    assert!(page_three.next_cursor.is_none());

    let returned = page_one
        .items
        .iter()
        .chain(page_two.items.iter())
        .chain(page_three.items.iter())
        .map(|item| item.id)
        .collect::<Vec<_>>();
    assert_eq!(
        returned,
        vec![Uuid::from_u128(2), Uuid::from_u128(1)]
    );
    assert_eq!(page_two.items[0].source, source_slug());
    assert_eq!(page_two.items[0].notification_type, notification_type());
    assert_eq!(page_two.items[0].template_key.as_str(), NOTIFICATION_TYPE);
    assert_eq!(page_two.items[0].state, NotificationState::Unread);
    let expected_target = Uuid::from_u128(102).to_string();
    assert_eq!(
        page_two.items[0].template_data.get("target_id"),
        Some(expected_target.as_str())
    );

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
            Uuid::from_u128(101)
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
            Uuid::from_u128(104),
            Uuid::from_u128(103),
            Uuid::from_u128(102),
            Uuid::from_u128(101)
        ]
    );

    let stored = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .filter(notification::Column::RecipientId.eq(recipient_id))
        .all(&db)
        .await
        .expect("stored notifications should remain readable");
    assert!(stored.iter().all(|row| row.state == NotificationState::Unread));
    assert!(stored.iter().all(|row| row.seen_at.is_none()));
    assert!(stored.iter().all(|row| row.read_at.is_none()));
    assert!(stored.iter().all(|row| row.archived_at.is_none()));
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
async fn state_filter_foreign_recipient_and_invalid_cursor_fail_closed_before_authorization() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(10);
    let recipient_id = Uuid::from_u128(11);
    let other_recipient_id = Uuid::from_u128(12);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;

    let base = fixed_time();
    seed_notification(
        &db,
        Uuid::from_u128(13),
        tenant_id,
        recipient_id,
        Uuid::from_u128(113),
        NotificationState::Unread,
        base.to_owned() + Duration::seconds(2),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(14),
        tenant_id,
        recipient_id,
        Uuid::from_u128(114),
        NotificationState::Archived,
        base + Duration::seconds(1),
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

    let archived = service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: Some(NotificationState::Archived),
            cursor: None,
            limit: 0,
        })
        .await
        .expect("exact archived state page should load");
    assert_eq!(archived.items.len(), 1);
    assert_eq!(archived.items[0].id, Uuid::from_u128(14));
    assert_eq!(archived.items[0].state, NotificationState::Archived);
    assert!(!archived.has_more);

    let foreign = service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id: other_recipient_id,
            state: None,
            cursor: None,
            limit: 10,
        })
        .await
        .expect("foreign recipient page should be indistinguishably empty");
    assert!(foreign.items.is_empty());
    assert!(!foreign.has_more);
    assert!(foreign.next_cursor.is_none());

    let error = service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: None,
            cursor: Some("invalid-cursor".to_string()),
            limit: 10,
        })
        .await
        .expect_err("invalid inbox cursor must fail before authorization");
    assert!(matches!(error, NotificationError::Validation(_)));

    assert_eq!(
        policy_calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .len(),
        1
    );
    assert_eq!(
        source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .len(),
        1
    );
}

#[tokio::test]
async fn retryable_policy_and_source_failures_abort_pages_without_mutating_rows() {
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
    let policy_failure_service = service(
        db.clone(),
        BTreeSet::new(),
        BTreeSet::from([Uuid::from_u128(123)]),
        BTreeSet::new(),
        BTreeSet::new(),
        policy_calls.clone(),
        source_calls.clone(),
    );

    let policy_error = policy_failure_service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: None,
            cursor: None,
            limit: 2,
        })
        .await
        .expect_err("retryable recipient policy failure must abort the whole page");
    assert_eq!(
        policy_error.stable_code(),
        "NOTIFICATION_RECIPIENT_POLICY_FAILURE"
    );
    assert!(policy_error.is_retryable());
    assert_eq!(
        policy_calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .len(),
        2
    );
    assert_eq!(
        source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .len(),
        1,
        "the retryable policy failure must stop before the second source call"
    );

    let source_policy_calls = Arc::new(Mutex::new(Vec::new()));
    let retryable_source_calls = Arc::new(Mutex::new(Vec::new()));
    let source_failure_service = service(
        db.clone(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::from([Uuid::from_u128(123)]),
        source_policy_calls.clone(),
        retryable_source_calls.clone(),
    );
    let source_error = source_failure_service
        .list_page(NotificationInboxListRequest {
            tenant_id,
            recipient_id,
            state: None,
            cursor: None,
            limit: 2,
        })
        .await
        .expect_err("retryable source owner failure must abort the whole page");
    assert_eq!(
        source_error.stable_code(),
        "NOTIFICATION_SOURCE_PROVIDER_FAILURE"
    );
    assert!(source_error.is_retryable());
    assert_eq!(
        source_policy_calls
            .lock()
            .expect("recipient policy call recorder should stay available")
            .len(),
        2
    );
    assert_eq!(
        retryable_source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .len(),
        2
    );

    let rows = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("notification rows should remain readable");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.state == NotificationState::Unread));
    assert!(rows.iter().all(|row| row.seen_at.is_none()));
    assert!(rows.iter().all(|row| row.read_at.is_none()));
    assert!(rows.iter().all(|row| row.archived_at.is_none()));
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
) -> NotificationInboxListService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(SelectiveSourceProvider {
            unavailable_targets: Arc::new(unavailable_targets),
            retryable_targets: Arc::new(retryable_source_targets),
            calls: source_calls,
        })
        .expect("test source provider should register");
    NotificationInboxListService::new(
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
    let archived_at =
        matches!(state, NotificationState::Archived).then_some(created_at.to_owned());
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
        "sqlite:file:notification_inbox_listing_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox listing sqlite database should connect");
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
