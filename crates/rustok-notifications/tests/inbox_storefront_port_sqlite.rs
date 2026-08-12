use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
use rustok_api::{PortActor, PortContext, PortErrorKind};
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
    NotificationInboxGroupStateAction, NotificationInboxStorefrontGroupItemsRequest,
    NotificationInboxStorefrontGroupStateRequest, NotificationInboxStorefrontGroupSummaryRequest,
    NotificationInboxStorefrontOpenDecision, NotificationInboxStorefrontOpenRequest,
    NotificationInboxStorefrontPort, NotificationInboxStorefrontService,
    NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
    NotificationRecipientPolicyError, NotificationRecipientPolicyRequest, NotificationsModule,
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
const GROUP_A: &str = "g1:test-source:00000000-0000-0000-0000-000000000001";
const GROUP_B: &str = "g1:test-source:00000000-0000-0000-0000-000000000002";

#[derive(Clone)]
struct AllowPolicy;

#[async_trait]
impl NotificationRecipientPolicy for AllowPolicy {
    async fn evaluate(
        &self,
        _request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError> {
        Ok(NotificationRecipientPolicyDecision::Allow)
    }
}

#[derive(Clone)]
struct AllowSource;

#[async_trait]
impl NotificationSourceProvider for AllowSource {
    fn slug(&self) -> NotificationSourceSlug {
        NotificationSourceSlug::new(SOURCE).expect("source slug should remain valid")
    }

    fn display_name(&self) -> &'static str {
        "Storefront port test source"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![
            NotificationTypeKey::new(NOTIFICATION_TYPE)
                .expect("notification type should remain valid"),
        ]
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
async fn storefront_reads_derive_exact_scope_and_delegate_authorized_owners() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let other_tenant_id = Uuid::from_u128(2);
    let recipient_id = Uuid::from_u128(3);
    let other_recipient_id = Uuid::from_u128(4);
    let other_tenant_recipient_id = Uuid::from_u128(5);
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;
    insert_user(&db, other_tenant_id, other_tenant_recipient_id).await;

    let base = fixed_time();
    for (id, row_tenant, row_recipient, group, target, state, seconds) in [
        (
            11_u128,
            tenant_id,
            recipient_id,
            GROUP_A,
            101_u128,
            NotificationState::Unread,
            3_i64,
        ),
        (
            12,
            tenant_id,
            recipient_id,
            GROUP_A,
            102,
            NotificationState::Read,
            2,
        ),
        (
            13,
            tenant_id,
            other_recipient_id,
            GROUP_A,
            103,
            NotificationState::Unread,
            4,
        ),
        (
            14,
            other_tenant_id,
            other_tenant_recipient_id,
            GROUP_B,
            104,
            NotificationState::Unread,
            5,
        ),
    ] {
        seed_notification(
            &db,
            Uuid::from_u128(id),
            row_tenant,
            row_recipient,
            group,
            Uuid::from_u128(target),
            state,
            base.to_owned() + ChronoDuration::seconds(seconds),
        )
        .await;
    }

    let service = service(db.clone());
    let context = read_context(tenant_id, recipient_id);
    let count = service
        .unread_count(context.clone())
        .await
        .expect("exact storefront unread count should load");
    assert_eq!(count.unread_count, 1);

    let summaries = service
        .list_group_summaries(
            context.clone(),
            NotificationInboxStorefrontGroupSummaryRequest {
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("exact storefront group summaries should load");
    assert_eq!(summaries.groups.len(), 1);
    assert_eq!(summaries.groups[0].group_key, GROUP_A);
    assert_eq!(summaries.groups[0].item_count, 2);
    assert_eq!(summaries.groups[0].unread_count, 1);
    assert_eq!(summaries.groups[0].latest_item.id, Uuid::from_u128(11));

    let items = service
        .list_group_items(
            context.clone(),
            NotificationInboxStorefrontGroupItemsRequest {
                group_key: GROUP_A.to_string(),
                state: None,
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("exact storefront group items should load");
    assert_eq!(
        items.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![Uuid::from_u128(11), Uuid::from_u128(12)]
    );

    let decision = service
        .authorize_open(
            context,
            NotificationInboxStorefrontOpenRequest {
                notification_id: Uuid::from_u128(11),
            },
        )
        .await
        .expect("owned storefront notification should authorize");
    match decision {
        NotificationInboxStorefrontOpenDecision::Allowed { route } => assert_eq!(
            route.as_str(),
            format!("/modules/test?target={}", Uuid::from_u128(101))
        ),
        NotificationInboxStorefrontOpenDecision::Unavailable => {
            panic!("owned storefront notification should remain available")
        }
    }

    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

#[tokio::test]
async fn storefront_writes_require_idempotency_and_preserve_exact_state_invariants() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let base = fixed_time();
    for (id, group, state, seconds) in [
        (31_u128, GROUP_A, NotificationState::Unread, 3_i64),
        (32, GROUP_A, NotificationState::Seen, 2),
        (33, GROUP_B, NotificationState::Unread, 4),
    ] {
        seed_notification(
            &db,
            Uuid::from_u128(id),
            tenant_id,
            recipient_id,
            group,
            Uuid::from_u128(id + 100),
            state,
            base.to_owned() + ChronoDuration::seconds(seconds),
        )
        .await;
    }

    let service = service(db.clone());
    let error = service
        .apply_group_state(
            read_context(tenant_id, recipient_id),
            NotificationInboxStorefrontGroupStateRequest {
                group_key: GROUP_A.to_string(),
                action: NotificationInboxGroupStateAction::MarkRead,
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect_err("write without idempotency key must fail before mutation");
    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "port.idempotency_key_required");
    assert_eq!(
        load_notification(&db, Uuid::from_u128(31)).await.state,
        NotificationState::Unread
    );

    let result = service
        .apply_group_state(
            write_context(tenant_id, recipient_id),
            NotificationInboxStorefrontGroupStateRequest {
                group_key: GROUP_A.to_string(),
                action: NotificationInboxGroupStateAction::MarkRead,
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("idempotent exact-group write should succeed");
    assert_eq!(
        (result.scanned, result.changed, result.has_more),
        (2, 2, false)
    );

    let unread = load_notification(&db, Uuid::from_u128(31)).await;
    assert_eq!(unread.state, NotificationState::Read);
    assert_eq!(unread.seen_at, unread.read_at);
    let seen = load_notification(&db, Uuid::from_u128(32)).await;
    assert_eq!(seen.state, NotificationState::Read);
    assert_eq!(
        seen.seen_at,
        Some(fixed_time() + ChronoDuration::seconds(2))
    );
    assert!(seen.read_at.is_some());
    assert_eq!(
        load_notification(&db, Uuid::from_u128(33)).await.state,
        NotificationState::Unread,
        "another group must remain unchanged"
    );
    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

#[tokio::test]
async fn storefront_scope_policy_and_owner_errors_fail_closed_without_mutation() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(40);
    let recipient_id = Uuid::from_u128(41);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    seed_notification(
        &db,
        Uuid::from_u128(42),
        tenant_id,
        recipient_id,
        GROUP_A,
        Uuid::from_u128(142),
        NotificationState::Unread,
        fixed_time(),
    )
    .await;

    let service = service(db.clone());
    let service_actor = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("storefront-host"),
        "en",
        "corr-service",
    )
    .with_deadline(Duration::from_secs(5));
    let forbidden = service
        .unread_count(service_actor)
        .await
        .expect_err("service actors must not select a user inbox");
    assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
    assert_eq!(forbidden.code, "notifications.storefront.user_required");

    for (context, code) in [
        (
            PortContext::new(
                "not-a-uuid",
                PortActor::user(recipient_id.to_string()),
                "en",
                "corr-tenant",
            )
            .with_deadline(Duration::from_secs(5)),
            "notifications.storefront.tenant_invalid",
        ),
        (
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user("not-a-uuid"),
                "en",
                "corr-user",
            )
            .with_deadline(Duration::from_secs(5)),
            "notifications.storefront.user_invalid",
        ),
    ] {
        let error = service
            .unread_count(context)
            .await
            .expect_err("invalid owner identity must fail before access");
        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, code);
    }

    let no_deadline = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(recipient_id.to_string()),
        "en",
        "corr-deadline",
    );
    let deadline_error = service
        .list_group_summaries(
            no_deadline,
            NotificationInboxStorefrontGroupSummaryRequest {
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect_err("read without deadline semantics must fail");
    assert_eq!(deadline_error.kind, PortErrorKind::Timeout);
    assert_eq!(deadline_error.code, "port.deadline_required");

    let invalid_cursor = service
        .list_group_summaries(
            read_context(tenant_id, recipient_id),
            NotificationInboxStorefrontGroupSummaryRequest {
                cursor: Some("invalid-cursor".to_string()),
                limit: 10,
            },
        )
        .await
        .expect_err("owner validation must map to a safe transport validation error");
    assert_eq!(invalid_cursor.kind, PortErrorKind::Validation);
    assert_eq!(invalid_cursor.code, "NOTIFICATION_VALIDATION_ERROR");
    assert_eq!(
        invalid_cursor.message,
        "notification inbox request is invalid"
    );

    let stored = load_notification(&db, Uuid::from_u128(42)).await;
    assert_eq!(stored.state, NotificationState::Unread);
    assert!(stored.seen_at.is_none());
    assert!(stored.read_at.is_none());
    assert!(stored.archived_at.is_none());
    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

fn service(db: DatabaseConnection) -> NotificationInboxStorefrontService {
    let mut registry = NotificationSourceRegistry::default();
    registry
        .register(AllowSource)
        .expect("test source provider should register");
    NotificationInboxStorefrontService::new(db, Arc::new(registry), Arc::new(AllowPolicy))
}

fn read_context(tenant_id: Uuid, recipient_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(recipient_id.to_string()),
        "en",
        format!("corr-{tenant_id}-{recipient_id}"),
    )
    .with_deadline(Duration::from_secs(5))
}

fn write_context(tenant_id: Uuid, recipient_id: Uuid) -> PortContext {
    read_context(tenant_id, recipient_id).with_idempotency_key(format!(
        "notification-group-state-{tenant_id}-{recipient_id}"
    ))
}

#[allow(clippy::too_many_arguments)]
async fn seed_notification(
    db: &DatabaseConnection,
    notification_id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
    group_key: &str,
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

async fn load_notification(db: &DatabaseConnection, id: Uuid) -> notification::Model {
    notification::Entity::find_by_id(id)
        .one(db)
        .await
        .expect("notification should load")
        .expect("notification should exist")
}

async fn delivery_count(db: &DatabaseConnection, tenant_id: Uuid) -> u64 {
    delivery_attempt::Entity::find()
        .filter(delivery_attempt::Column::TenantId.eq(tenant_id))
        .count(db)
        .await
        .expect("delivery attempt count should succeed")
}

fn fixed_time() -> DateTime<FixedOffset> {
    DateTime::<Utc>::from_timestamp(1_800_000_000, 123_456_789)
        .expect("test timestamp should remain valid")
        .fixed_offset()
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_inbox_storefront_port_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification storefront port SQLite database should connect");
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
