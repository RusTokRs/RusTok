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
    MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationError, NotificationInboxGroupSummaryRequest,
    NotificationInboxGroupSummaryService, NotificationRecipientPolicy,
    NotificationRecipientPolicyDecision, NotificationRecipientPolicyError,
    NotificationRecipientPolicyRequest, NotificationRecipientSuppression, NotificationsModule,
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
const GROUP_A: &str = "g1:test-source:00000000-0000-0000-0000-0000000000a1";
const GROUP_B: &str = "g1:test-source:00000000-0000-0000-0000-0000000000b1";
const GROUP_C: &str = "g1:test-source:00000000-0000-0000-0000-0000000000c1";
const GROUP_D: &str = "g1:test-source:00000000-0000-0000-0000-0000000000d1";

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
        "Group inbox summary test source"
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
async fn summaries_count_non_archived_rows_order_latest_and_preserve_sparse_progress() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    let other_recipient_id = Uuid::from_u128(3);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;

    let target_a = Uuid::from_u128(0xa1);
    let target_b = Uuid::from_u128(0xb1);
    let target_c = Uuid::from_u128(0xc1);
    let target_d = Uuid::from_u128(0xd1);
    let base = fixed_time();

    for (id, target, group, state, seconds) in [
        (
            Uuid::from_u128(109),
            target_a,
            GROUP_A,
            NotificationState::Archived,
            9,
        ),
        (
            Uuid::from_u128(108),
            target_a,
            GROUP_A,
            NotificationState::Seen,
            8,
        ),
        (
            Uuid::from_u128(107),
            target_b,
            GROUP_B,
            NotificationState::Unread,
            7,
        ),
        (
            Uuid::from_u128(106),
            target_c,
            GROUP_C,
            NotificationState::Read,
            6,
        ),
        (
            Uuid::from_u128(105),
            target_a,
            GROUP_A,
            NotificationState::Unread,
            5,
        ),
        (
            Uuid::from_u128(104),
            target_b,
            GROUP_B,
            NotificationState::Unread,
            4,
        ),
        (
            Uuid::from_u128(103),
            target_a,
            GROUP_A,
            NotificationState::Read,
            3,
        ),
        (
            Uuid::from_u128(102),
            target_d,
            GROUP_D,
            NotificationState::Archived,
            10,
        ),
    ] {
        seed_notification(
            &db,
            id,
            tenant_id,
            recipient_id,
            target,
            group,
            state,
            base.to_owned() + Duration::seconds(seconds),
        )
        .await;
    }
    seed_notification(
        &db,
        Uuid::from_u128(200),
        tenant_id,
        other_recipient_id,
        Uuid::from_u128(0xee),
        GROUP_A,
        NotificationState::Unread,
        base + Duration::seconds(20),
    )
    .await;

    let policy_calls = Arc::new(Mutex::new(Vec::new()));
    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db.clone(),
        BTreeSet::from([target_c]),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        policy_calls.clone(),
        source_calls.clone(),
    );

    let first = service
        .list_page(summary_request(tenant_id, recipient_id, None, 2))
        .await
        .expect("first summary page should load");
    assert_eq!(first.groups.len(), 2);
    assert!(first.has_more);
    let cursor = first
        .next_cursor
        .expect("raw third group should produce continuation");

    assert_eq!(first.groups[0].group_key, GROUP_A);
    assert_eq!(first.groups[0].item_count, 3);
    assert_eq!(first.groups[0].unread_count, 1);
    assert_eq!(first.groups[0].latest_item.id, Uuid::from_u128(108));
    assert_eq!(first.groups[0].latest_item.state, NotificationState::Seen);

    assert_eq!(first.groups[1].group_key, GROUP_B);
    assert_eq!(first.groups[1].item_count, 2);
    assert_eq!(first.groups[1].unread_count, 2);
    assert_eq!(first.groups[1].latest_item.id, Uuid::from_u128(107));
    assert_eq!(first.groups[1].latest_item.state, NotificationState::Unread);

    let second = service
        .list_page(summary_request(tenant_id, recipient_id, Some(cursor), 2))
        .await
        .expect("suppressed terminal summary page should load");
    assert!(second.groups.is_empty());
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());

    let policy_targets = policy_calls
        .lock()
        .expect("recipient policy call recorder should stay available")
        .iter()
        .map(|request| request.target.id)
        .collect::<Vec<_>>();
    assert_eq!(policy_targets, vec![target_a, target_b, target_c]);
    let source_targets = source_calls
        .lock()
        .expect("source authorization call recorder should stay available")
        .iter()
        .map(|request| request.target.id)
        .collect::<Vec<_>>();
    assert_eq!(source_targets, vec![target_a, target_b]);
    assert!(!policy_targets.contains(&target_d));

    let stored = notification::Entity::find()
        .filter(notification::Column::TenantId.eq(tenant_id))
        .filter(notification::Column::RecipientId.eq(recipient_id))
        .all(&db)
        .await
        .expect("stored notifications should remain readable");
    assert_eq!(stored.len(), 8);
    assert!(stored.iter().all(|row| row.updated_at == row.created_at));
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
async fn missing_foreign_archived_and_invalid_summary_requests_fail_closed() {
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
        Uuid::from_u128(0xd1),
        GROUP_D,
        NotificationState::Archived,
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

    for request in [
        summary_request(tenant_id, recipient_id, None, 10),
        summary_request(tenant_id, other_recipient_id, None, 10),
    ] {
        let page = service
            .list_page(request)
            .await
            .expect("missing or archived-only scope should be empty");
        assert!(page.groups.is_empty());
        assert!(!page.has_more);
        assert!(page.next_cursor.is_none());
    }

    for request in [
        summary_request(Uuid::nil(), recipient_id, None, 10),
        summary_request(tenant_id, Uuid::nil(), None, 10),
        summary_request(
            tenant_id,
            recipient_id,
            Some("invalid-cursor".to_string()),
            10,
        ),
    ] {
        let error = service
            .list_page(request)
            .await
            .expect_err("invalid summary request must fail before authorization");
        assert!(matches!(error, NotificationError::Validation(_)));
    }

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
async fn retryable_summary_authorization_failure_aborts_without_partial_result_or_mutation() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let allowed_target = Uuid::from_u128(0xa2);
    let retryable_target = Uuid::from_u128(0xb2);
    let base = fixed_time();
    seed_notification(
        &db,
        Uuid::from_u128(22),
        tenant_id,
        recipient_id,
        allowed_target,
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
        retryable_target,
        GROUP_B,
        NotificationState::Unread,
        base + Duration::seconds(1),
    )
    .await;

    let source_calls = Arc::new(Mutex::new(Vec::new()));
    let service = service(
        db.clone(),
        BTreeSet::new(),
        BTreeSet::from([retryable_target]),
        BTreeSet::new(),
        BTreeSet::new(),
        Arc::new(Mutex::new(Vec::new())),
        source_calls.clone(),
    );

    let error = service
        .list_page(summary_request(tenant_id, recipient_id, None, 2))
        .await
        .expect_err("retryable recipient policy failure must abort the summary page");
    assert_eq!(error.stable_code(), "NOTIFICATION_RECIPIENT_POLICY_FAILURE");
    assert!(error.is_retryable());
    assert_eq!(
        source_calls
            .lock()
            .expect("source authorization call recorder should stay available")
            .iter()
            .map(|request| request.target.id)
            .collect::<Vec<_>>(),
        vec![allowed_target]
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
fn group_summary_limits_reuse_shared_inbox_bounds() {
    assert_eq!(
        summary_request(Uuid::from_u128(30), Uuid::from_u128(31), None, 0).bounded_limit(),
        20
    );
    assert_eq!(
        summary_request(Uuid::from_u128(32), Uuid::from_u128(33), None, u16::MAX,).bounded_limit(),
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
) -> NotificationInboxGroupSummaryService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(SelectiveSourceProvider {
            unavailable_targets: Arc::new(unavailable_targets),
            retryable_targets: Arc::new(retryable_source_targets),
            calls: source_calls,
        })
        .expect("test source provider should register");
    NotificationInboxGroupSummaryService::new(
        db,
        Arc::new(registry),
        Arc::new(SelectiveRecipientPolicy {
            suppressed_targets: Arc::new(suppressed_targets),
            retryable_targets: Arc::new(retryable_policy_targets),
            calls: policy_calls,
        }),
    )
}

fn summary_request(
    tenant_id: Uuid,
    recipient_id: Uuid,
    cursor: Option<String>,
    limit: u16,
) -> NotificationInboxGroupSummaryRequest {
    NotificationInboxGroupSummaryRequest {
        tenant_id,
        recipient_id,
        cursor,
        limit,
    }
}

#[allow(clippy::too_many_arguments)]
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
        .expect("test timestamp should stay valid")
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
        "sqlite:file:notification_inbox_group_summary_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox group summary SQLite database should connect");
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
