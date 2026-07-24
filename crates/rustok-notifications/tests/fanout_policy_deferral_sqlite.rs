use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_notifications::api::{
    NotificationSourceEventRef, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTypeKey,
};
use rustok_notifications::entities::source_inbox;
use rustok_notifications::model::NotificationSourceInboxStatus;
use rustok_notifications::{
    NotificationFanoutPolicyDeferral, NotificationFanoutService, NotificationFanoutWorker,
    NotificationsModule,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn tenant_policy_deferral_removes_disabled_work_from_bounded_head() {
    let db = setup().await;
    let first_tenant = Uuid::new_v4();
    let second_tenant = Uuid::new_v4();
    insert_tenant(&db, first_tenant).await;
    insert_tenant(&db, second_tenant).await;

    let registry = Arc::new(NotificationSourceRegistry::default());
    let service = NotificationFanoutService::new(db.clone(), registry.clone());
    service
        .enqueue_source_event(source_event(first_tenant, Uuid::new_v4(), 1))
        .await
        .expect("first source event should enter durable inbox");
    service
        .enqueue_source_event(source_event(second_tenant, Uuid::new_v4(), 1))
        .await
        .expect("second source event should enter durable inbox");

    let worker = NotificationFanoutWorker::new(db.clone(), registry, "policy-worker", 1, 1)
        .expect("policy fanout worker should compose");
    let first_page = worker
        .claimable_source_inbox_work()
        .await
        .expect("first bounded work page should select");
    assert_eq!(first_page.len(), 1);
    let deferred = first_page[0];

    worker
        .defer_source_inbox(deferred, NotificationFanoutPolicyDeferral::TenantDisabled)
        .await
        .expect("disabled tenant work should enter durable retry backoff");

    let deferred_row = source_inbox::Entity::find_by_id(deferred.inbox_id)
        .one(&db)
        .await
        .expect("deferred source query should succeed")
        .expect("deferred source row should exist");
    assert_eq!(
        deferred_row.status,
        NotificationSourceInboxStatus::RetryableError
    );
    assert_eq!(deferred_row.attempt_count, 1);
    assert!(deferred_row.next_attempt_at.is_some());
    assert_eq!(
        deferred_row.last_error_code.as_deref(),
        Some("NOTIFICATION_TENANT_CAPABILITY_DISABLED")
    );
    assert!(deferred_row.lease_owner.is_none());
    assert!(deferred_row.lease_expires_at.is_none());

    let next_page = worker
        .claimable_source_inbox_work()
        .await
        .expect("later enabled work should reach bounded head");
    assert_eq!(next_page.len(), 1);
    assert_ne!(next_page[0].inbox_id, deferred.inbox_id);
    assert_ne!(next_page[0].tenant_id, deferred.tenant_id);

    let future_retry_count = source_inbox::Entity::find()
        .filter(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::RetryableError))
        .count(&db)
        .await
        .expect("retryable source count should succeed");
    assert_eq!(future_retry_count, 1);
}

fn source_event(tenant_id: Uuid, event_id: Uuid, revision: u64) -> NotificationSourceEventRef {
    NotificationSourceEventRef::new(
        tenant_id,
        event_id,
        NotificationSourceSlug::new("policy-source").expect("source slug must stay valid"),
        NotificationTypeKey::new("policy.event").expect("event type must stay valid"),
        revision,
    )
    .expect("source event must stay valid")
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_fanout_policy_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("fanout policy sqlite database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should enable");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL);
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
