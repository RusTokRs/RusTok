use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::MigrationSource;
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    MAX_NOTIFICATION_INBOX_PAGE_SIZE, NotificationError, NotificationInboxMarkAllArchivePage,
    NotificationInboxMarkAllArchiveRequest, NotificationInboxMarkAllArchiveService,
    NotificationInboxStateDecision, NotificationInboxStateRequest, NotificationInboxStateService,
    NotificationInboxStateSnapshot, NotificationInboxUnreadCount,
    NotificationInboxUnreadCountRequest, NotificationInboxUnreadCountService, NotificationsModule,
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

#[tokio::test]
async fn bounded_page_archives_non_archived_and_preserves_state_history() {
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

    let unread_id = Uuid::from_u128(10);
    let seen_id = Uuid::from_u128(11);
    let read_id = Uuid::from_u128(12);
    let archived_id = Uuid::from_u128(13);
    for notification_id in [unread_id, seen_id, read_id, archived_id] {
        seed_unread_notification(&db, notification_id, tenant_id, recipient_id).await;
    }
    let other_recipient_unread_id = Uuid::from_u128(14);
    seed_unread_notification(
        &db,
        other_recipient_unread_id,
        tenant_id,
        other_recipient_id,
    )
    .await;
    let other_tenant_unread_id = Uuid::from_u128(15);
    seed_unread_notification(
        &db,
        other_tenant_unread_id,
        other_tenant_id,
        other_tenant_recipient_id,
    )
    .await;

    let state = NotificationInboxStateService::new(db.clone());
    let (_, seen_before) = available(
        state
            .mark_seen(state_request(tenant_id, recipient_id, seen_id))
            .await
            .expect("seen fixture should transition"),
    );
    let (_, read_before) = available(
        state
            .mark_read(state_request(tenant_id, recipient_id, read_id))
            .await
            .expect("read fixture should transition"),
    );
    let (_, archived_before) = available(
        state
            .archive(state_request(tenant_id, recipient_id, archived_id))
            .await
            .expect("archived fixture should transition"),
    );

    let service = NotificationInboxMarkAllArchiveService::new(db.clone());
    assert_eq!(
        mark_page(&service, tenant_id, recipient_id, None, 64).await,
        NotificationInboxMarkAllArchivePage {
            scanned: 3,
            marked_archived: 3,
            next_cursor: None,
            has_more: false,
        }
    );

    let unread_after = load_notification(&db, unread_id).await;
    assert_eq!(unread_after.state, NotificationState::Archived);
    assert!(unread_after.seen_at.is_none());
    assert!(unread_after.read_at.is_none());
    assert!(unread_after.archived_at.is_some());

    let seen_after = load_notification(&db, seen_id).await;
    assert_eq!(seen_after.state, NotificationState::Archived);
    assert_eq!(seen_after.seen_at, seen_before.seen_at);
    assert_eq!(seen_after.read_at, seen_before.read_at);
    assert!(seen_after.archived_at.is_some());

    let read_after = load_notification(&db, read_id).await;
    assert_eq!(read_after.state, NotificationState::Archived);
    assert_eq!(read_after.seen_at, read_before.seen_at);
    assert_eq!(read_after.read_at, read_before.read_at);
    assert!(read_after.archived_at.is_some());

    let archived_after = load_notification(&db, archived_id).await;
    assert_eq!(archived_after.state, archived_before.state);
    assert_eq!(archived_after.seen_at, archived_before.seen_at);
    assert_eq!(archived_after.read_at, archived_before.read_at);
    assert_eq!(archived_after.archived_at, archived_before.archived_at);
    assert_eq!(archived_after.updated_at, archived_before.updated_at);

    assert_eq!(
        load_notification(&db, other_recipient_unread_id).await.state,
        NotificationState::Unread
    );
    assert_eq!(
        load_notification(&db, other_tenant_unread_id).await.state,
        NotificationState::Unread
    );

    let counts = NotificationInboxUnreadCountService::new(db.clone());
    assert_eq!(
        count_unread(&counts, tenant_id, recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 0 }
    );
    assert_eq!(
        count_unread(&counts, tenant_id, other_recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 1 }
    );
    assert_eq!(
        count_unread(&counts, other_tenant_id, other_tenant_recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 1 }
    );
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
async fn mark_all_archive_pages_are_bounded_and_resumable() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    for value in 100..105 {
        seed_unread_notification(&db, Uuid::from_u128(value), tenant_id, recipient_id).await;
    }

    let service = NotificationInboxMarkAllArchiveService::new(db.clone());
    let first = mark_page(&service, tenant_id, recipient_id, None, 2).await;
    assert_eq!(first.scanned, 2);
    assert_eq!(first.marked_archived, 2);
    assert!(first.has_more);
    assert!(first.next_cursor.is_some());

    let second = mark_page(
        &service,
        tenant_id,
        recipient_id,
        first.next_cursor,
        2,
    )
    .await;
    assert_eq!(second.scanned, 2);
    assert_eq!(second.marked_archived, 2);
    assert!(second.has_more);
    assert!(second.next_cursor.is_some());

    let third = mark_page(
        &service,
        tenant_id,
        recipient_id,
        second.next_cursor,
        2,
    )
    .await;
    assert_eq!(
        third,
        NotificationInboxMarkAllArchivePage {
            scanned: 1,
            marked_archived: 1,
            next_cursor: None,
            has_more: false,
        }
    );

    assert_eq!(
        mark_page(&service, tenant_id, recipient_id, None, 2).await,
        NotificationInboxMarkAllArchivePage {
            scanned: 0,
            marked_archived: 0,
            next_cursor: None,
            has_more: false,
        }
    );
    assert_eq!(
        count_unread(
            &NotificationInboxUnreadCountService::new(db),
            tenant_id,
            recipient_id,
        )
        .await,
        NotificationInboxUnreadCount { unread_count: 0 }
    );
}

#[tokio::test]
async fn empty_foreign_and_invalid_mark_all_archive_requests_fail_closed() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(30);
    let other_tenant_id = Uuid::from_u128(31);
    let recipient_id = Uuid::from_u128(32);
    let other_recipient_id = Uuid::from_u128(33);
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, other_tenant_id, other_recipient_id).await;
    seed_unread_notification(
        &db,
        Uuid::from_u128(34),
        tenant_id,
        recipient_id,
    )
    .await;

    let service = NotificationInboxMarkAllArchiveService::new(db);
    for (scope_tenant_id, scope_recipient_id) in [
        (other_tenant_id, other_recipient_id),
        (other_tenant_id, recipient_id),
        (tenant_id, other_recipient_id),
    ] {
        assert_eq!(
            mark_page(
                &service,
                scope_tenant_id,
                scope_recipient_id,
                None,
                20,
            )
            .await,
            NotificationInboxMarkAllArchivePage {
                scanned: 0,
                marked_archived: 0,
                next_cursor: None,
                has_more: false,
            }
        );
    }

    for request in [
        NotificationInboxMarkAllArchiveRequest {
            tenant_id: Uuid::nil(),
            recipient_id,
            cursor: None,
            limit: 20,
        },
        NotificationInboxMarkAllArchiveRequest {
            tenant_id,
            recipient_id: Uuid::nil(),
            cursor: None,
            limit: 20,
        },
        NotificationInboxMarkAllArchiveRequest {
            tenant_id,
            recipient_id,
            cursor: Some("invalid-cursor".to_string()),
            limit: 20,
        },
    ] {
        let error = service
            .mark_page(request)
            .await
            .expect_err("invalid mark-all-archive request must be rejected");
        assert!(matches!(error, NotificationError::Validation(_)));
    }
}

#[test]
fn mark_all_archive_limits_use_shared_inbox_bounds() {
    assert_eq!(
        NotificationInboxMarkAllArchiveRequest {
            tenant_id: Uuid::from_u128(40),
            recipient_id: Uuid::from_u128(41),
            cursor: None,
            limit: 0,
        }
        .bounded_limit(),
        20
    );
    assert_eq!(
        NotificationInboxMarkAllArchiveRequest {
            tenant_id: Uuid::from_u128(42),
            recipient_id: Uuid::from_u128(43),
            cursor: None,
            limit: u16::MAX,
        }
        .bounded_limit(),
        u64::from(MAX_NOTIFICATION_INBOX_PAGE_SIZE)
    );
}

async fn mark_page(
    service: &NotificationInboxMarkAllArchiveService,
    tenant_id: Uuid,
    recipient_id: Uuid,
    cursor: Option<String>,
    limit: u16,
) -> NotificationInboxMarkAllArchivePage {
    service
        .mark_page(NotificationInboxMarkAllArchiveRequest {
            tenant_id,
            recipient_id,
            cursor,
            limit,
        })
        .await
        .expect("bounded mark-all-archive page should succeed")
}

async fn count_unread(
    service: &NotificationInboxUnreadCountService,
    tenant_id: Uuid,
    recipient_id: Uuid,
) -> NotificationInboxUnreadCount {
    service
        .count_unread(NotificationInboxUnreadCountRequest {
            tenant_id,
            recipient_id,
        })
        .await
        .expect("exact recipient unread count should succeed")
}

fn state_request(
    tenant_id: Uuid,
    recipient_id: Uuid,
    notification_id: Uuid,
) -> NotificationInboxStateRequest {
    NotificationInboxStateRequest {
        tenant_id,
        recipient_id,
        notification_id,
    }
}

fn available(
    decision: NotificationInboxStateDecision,
) -> (bool, NotificationInboxStateSnapshot) {
    match decision {
        NotificationInboxStateDecision::Available { changed, snapshot } => (changed, snapshot),
        NotificationInboxStateDecision::Unavailable => {
            panic!("exact recipient notification should remain available")
        }
    }
}

async fn load_notification(db: &DatabaseConnection, id: Uuid) -> notification::Model {
    notification::Entity::find_by_id(id)
        .one(db)
        .await
        .expect("notification lookup should succeed")
        .expect("notification fixture should exist")
}

async fn seed_unread_notification(
    db: &DatabaseConnection,
    notification_id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
) {
    let timestamp = fixed_time();
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
        target_id: Set(Uuid::new_v4()),
        actor_id: Set(None),
        priority: Set(NotificationPriorityValue::Normal),
        state: Set(NotificationState::Unread),
        template_data_json: Set(serde_json::json!({"notification_id": notification_id})),
        group_key: Set(None),
        idempotency_key: Set(format!("notification:{notification_id}")),
        seen_at: Set(None),
        read_at: Set(None),
        archived_at: Set(None),
        created_at: Set(timestamp.to_owned()),
        updated_at: Set(timestamp),
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

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_inbox_mark_all_archive_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox mark-all-archive sqlite database should connect");
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
