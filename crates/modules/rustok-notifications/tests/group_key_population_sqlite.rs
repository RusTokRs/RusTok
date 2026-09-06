use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::MigrationSource;
use rustok_notifications::NotificationsModule;
use rustok_notifications::entities::{delivery_attempt, notification};
use rustok_notifications::model::{NotificationPriorityValue, NotificationState};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE: &str = "test-source";
const TARGET_KIND: &str = "test.target";
const OTHER_TARGET_KIND: &str = "test.other-target";
const NOTIFICATION_TYPE: &str = "test.notification";

#[tokio::test]
async fn migration_backfills_null_group_keys_and_preserves_explicit_keys() {
    let db = setup_platform().await;
    let tenant_id = Uuid::from_u128(1);
    let recipient_id = Uuid::from_u128(2);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let migrations = NotificationsModule.migrations();
    let manager = SchemaManager::new(&db);
    for migration in migrations.iter().take(5) {
        migration
            .up(&manager)
            .await
            .expect("pre-grouping notification migration should apply");
    }

    let target_id = Uuid::from_u128(10);
    let null_group_id = Uuid::from_u128(11);
    let explicit_group_id = Uuid::from_u128(12);
    seed_notification(
        &db,
        null_group_id,
        tenant_id,
        recipient_id,
        SOURCE,
        TARGET_KIND,
        target_id,
        NOTIFICATION_TYPE,
        None,
    )
    .await;
    seed_notification(
        &db,
        explicit_group_id,
        tenant_id,
        recipient_id,
        SOURCE,
        TARGET_KIND,
        Uuid::from_u128(13),
        "test.explicit",
        Some("source-owned-explicit-group"),
    )
    .await;

    migrations[5]
        .up(&manager)
        .await
        .expect("group-key population migration should apply");

    let backfilled = notification::Entity::find_by_id(null_group_id)
        .one(&db)
        .await
        .expect("backfilled notification should load")
        .expect("backfilled notification should exist");
    assert_eq!(
        backfilled.group_key.as_deref(),
        Some(expected_group_key(SOURCE, target_id).as_str())
    );
    assert_eq!(backfilled.state, NotificationState::Unread);
    assert!(backfilled.seen_at.is_none());
    assert!(backfilled.read_at.is_none());
    assert!(backfilled.archived_at.is_none());

    let explicit = notification::Entity::find_by_id(explicit_group_id)
        .one(&db)
        .await
        .expect("explicit notification should load")
        .expect("explicit notification should exist");
    assert_eq!(
        explicit.group_key.as_deref(),
        Some("source-owned-explicit-group")
    );

    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

#[tokio::test]
async fn new_rows_receive_stable_target_group_keys_without_delivery_mutation() {
    let db = setup_platform().await;
    let tenant_id = Uuid::from_u128(20);
    let recipient_id = Uuid::from_u128(21);
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, recipient_id).await;

    let manager = SchemaManager::new(&db);
    for migration in NotificationsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("notification migration should apply");
    }

    let target_id = Uuid::from_u128(30);
    let first_id = Uuid::from_u128(31);
    let second_id = Uuid::from_u128(32);
    let other_owner_id = Uuid::from_u128(33);
    let explicit_id = Uuid::from_u128(34);

    seed_notification(
        &db,
        first_id,
        tenant_id,
        recipient_id,
        SOURCE,
        TARGET_KIND,
        target_id,
        "test.first",
        None,
    )
    .await;
    seed_notification(
        &db,
        second_id,
        tenant_id,
        recipient_id,
        SOURCE,
        OTHER_TARGET_KIND,
        target_id,
        "test.second",
        None,
    )
    .await;
    seed_notification(
        &db,
        other_owner_id,
        tenant_id,
        recipient_id,
        "other-source",
        TARGET_KIND,
        target_id,
        "test.other-owner",
        None,
    )
    .await;
    seed_notification(
        &db,
        explicit_id,
        tenant_id,
        recipient_id,
        SOURCE,
        TARGET_KIND,
        Uuid::from_u128(35),
        "test.explicit-new",
        Some("explicit-new-group"),
    )
    .await;

    let expected = expected_group_key(SOURCE, target_id);
    assert!(expected.len() <= 191);

    let first = load_notification(&db, first_id).await;
    let second = load_notification(&db, second_id).await;
    let other_owner = load_notification(&db, other_owner_id).await;
    let explicit = load_notification(&db, explicit_id).await;

    assert_eq!(first.group_key.as_deref(), Some(expected.as_str()));
    assert_eq!(second.group_key.as_deref(), Some(expected.as_str()));
    assert_eq!(
        other_owner.group_key.as_deref(),
        Some(expected_group_key("other-source", target_id).as_str())
    );
    assert_ne!(first.group_key, other_owner.group_key);
    assert_eq!(explicit.group_key.as_deref(), Some("explicit-new-group"));

    for row in [first, second, other_owner, explicit] {
        assert_eq!(row.state, NotificationState::Unread);
        assert!(row.seen_at.is_none());
        assert!(row.read_at.is_none());
        assert!(row.archived_at.is_none());
    }
    assert_eq!(delivery_count(&db, tenant_id).await, 0);
}

#[allow(clippy::too_many_arguments)]
async fn seed_notification(
    db: &DatabaseConnection,
    notification_id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
    target_owner: &str,
    target_kind: &str,
    target_id: Uuid,
    notification_type: &str,
    group_key: Option<&str>,
) {
    let timestamp = fixed_time();
    notification::ActiveModel {
        id: Set(notification_id),
        tenant_id: Set(tenant_id),
        recipient_id: Set(recipient_id),
        source_slug: Set(target_owner.to_string()),
        source_event_id: Set(Uuid::new_v4()),
        source_revision: Set(1),
        notification_type: Set(notification_type.to_string()),
        template_key: Set(notification_type.to_string()),
        target_owner: Set(target_owner.to_string()),
        target_kind: Set(target_kind.to_string()),
        target_id: Set(target_id),
        actor_id: Set(None),
        priority: Set(NotificationPriorityValue::Normal),
        state: Set(NotificationState::Unread),
        template_data_json: Set(serde_json::json!({"target_id": target_id})),
        group_key: Set(group_key.map(str::to_string)),
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

fn expected_group_key(target_owner: &str, target_id: Uuid) -> String {
    format!("g1:{target_owner}:{target_id}")
}

fn fixed_time() -> DateTime<FixedOffset> {
    DateTime::<Utc>::from_timestamp(1_800_000_000, 123_456_789)
        .expect("test timestamp should stay valid")
        .fixed_offset()
}

async fn setup_platform() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_group_key_population_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification group-key SQLite database should connect");
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
