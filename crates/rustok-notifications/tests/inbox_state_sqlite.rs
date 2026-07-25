use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::MigrationSource;
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    NotificationError, NotificationInboxStateDecision, NotificationInboxStateRequest,
    NotificationInboxStateService, NotificationInboxStateSnapshot, NotificationsModule,
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
async fn exact_recipient_transitions_are_monotonic_and_idempotent() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    let notification_id = Uuid::from_u128(3);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    seed_unread_notification(&db, notification_id, tenant_id, recipient_id).await;

    let service = NotificationInboxStateService::new(db.clone());
    let request = NotificationInboxStateRequest {
        tenant_id,
        recipient_id,
        notification_id,
    };

    let (changed, seen) = available(
        service
            .mark_seen(request.clone())
            .await
            .expect("unread notification should become seen"),
    );
    assert!(changed);
    assert_eq!(seen.state, NotificationState::Seen);
    assert!(seen.seen_at.is_some());
    assert!(seen.read_at.is_none());
    assert!(seen.archived_at.is_none());

    let (changed, seen_again) = available(
        service
            .mark_seen(request.clone())
            .await
            .expect("mark seen should be idempotent"),
    );
    assert!(!changed);
    assert_eq!(seen_again, seen);

    let (changed, read) = available(
        service
            .mark_read(request.clone())
            .await
            .expect("seen notification should become read"),
    );
    assert!(changed);
    assert_eq!(read.state, NotificationState::Read);
    assert_eq!(read.seen_at, seen.seen_at);
    assert!(read.read_at.is_some());
    assert!(read.archived_at.is_none());

    let (changed, after_seen) = available(
        service
            .mark_seen(request.clone())
            .await
            .expect("mark seen must not downgrade read state"),
    );
    assert!(!changed);
    assert_eq!(after_seen, read);

    let (changed, archived) = available(
        service
            .archive(request.clone())
            .await
            .expect("read notification should archive"),
    );
    assert!(changed);
    assert_eq!(archived.state, NotificationState::Archived);
    assert_eq!(archived.seen_at, read.seen_at);
    assert_eq!(archived.read_at, read.read_at);
    assert!(archived.archived_at.is_some());

    for decision in [
        service
            .mark_seen(request.clone())
            .await
            .expect("archived notification should not downgrade to seen"),
        service
            .mark_read(request.clone())
            .await
            .expect("archived notification should not downgrade to read"),
        service
            .archive(request)
            .await
            .expect("archive should be idempotent"),
    ] {
        let (changed, snapshot) = available(decision);
        assert!(!changed);
        assert_eq!(snapshot, archived);
    }

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
async fn direct_read_and_archive_preserve_timestamp_invariants() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(10);
    let recipient_id = Uuid::from_u128(11);
    let read_notification_id = Uuid::from_u128(12);
    let archived_notification_id = Uuid::from_u128(13);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    seed_unread_notification(&db, read_notification_id, tenant_id, recipient_id).await;
    seed_unread_notification(
        &db,
        archived_notification_id,
        tenant_id,
        recipient_id,
    )
    .await;

    let service = NotificationInboxStateService::new(db.clone());
    let (changed, read) = available(
        service
            .mark_read(NotificationInboxStateRequest {
                tenant_id,
                recipient_id,
                notification_id: read_notification_id,
            })
            .await
            .expect("unread notification should become directly read"),
    );
    assert!(changed);
    assert_eq!(read.state, NotificationState::Read);
    assert!(read.seen_at.is_some());
    assert_eq!(read.seen_at, read.read_at);
    assert!(read.archived_at.is_none());

    let (changed, archived) = available(
        service
            .archive(NotificationInboxStateRequest {
                tenant_id,
                recipient_id,
                notification_id: archived_notification_id,
            })
            .await
            .expect("unread notification should archive directly"),
    );
    assert!(changed);
    assert_eq!(archived.state, NotificationState::Archived);
    assert!(archived.seen_at.is_none());
    assert!(archived.read_at.is_none());
    assert!(archived.archived_at.is_some());

    let stored_read = notification::Entity::find_by_id(read_notification_id)
        .one(&db)
        .await
        .expect("read notification lookup should succeed")
        .expect("read notification should remain stored");
    assert_eq!(stored_read.state, NotificationState::Read);
    assert_eq!(stored_read.seen_at, stored_read.read_at);

    let stored_archived = notification::Entity::find_by_id(archived_notification_id)
        .one(&db)
        .await
        .expect("archived notification lookup should succeed")
        .expect("archived notification should remain stored");
    assert_eq!(stored_archived.state, NotificationState::Archived);
    assert!(stored_archived.seen_at.is_none());
    assert!(stored_archived.read_at.is_none());
    assert!(stored_archived.archived_at.is_some());
}

#[tokio::test]
async fn foreign_missing_and_invalid_requests_fail_closed_without_mutation() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let other_tenant_id = Uuid::from_u128(21);
    let recipient_id = Uuid::from_u128(22);
    let other_recipient_id = Uuid::from_u128(23);
    let notification_id = Uuid::from_u128(24);
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;
    seed_unread_notification(&db, notification_id, tenant_id, recipient_id).await;

    let service = NotificationInboxStateService::new(db.clone());
    for request in [
        NotificationInboxStateRequest {
            tenant_id,
            recipient_id: other_recipient_id,
            notification_id,
        },
        NotificationInboxStateRequest {
            tenant_id: other_tenant_id,
            recipient_id,
            notification_id,
        },
        NotificationInboxStateRequest {
            tenant_id,
            recipient_id,
            notification_id: Uuid::from_u128(25),
        },
    ] {
        assert_eq!(
            service
                .archive(request)
                .await
                .expect("foreign or missing notification should fail closed"),
            NotificationInboxStateDecision::Unavailable
        );
    }

    for request in [
        NotificationInboxStateRequest {
            tenant_id: Uuid::nil(),
            recipient_id,
            notification_id,
        },
        NotificationInboxStateRequest {
            tenant_id,
            recipient_id: Uuid::nil(),
            notification_id,
        },
        NotificationInboxStateRequest {
            tenant_id,
            recipient_id,
            notification_id: Uuid::nil(),
        },
    ] {
        let error = service
            .mark_seen(request)
            .await
            .expect_err("nil state identity must be rejected");
        assert!(matches!(error, NotificationError::Validation(_)));
    }

    let stored = notification::Entity::find_by_id(notification_id)
        .one(&db)
        .await
        .expect("notification lookup should succeed")
        .expect("notification should remain stored");
    assert_eq!(stored.state, NotificationState::Unread);
    assert!(stored.seen_at.is_none());
    assert!(stored.read_at.is_none());
    assert!(stored.archived_at.is_none());
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
        "sqlite:file:notification_inbox_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox state sqlite database should connect");
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
