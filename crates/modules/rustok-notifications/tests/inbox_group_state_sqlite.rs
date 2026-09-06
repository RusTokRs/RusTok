use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_core::MigrationSource;
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES, MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationError,
    NotificationInboxGroupStateAction, NotificationInboxGroupStateRequest,
    NotificationInboxGroupStateService, NotificationsModule,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "test-source";
const TARGET_KIND: &str = "test.target";
const GROUP_A: &str = "g1:test-source:00000000-0000-0000-0000-000000000001";
const GROUP_B: &str = "g1:test-source:00000000-0000-0000-0000-000000000002";

#[tokio::test]
async fn bounded_group_mark_read_is_exact_and_cursor_stable() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    let other_recipient_id = Uuid::from_u128(3);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;

    let base = fixed_time();
    seed_notification(
        &db,
        Uuid::from_u128(11),
        tenant_id,
        recipient_id,
        GROUP_A,
        NotificationState::Unread,
        base.to_owned() + Duration::seconds(5),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(12),
        tenant_id,
        recipient_id,
        GROUP_A,
        NotificationState::Seen,
        base.to_owned() + Duration::seconds(4),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(13),
        tenant_id,
        recipient_id,
        GROUP_A,
        NotificationState::Read,
        base.to_owned() + Duration::seconds(3),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(14),
        tenant_id,
        recipient_id,
        GROUP_A,
        NotificationState::Archived,
        base.to_owned() + Duration::seconds(2),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(15),
        tenant_id,
        recipient_id,
        GROUP_B,
        NotificationState::Unread,
        base.to_owned() + Duration::seconds(6),
    )
    .await;
    seed_notification(
        &db,
        Uuid::from_u128(16),
        tenant_id,
        other_recipient_id,
        GROUP_A,
        NotificationState::Unread,
        base + Duration::seconds(7),
    )
    .await;

    let service = NotificationInboxGroupStateService::new(db.clone());
    let first = service
        .apply_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            None,
            1,
        ))
        .await
        .expect("first bounded group mark-read page should succeed");
    assert_eq!(first.scanned, 1);
    assert_eq!(first.changed, 1);
    assert!(first.has_more);
    let cursor = first
        .next_cursor
        .expect("first bounded group mark-read page should continue");

    let second = service
        .apply_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            Some(cursor),
            1,
        ))
        .await
        .expect("second bounded group mark-read page should succeed");
    assert_eq!(second.scanned, 1);
    assert_eq!(second.changed, 1);
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());

    let direct = load_notification(&db, Uuid::from_u128(11)).await;
    assert_eq!(direct.state, NotificationState::Read);
    assert!(direct.seen_at.is_some());
    assert_eq!(direct.seen_at, direct.read_at);

    let from_seen = load_notification(&db, Uuid::from_u128(12)).await;
    assert_eq!(from_seen.state, NotificationState::Read);
    assert_eq!(
        from_seen.seen_at,
        Some(fixed_time() + Duration::seconds(4)),
        "seen-to-read must preserve the existing seen timestamp"
    );
    assert!(from_seen.read_at.is_some());

    assert_eq!(
        load_notification(&db, Uuid::from_u128(13)).await.state,
        NotificationState::Read
    );
    assert_eq!(
        load_notification(&db, Uuid::from_u128(14)).await.state,
        NotificationState::Archived
    );
    assert_eq!(
        load_notification(&db, Uuid::from_u128(15)).await.state,
        NotificationState::Unread,
        "another group must remain unchanged"
    );
    assert_eq!(
        load_notification(&db, Uuid::from_u128(16)).await.state,
        NotificationState::Unread,
        "another recipient must remain unchanged"
    );
    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

#[tokio::test]
async fn group_mark_unread_and_archive_preserve_exact_state_invariants() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let base = fixed_time();
    for (id, group, state, seconds) in [
        (31_u128, GROUP_A, NotificationState::Seen, 5_i64),
        (32, GROUP_A, NotificationState::Read, 4),
        (33, GROUP_A, NotificationState::Unread, 3),
        (34, GROUP_A, NotificationState::Archived, 2),
        (35, GROUP_B, NotificationState::Read, 6),
        (36, GROUP_B, NotificationState::Seen, 5),
    ] {
        seed_notification(
            &db,
            Uuid::from_u128(id),
            tenant_id,
            recipient_id,
            group,
            state,
            base.to_owned() + Duration::seconds(seconds),
        )
        .await;
    }

    let service = NotificationInboxGroupStateService::new(db.clone());
    let unread = service
        .apply_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            NotificationInboxGroupStateAction::MarkUnread,
            None,
            0,
        ))
        .await
        .expect("group mark-unread should succeed");
    assert_eq!(unread.scanned, 2);
    assert_eq!(unread.changed, 2);
    assert!(!unread.has_more);

    for id in [31_u128, 32, 33] {
        let row = load_notification(&db, Uuid::from_u128(id)).await;
        assert_eq!(row.state, NotificationState::Unread);
        assert!(row.seen_at.is_none());
        assert!(row.read_at.is_none());
        assert!(row.archived_at.is_none());
    }
    assert_eq!(
        load_notification(&db, Uuid::from_u128(34)).await.state,
        NotificationState::Archived
    );

    let archived = service
        .apply_page(group_request(
            tenant_id,
            recipient_id,
            GROUP_B,
            NotificationInboxGroupStateAction::Archive,
            None,
            u16::MAX,
        ))
        .await
        .expect("group archive should succeed");
    assert_eq!(archived.scanned, 2);
    assert_eq!(archived.changed, 2);
    assert!(!archived.has_more);

    let archived_read = load_notification(&db, Uuid::from_u128(35)).await;
    assert_eq!(archived_read.state, NotificationState::Archived);
    assert_eq!(
        archived_read.seen_at,
        Some(fixed_time() + Duration::seconds(6))
    );
    assert_eq!(archived_read.read_at, archived_read.seen_at);
    assert!(archived_read.archived_at.is_some());

    let archived_seen = load_notification(&db, Uuid::from_u128(36)).await;
    assert_eq!(archived_seen.state, NotificationState::Archived);
    assert_eq!(
        archived_seen.seen_at,
        Some(fixed_time() + Duration::seconds(5))
    );
    assert!(archived_seen.read_at.is_none());
    assert!(archived_seen.archived_at.is_some());
    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

#[tokio::test]
async fn missing_foreign_and_invalid_group_state_requests_fail_closed() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(40);
    let recipient_id = Uuid::from_u128(41);
    let other_recipient_id = Uuid::from_u128(42);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;
    seed_notification(
        &db,
        Uuid::from_u128(43),
        tenant_id,
        recipient_id,
        GROUP_A,
        NotificationState::Unread,
        fixed_time(),
    )
    .await;

    let service = NotificationInboxGroupStateService::new(db.clone());
    for request in [
        group_request(
            Uuid::nil(),
            recipient_id,
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            None,
            10,
        ),
        group_request(
            tenant_id,
            Uuid::nil(),
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            None,
            10,
        ),
        group_request(
            tenant_id,
            recipient_id,
            "",
            NotificationInboxGroupStateAction::MarkRead,
            None,
            10,
        ),
        group_request(
            tenant_id,
            recipient_id,
            " group",
            NotificationInboxGroupStateAction::MarkRead,
            None,
            10,
        ),
        group_request(
            tenant_id,
            recipient_id,
            "group\nkey",
            NotificationInboxGroupStateAction::Archive,
            None,
            10,
        ),
        group_request(
            tenant_id,
            recipient_id,
            &"g".repeat(MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES + 1),
            NotificationInboxGroupStateAction::MarkUnread,
            None,
            10,
        ),
        group_request(
            tenant_id,
            recipient_id,
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            Some("invalid-cursor".to_string()),
            10,
        ),
    ] {
        let error = service
            .apply_page(request)
            .await
            .expect_err("invalid group-state request must fail before mutation");
        assert!(matches!(error, NotificationError::Validation(_)));
    }

    let missing = service
        .apply_page(group_request(
            tenant_id,
            recipient_id,
            "g1:test-source:00000000-0000-0000-0000-000000000099",
            NotificationInboxGroupStateAction::MarkRead,
            None,
            10,
        ))
        .await
        .expect("missing group should be indistinguishably empty");
    assert_eq!(missing.scanned, 0);
    assert_eq!(missing.changed, 0);
    assert!(!missing.has_more);

    let foreign = service
        .apply_page(group_request(
            tenant_id,
            other_recipient_id,
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            None,
            10,
        ))
        .await
        .expect("foreign group scope should be indistinguishably empty");
    assert_eq!(foreign.scanned, 0);
    assert_eq!(foreign.changed, 0);
    assert!(!foreign.has_more);

    let stored = load_notification(&db, Uuid::from_u128(43)).await;
    assert_eq!(stored.state, NotificationState::Unread);
    assert!(stored.seen_at.is_none());
    assert!(stored.read_at.is_none());
    assert!(stored.archived_at.is_none());
}

#[test]
fn group_state_limits_reuse_shared_inbox_bounds() {
    assert_eq!(
        group_request(
            Uuid::from_u128(50),
            Uuid::from_u128(51),
            GROUP_A,
            NotificationInboxGroupStateAction::Archive,
            None,
            0,
        )
        .bounded_limit(),
        20
    );
    assert_eq!(
        group_request(
            Uuid::from_u128(52),
            Uuid::from_u128(53),
            GROUP_A,
            NotificationInboxGroupStateAction::MarkRead,
            None,
            u16::MAX,
        )
        .bounded_limit(),
        u64::from(MAX_NOTIFICATION_INBOX_PAGE_SIZE)
    );
}

fn group_request(
    tenant_id: Uuid,
    recipient_id: Uuid,
    group_key: &str,
    action: NotificationInboxGroupStateAction,
    cursor: Option<String>,
    limit: u16,
) -> NotificationInboxGroupStateRequest {
    NotificationInboxGroupStateRequest {
        tenant_id,
        recipient_id,
        group_key: group_key.to_string(),
        action,
        cursor,
        limit,
    }
}

async fn seed_notification(
    db: &DatabaseConnection,
    notification_id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
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
        notification_type: Set("test.notification".to_string()),
        template_key: Set("test.notification".to_string()),
        target_owner: Set(SOURCE.to_string()),
        target_kind: Set(TARGET_KIND.to_string()),
        target_id: Set(Uuid::new_v4()),
        actor_id: Set(None),
        priority: Set(NotificationPriorityValue::Normal),
        state: Set(state),
        template_data_json: Set(serde_json::json!({"notification_id": notification_id})),
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
        "sqlite:file:notification_inbox_group_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification group-state SQLite database should connect");
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
