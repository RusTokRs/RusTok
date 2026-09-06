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
    MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES, MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationError,
    NotificationInboxGroupListRequest, NotificationInboxGroupListService,
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
const GROUP_A: &str = "forum.topic.group-a";
const GROUP_B: &str = "forum.topic.group-b";

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
        "Group inbox listing test source"
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
async fn exact_group_sparse_pages_exclude_other_groups_and_preserve_progress() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let base = fixed_time();
    for (notification_id, target_id, group_key, seconds) in [
        (Uuid::from_u128(5), Uuid::from_u128(105), GROUP_A, 5),
        (Uuid::from_u128(4), Uuid::from_u128(104), GROUP_A, 4),
        (Uuid::from_u128(3), Uuid::from_u128(103), GROUP_B, 3),
        (Uuid::from_u128(2), Uuid::from_u128(102), GROUP_A, 2),
        (Uuid::from_u128(1), Uuid::from_u128(101), GROUP_A, 1),
    ] {
        seed_notification(
            &db,
            notification_id,
            tenant_id,
            recipient_id,
            target_id,
            group_key,
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
        BTreeSet::from([Uuid::from_u128(104)]),
        BTreeSet::new(),
        policy_calls.clone(),
        source_calls.clone(),
    );

    let first = service
        .list_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            None,
            None,
            2,
        ))
        .await
        .expect("first sparse group page should load");
    assert!(first.items.is_empty());
    assert!(first.has_more);
    let cursor = first
        .next_cursor
        .expect("suppressed raw group page should still advance");

    let second = service
        .list_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            None,
            Some(cursor),
            2,
        ))
        .await
        .expect("terminal group page should load");
    assert_eq!(
        second.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![Uuid::from_u128(2), Uuid::from_u128(1)]
    );
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());
    assert!(
        second
            .items
            .iter()
            .all(|item| item.state == NotificationState::Unread)
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
            Uuid::from_u128(102),
            Uuid::from_u128(101)
        ]
    );
    assert!(!policy_targets.contains(&Uuid::from_u128(103)));
    assert!(!source_targets.contains(&Uuid::from_u128(103)));

    let stored = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("stored notifications should remain readable");
    assert!(
        stored
            .iter()
            .all(|row| row.state == NotificationState::Unread)
    );
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
async fn state_filter_missing_foreign_and_invalid_group_requests_fail_closed() {
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
        GROUP_A,
        NotificationState::Unread,
        base.to_owned() + Duration::seconds(3),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(14),
        tenant_id,
        recipient_id,
        Uuid::from_u128(114),
        GROUP_A,
        NotificationState::Archived,
        base.to_owned() + Duration::seconds(2),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(15),
        tenant_id,
        other_recipient_id,
        Uuid::from_u128(115),
        GROUP_A,
        NotificationState::Unread,
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
        .list_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            Some(NotificationState::Archived),
            None,
            0,
        ))
        .await
        .expect("exact archived group page should load");
    assert_eq!(archived.items.len(), 1);
    assert_eq!(archived.items[0].id, Uuid::from_u128(14));
    assert_eq!(archived.items[0].state, NotificationState::Archived);

    let missing = service
        .list_page(group_request(
            tenant_id,
            recipient_id,
            "forum.topic.missing",
            None,
            None,
            10,
        ))
        .await
        .expect("missing group should be indistinguishably empty");
    assert!(missing.items.is_empty());
    assert!(!missing.has_more);

    let foreign = service
        .list_page(group_request(
            tenant_id,
            other_recipient_id,
            GROUP_B,
            None,
            None,
            10,
        ))
        .await
        .expect("foreign group scope should be indistinguishably empty");
    assert!(foreign.items.is_empty());
    assert!(!foreign.has_more);

    let oversized = "g".repeat(MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES + 1);
    for request in [
        group_request(Uuid::nil(), recipient_id, GROUP_A, None, None, 10),
        group_request(tenant_id, Uuid::nil(), GROUP_A, None, None, 10),
        group_request(tenant_id, recipient_id, "", None, None, 10),
        group_request(tenant_id, recipient_id, " group", None, None, 10),
        group_request(tenant_id, recipient_id, "group\nkey", None, None, 10),
        group_request(tenant_id, recipient_id, oversized.as_str(), None, None, 10),
        group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            None,
            Some("invalid-cursor".to_string()),
            10,
        ),
    ] {
        let error = service
            .list_page(request)
            .await
            .expect_err("invalid group request must fail before authorization");
        assert!(matches!(error, NotificationError::Validation(_)));
    }

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
async fn retryable_group_authorization_failure_aborts_without_partial_result_or_mutation() {
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
        GROUP_A,
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
        GROUP_A,
        NotificationState::Unread,
        base + Duration::seconds(1),
    )
    .await;

    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db.clone(),
        BTreeSet::new(),
        BTreeSet::from([Uuid::from_u128(123)]),
        BTreeSet::new(),
        BTreeSet::new(),
        policy_calls,
        source_calls.clone(),
    );

    let error = service
        .list_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            None,
            None,
            2,
        ))
        .await
        .expect_err("retryable recipient policy failure must abort the group page");
    assert_eq!(error.stable_code(), "NOTIFICATION_RECIPIENT_POLICY_FAILURE");
    assert!(error.is_retryable());
    assert_eq!(
        source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .len(),
        1,
        "the retryable second-row policy failure must stop before its source call"
    );

    let rows = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("notification rows should remain readable");
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| row.state == NotificationState::Unread)
    );
    assert!(rows.iter().all(|row| row.updated_at == row.created_at));
}

#[test]
fn group_listing_limits_reuse_shared_inbox_bounds() {
    assert_eq!(
        group_request(
            Uuid::from_u128(30),
            Uuid::from_u128(31),
            GROUP_A,
            None,
            None,
            0,
        )
        .bounded_limit(),
        20
    );
    assert_eq!(
        group_request(
            Uuid::from_u128(32),
            Uuid::from_u128(33),
            GROUP_A,
            None,
            None,
            u16::MAX,
        )
        .bounded_limit(),
        u64::from(MAX_NOTIFICATION_INBOX_PAGE_SIZE)
    );
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
) -> NotificationInboxGroupListService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(SelectiveSourceProvider {
            unavailable_targets: Arc::new(unavailable_targets),
            retryable_targets: Arc::new(retryable_source_targets),
            calls: source_calls,
        })
        .expect("test source provider should register");
    NotificationInboxGroupListService::new(
        db,
        Arc::new(registry),
        Arc::new(SelectiveRecipientPolicy {
            suppressed_targets: Arc::new(suppressed_targets),
            retryable_targets: Arc::new(retryable_policy_targets),
            calls: policy_calls,
        }),
    )
}

fn group_request(
    tenant_id: Uuid,
    recipient_id: Uuid,
    group_key: &str,
    state: Option<NotificationState>,
    cursor: Option<String>,
    limit: u16,
) -> NotificationInboxGroupListRequest {
    NotificationInboxGroupListRequest {
        tenant_id,
        recipient_id,
        group_key: group_key.to_string(),
        state,
        cursor,
        limit,
    }
}

async fn seed_notification(
    db: &DatabaseConnection,
    notification_id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
    target_id: Uuid,
    group_key: &str,
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
        group_key: Set(Some(group_key.to_string())),
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
        "sqlite:file:notification_inbox_group_listing_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox group listing sqlite database should connect");
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
