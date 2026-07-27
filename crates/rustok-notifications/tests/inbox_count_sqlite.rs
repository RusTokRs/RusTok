use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::MigrationSource;
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    NotificationError, NotificationInboxStateRequest, NotificationInboxStateService,
    NotificationInboxUnreadCount, NotificationInboxUnreadCountRequest,
    NotificationInboxUnreadCountService, NotificationsModule,
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
async fn count_tracks_exact_recipient_unread_owner_state() {
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
    seed_unread_notification(&db, Uuid::from_u128(14), tenant_id, other_recipient_id).await;
    seed_unread_notification(
        &db,
        Uuid::from_u128(15),
        other_tenant_id,
        other_tenant_recipient_id,
    )
    .await;

    let state = NotificationInboxStateService::new(db.clone());
    state
        .mark_seen(state_request(tenant_id, recipient_id, seen_id))
        .await
        .expect("seen fixture should transition");
    state
        .mark_read(state_request(tenant_id, recipient_id, read_id))
        .await
        .expect("read fixture should transition");
    state
        .archive(state_request(tenant_id, recipient_id, archived_id))
        .await
        .expect("archived fixture should transition");

    let stored_before = notification::Entity::find_by_id(unread_id)
        .one(&db)
        .await
        .expect("unread fixture lookup should succeed")
        .expect("unread fixture should exist");
    let counts = NotificationInboxUnreadCountService::new(db.clone());
    assert_eq!(
        count_unread(&counts, tenant_id, recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 1 }
    );
    assert_eq!(
        count_unread(&counts, tenant_id, other_recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 1 }
    );
    assert_eq!(
        count_unread(&counts, other_tenant_id, other_tenant_recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 1 }
    );

    let stored_after = notification::Entity::find_by_id(unread_id)
        .one(&db)
        .await
        .expect("unread fixture lookup should succeed")
        .expect("unread fixture should exist");
    assert_eq!(stored_after, stored_before);

    state
        .mark_unread(state_request(tenant_id, recipient_id, seen_id))
        .await
        .expect("seen fixture should reopen as unread");
    assert_eq!(
        count_unread(&counts, tenant_id, recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 2 }
    );

    state
        .mark_unread(state_request(tenant_id, recipient_id, read_id))
        .await
        .expect("read fixture should reopen as unread");
    assert_eq!(
        count_unread(&counts, tenant_id, recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 3 }
    );

    state
        .mark_unread(state_request(tenant_id, recipient_id, archived_id))
        .await
        .expect("archived fixture should remain terminal");
    assert_eq!(
        count_unread(&counts, tenant_id, recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 3 }
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
async fn empty_and_foreign_scopes_return_zero_without_an_oracle() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let other_tenant_id = Uuid::from_u128(21);
    let recipient_id = Uuid::from_u128(22);
    let other_recipient_id = Uuid::from_u128(23);
    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, other_tenant_id, other_recipient_id).await;
    seed_unread_notification(&db, Uuid::from_u128(24), tenant_id, recipient_id).await;

    let counts = NotificationInboxUnreadCountService::new(db);
    assert_eq!(
        count_unread(&counts, other_tenant_id, other_recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 0 }
    );
    assert_eq!(
        count_unread(&counts, other_tenant_id, recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 0 }
    );
    assert_eq!(
        count_unread(&counts, tenant_id, other_recipient_id).await,
        NotificationInboxUnreadCount { unread_count: 0 }
    );
}

#[tokio::test]
async fn nil_count_identity_is_rejected() {
    let db = setup().await;
    let counts = NotificationInboxUnreadCountService::new(db);
    for request in [
        NotificationInboxUnreadCountRequest {
            tenant_id: Uuid::nil(),
            recipient_id: Uuid::from_u128(30),
        },
        NotificationInboxUnreadCountRequest {
            tenant_id: Uuid::from_u128(31),
            recipient_id: Uuid::nil(),
        },
    ] {
        let error = counts
            .count_unread(request)
            .await
            .expect_err("nil unread count identity must be rejected");
        assert!(matches!(error, NotificationError::Validation(_)));
    }
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
        "sqlite:file:notification_inbox_count_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox count sqlite database should connect");
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
