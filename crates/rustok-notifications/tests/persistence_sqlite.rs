use rustok_core::MigrationSource;
use rustok_notifications::NotificationsModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn notification_persistence_enforces_sqlite_invariants() {
    let db = setup().await;
    let manager = SchemaManager::new(&db);
    for migration in NotificationsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("notification migration should apply");
    }

    for table in [
        "notifications",
        "notification_delivery_attempts",
        "notification_fanout_jobs",
        "notification_fanout_items",
        "notification_preferences",
        "notification_digest_jobs",
        "notification_digest_items",
        "notification_push_subscriptions",
    ] {
        assert!(manager
            .has_table(table)
            .await
            .expect("table lookup should succeed"));
    }

    let tenant = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    let recipient = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let foreign_user = Uuid::new_v4();
    insert_tenant(&db, tenant).await;
    insert_tenant(&db, foreign_tenant).await;
    insert_user(&db, tenant, recipient).await;
    insert_user(&db, tenant, actor).await;
    insert_user(&db, foreign_tenant, foreign_user).await;

    let notification_id = Uuid::new_v4();
    let source_event_id = Uuid::new_v4();
    insert_notification(
        &db,
        notification_id,
        tenant,
        recipient,
        Some(actor),
        source_event_id,
        "unread",
        None,
        None,
        "{\"topic_id\":\"one\"}",
        "notification-one",
    )
    .await
    .expect("valid notification should persist");

    let duplicate = insert_notification(
        &db,
        Uuid::new_v4(),
        tenant,
        recipient,
        Some(actor),
        source_event_id,
        "unread",
        None,
        None,
        "{\"topic_id\":\"duplicate\"}",
        "notification-two",
    )
    .await;
    assert!(duplicate.is_err(), "source-event recipient dedupe must hold");

    let cross_tenant = insert_notification(
        &db,
        Uuid::new_v4(),
        tenant,
        foreign_user,
        None,
        Uuid::new_v4(),
        "unread",
        None,
        None,
        "{}",
        "cross-tenant-recipient",
    )
    .await;
    assert!(cross_tenant.is_err(), "recipient tenant mismatch must fail");

    let actor_mismatch = insert_notification(
        &db,
        Uuid::new_v4(),
        tenant,
        recipient,
        Some(foreign_user),
        Uuid::new_v4(),
        "unread",
        None,
        None,
        "{}",
        "cross-tenant-actor",
    )
    .await;
    assert!(actor_mismatch.is_err(), "actor tenant mismatch must fail");

    let read_without_seen = insert_notification(
        &db,
        Uuid::new_v4(),
        tenant,
        recipient,
        None,
        Uuid::new_v4(),
        "read",
        None,
        Some("2026-07-21T12:00:00Z"),
        "{}",
        "read-without-seen",
    )
    .await;
    assert!(read_without_seen.is_err(), "read must imply seen");

    let oversized = serde_json::json!({"value": "x".repeat(8300)}).to_string();
    let oversized_payload = insert_notification(
        &db,
        Uuid::new_v4(),
        tenant,
        recipient,
        None,
        Uuid::new_v4(),
        "unread",
        None,
        None,
        &oversized,
        "oversized-payload",
    )
    .await;
    assert!(oversized_payload.is_err(), "payload bound must hold");

    let invalid_delivery = db
        .execute_unprepared(&format!(
            "INSERT INTO notification_delivery_attempts \
             (id, tenant_id, notification_id, recipient_id, channel, status, idempotency_key, attempt_count) \
             VALUES ('{}', '{}', '{}', '{}', 'email', 'leased', 'delivery-one', 0)",
            Uuid::new_v4(), tenant, notification_id, recipient
        ))
        .await;
    assert!(invalid_delivery.is_err(), "leased delivery needs lease fields");

    let fanout_job_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO notification_fanout_jobs \
         (id, tenant_id, source_slug, source_event_id, source_revision, notification_type, descriptor_json, status, attempt_count) \
         VALUES ('{}', '{}', 'forum', '{}', 1, 'forum.topic.created', '{{}}', 'pending', 0)",
        fanout_job_id,
        tenant,
        Uuid::new_v4()
    ))
    .await
    .expect("valid fanout job should persist");

    let cross_tenant_item = db
        .execute_unprepared(&format!(
            "INSERT INTO notification_fanout_items \
             (id, tenant_id, fanout_job_id, recipient_id, status, idempotency_key) \
             VALUES ('{}', '{}', '{}', '{}', 'pending', 'fanout-cross-tenant')",
            Uuid::new_v4(), tenant, fanout_job_id, foreign_user
        ))
        .await;
    assert!(cross_tenant_item.is_err(), "fanout recipient tenant mismatch must fail");

    db.execute_unprepared(&format!(
        "INSERT INTO notification_preferences \
         (id, tenant_id, user_id, source_scope, type_scope, delivery_mode, digest_mode, timezone, revision) \
         VALUES ('{}', '{}', '{}', '*', '*', 'instant', 'daily', 'UTC', 1)",
        Uuid::new_v4(), tenant, recipient
    ))
    .await
    .expect("default preference should persist");
    let duplicate_preference = db
        .execute_unprepared(&format!(
            "INSERT INTO notification_preferences \
             (id, tenant_id, user_id, source_scope, type_scope, delivery_mode, digest_mode, timezone, revision) \
             VALUES ('{}', '{}', '{}', '*', '*', 'off', 'weekly', 'UTC', 1)",
            Uuid::new_v4(), tenant, recipient
        ))
        .await;
    assert!(duplicate_preference.is_err(), "preference scope must be unique");

    let invalid_push = db
        .execute_unprepared(&format!(
            "INSERT INTO notification_push_subscriptions \
             (id, tenant_id, user_id, platform, endpoint_hash, encrypted_endpoint, key_version, status, failure_count) \
             VALUES ('{}', '{}', '{}', 'web', 'raw-endpoint', 'https://push.invalid', 'v1', 'active', 0)",
            Uuid::new_v4(), tenant, recipient
        ))
        .await;
    assert!(invalid_push.is_err(), "push endpoint hash must be normalized");
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_persistence_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("notification sqlite database should connect");
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
    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES ('{tenant_id}')"
    ))
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

#[allow(clippy::too_many_arguments)]
async fn insert_notification(
    db: &DatabaseConnection,
    id: Uuid,
    tenant_id: Uuid,
    recipient_id: Uuid,
    actor_id: Option<Uuid>,
    source_event_id: Uuid,
    state: &str,
    seen_at: Option<&str>,
    read_at: Option<&str>,
    payload: &str,
    idempotency_key: &str,
) -> Result<(), sea_orm::DbErr> {
    let actor = actor_id
        .map(|id| format!("'{id}'"))
        .unwrap_or_else(|| "NULL".to_string());
    let seen = seen_at
        .map(|value| format!("'{value}'"))
        .unwrap_or_else(|| "NULL".to_string());
    let read = read_at
        .map(|value| format!("'{value}'"))
        .unwrap_or_else(|| "NULL".to_string());
    let payload = payload.replace('\'', "''");
    db.execute_unprepared(&format!(
        "INSERT INTO notifications \
         (id, tenant_id, recipient_id, source_slug, source_event_id, source_revision, notification_type, \
          template_key, target_owner, target_kind, target_id, actor_id, priority, state, template_data_json, \
          idempotency_key, seen_at, read_at) \
         VALUES ('{id}', '{tenant_id}', '{recipient_id}', 'forum', '{source_event_id}', 1, \
          'forum.topic.created', 'forum.topic.created', 'forum', 'forum.topic', '{}', {actor}, \
          'normal', '{state}', '{payload}', '{idempotency_key}', {seen}, {read})",
        Uuid::new_v4()
    ))
    .await
    .map(|_| ())
}
