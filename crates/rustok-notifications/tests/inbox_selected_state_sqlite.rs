use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::MigrationSource;
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use rustok_notifications::{
    MAX_NOTIFICATION_INBOX_SELECTED_IDS, NotificationError, NotificationInboxSelectedAction,
    NotificationInboxSelectedStateRequest, NotificationInboxSelectedStateResult,
    NotificationInboxSelectedStateService, NotificationInboxStateRequest,
    NotificationInboxStateService, NotificationsModule,
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
async fn selected_actions_delegate_to_exact_state_owner_without_oracles() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    let other_recipient_id = Uuid::from_u128(3);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    insert_user(&db, tenant_id, other_recipient_id).await;

    let unread_id = Uuid::from_u128(10);
    let seen_id = Uuid::from_u128(11);
    let read_id = Uuid::from_u128(12);
    let archived_id = Uuid::from_u128(13);
    let foreign_id = Uuid::from_u128(14);
    for notification_id in [unread_id, seen_id, read_id, archived_id] {
        seed_unread_notification(&db, notification_id, tenant_id, recipient_id).await;
    }
    seed_unread_notification(&db, foreign_id, tenant_id, other_recipient_id).await;

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
        .expect("archive fixture should transition");

    let service = NotificationInboxSelectedStateService::new(db.clone());
    let selected = vec![unread_id, seen_id, read_id, archived_id, foreign_id];

    assert_eq!(
        apply(&service, tenant_id, recipient_id, NotificationInboxSelectedAction::MarkSeen, selected.clone()).await,
        NotificationInboxSelectedStateResult {
            requested: 5,
            changed: 1,
            not_changed: 4,
        }
    );
    assert_eq!(
        apply(&service, tenant_id, recipient_id, NotificationInboxSelectedAction::MarkRead, selected.clone()).await,
        NotificationInboxSelectedStateResult {
            requested: 5,
            changed: 2,
            not_changed: 3,
        }
    );
    assert_eq!(
        apply(&service, tenant_id, recipient_id, NotificationInboxSelectedAction::MarkUnread, selected.clone()).await,
        NotificationInboxSelectedStateResult {
            requested: 5,
            changed: 3,
            not_changed: 2,
        }
    );
    assert_eq!(
        apply(&service, tenant_id, recipient_id, NotificationInboxSelectedAction::Archive, selected).await,
        NotificationInboxSelectedStateResult {
            requested: 5,
            changed: 3,
            not_changed: 2,
        }
    );

    for notification_id in [unread_id, seen_id, read_id, archived_id] {
        assert_eq!(
            load_notification(&db, notification_id).await.state,
            NotificationState::Archived
        );
    }
    assert_eq!(
        load_notification(&db, foreign_id).await.state,
        NotificationState::Unread
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
async fn invalid_selected_state_requests_fail_before_mutation() {
    let db = setup().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    let notification_id = Uuid::from_u128(22);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;
    seed_unread_notification(&db, notification_id, tenant_id, recipient_id).await;

    let service = NotificationInboxSelectedStateService::new(db.clone());
    let oversized = (1..=(MAX_NOTIFICATION_INBOX_SELECTED_IDS + 1))
        .map(|value| Uuid::from_u128(1_000 + value as u128))
        .collect::<Vec<_>>();
    let invalid = [
        NotificationInboxSelectedStateRequest {
            tenant_id: Uuid::nil(),
            recipient_id,
            action: NotificationInboxSelectedAction::MarkRead,
            notification_ids: vec![notification_id],
        },
        NotificationInboxSelectedStateRequest {
            tenant_id,
            recipient_id: Uuid::nil(),
            action: NotificationInboxSelectedAction::MarkRead,
            notification_ids: vec![notification_id],
        },
        NotificationInboxSelectedStateRequest {
            tenant_id,
            recipient_id,
            action: NotificationInboxSelectedAction::MarkRead,
            notification_ids: Vec::new(),
        },
        NotificationInboxSelectedStateRequest {
            tenant_id,
            recipient_id,
            action: NotificationInboxSelectedAction::MarkRead,
            notification_ids: oversized,
        },
        NotificationInboxSelectedStateRequest {
            tenant_id,
            recipient_id,
            action: NotificationInboxSelectedAction::MarkRead,
            notification_ids: vec![Uuid::nil()],
        },
        NotificationInboxSelectedStateRequest {
            tenant_id,
            recipient_id,
            action: NotificationInboxSelectedAction::MarkRead,
            notification_ids: vec![notification_id, notification_id],
        },
    ];

    for request in invalid {
        let error = service
            .apply(request)
            .await
            .expect_err("invalid selected-state request must be rejected");
        assert!(matches!(error, NotificationError::Validation(_)));
    }
    assert_eq!(
        load_notification(&db, notification_id).await.state,
        NotificationState::Unread
    );
}

#[test]
fn selected_state_bound_matches_the_shared_inbox_hard_limit() {
    assert_eq!(MAX_NOTIFICATION_INBOX_SELECTED_IDS, 64);
}

async fn apply(
    service: &NotificationInboxSelectedStateService,
    tenant_id: Uuid,
    recipient_id: Uuid,
    action: NotificationInboxSelectedAction,
    notification_ids: Vec<Uuid>,
) -> NotificationInboxSelectedStateResult {
    service
        .apply(NotificationInboxSelectedStateRequest {
            tenant_id,
            recipient_id,
            action,
            notification_ids,
        })
        .await
        .expect("selected-state owner command should succeed")
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
        "sqlite:file:notification_inbox_selected_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification inbox selected-state sqlite database should connect");
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
