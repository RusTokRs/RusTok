mod support;

use std::time::Duration;

use chrono::{DateTime, Utc};
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_social_graph::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphModule,
    SocialGraphReceiptCleanupCommand, SocialGraphReceiptMaintenancePort,
    SocialGraphReceiptMaintenanceService, SocialRelationKind,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const AGED_COMPLETION: i64 = 1_500_000_000;
const CLEANUP_CUTOFF: i64 = 1_600_000_000;

#[tokio::test]
async fn cleanup_is_bounded_tenant_scoped_and_completed_only() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();
    let other_source_user_id = Uuid::new_v4();
    let other_target_user_id = Uuid::new_v4();

    insert_tenant(&db, tenant_id).await;
    insert_tenant(&db, other_tenant_id).await;
    insert_user(&db, tenant_id, source_user_id).await;
    insert_user(&db, tenant_id, target_user_id).await;
    insert_user(&db, other_tenant_id, other_source_user_id).await;
    insert_user(&db, other_tenant_id, other_target_user_id).await;

    let relation_service = support::write_service(db.clone());
    SocialGraphCommandPort::set_relation(
        &relation_service,
        write_context(tenant_id, source_user_id, "cleanup-follow"),
        command(source_user_id, target_user_id, true, None),
    )
    .await
    .expect("follow receipt should commit");
    SocialGraphCommandPort::set_relation(
        &relation_service,
        write_context(tenant_id, source_user_id, "cleanup-unfollow"),
        command(source_user_id, target_user_id, false, Some(1)),
    )
    .await
    .expect("unfollow receipt should commit");
    SocialGraphCommandPort::set_relation(
        &relation_service,
        write_context(other_tenant_id, other_source_user_id, "other-tenant-follow"),
        command(other_source_user_id, other_target_user_id, true, None),
    )
    .await
    .expect("other tenant receipt should commit");
    age_completed_receipts(&db, tenant_id).await;
    age_completed_receipts(&db, other_tenant_id).await;
    insert_processing_receipt(&db, tenant_id).await;

    let maintenance = SocialGraphReceiptMaintenanceService::new(db.clone());
    let dry_run = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        maintenance_context(tenant_id, "cleanup-dry-run"),
        cleanup_command(CLEANUP_CUTOFF, 1, true),
    )
    .await
    .expect("dry-run cleanup should inspect one receipt");
    assert_eq!(dry_run.matched_receipts, 1);
    assert_eq!(dry_run.deleted_receipts, 0);
    assert_eq!(
        dry_run.oldest_retained_completed_at_unix_seconds,
        Some(AGED_COMPLETION)
    );
    assert_eq!(receipt_count(&db, tenant_id, "completed").await, 2);

    let first = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        maintenance_context(tenant_id, "cleanup-first"),
        cleanup_command(CLEANUP_CUTOFF, 1, false),
    )
    .await
    .expect("first bounded cleanup should delete one receipt");
    assert_eq!(first.matched_receipts, 1);
    assert_eq!(first.deleted_receipts, 1);
    assert_eq!(
        first.oldest_retained_completed_at_unix_seconds,
        Some(AGED_COMPLETION)
    );
    assert_eq!(receipt_count(&db, tenant_id, "completed").await, 1);

    let second = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        maintenance_context(tenant_id, "cleanup-second"),
        cleanup_command(CLEANUP_CUTOFF, 100, false),
    )
    .await
    .expect("second cleanup should delete the remaining completed receipt");
    assert_eq!(second.matched_receipts, 1);
    assert_eq!(second.deleted_receipts, 1);
    assert_eq!(second.oldest_retained_completed_at_unix_seconds, None);
    assert_eq!(receipt_count(&db, tenant_id, "completed").await, 0);
    assert_eq!(receipt_count(&db, tenant_id, "processing").await, 1);
    assert_eq!(receipt_count(&db, other_tenant_id, "completed").await, 1);
}

#[tokio::test]
async fn cleanup_rejects_user_actor_invalid_limits_and_future_cutoffs() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, user_id).await;
    let maintenance = SocialGraphReceiptMaintenanceService::new(db);

    let forbidden = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        user_maintenance_context(tenant_id, user_id, "cleanup-user"),
        cleanup_command(CLEANUP_CUTOFF, 1, true),
    )
    .await
    .expect_err("user actor must not run receipt cleanup");
    assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
    assert_eq!(forbidden.code, "social_graph.receipt_cleanup_forbidden");

    let invalid = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        maintenance_context(tenant_id, "cleanup-invalid-limit"),
        cleanup_command(CLEANUP_CUTOFF, 0, true),
    )
    .await
    .expect_err("zero cleanup limit must be rejected");
    assert_eq!(invalid.kind, PortErrorKind::Validation);
    assert_eq!(invalid.code, "social_graph.receipt_cleanup_limit_invalid");

    let future = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        maintenance_context(tenant_id, "cleanup-future-cutoff"),
        cleanup_command(Utc::now().timestamp() + 3_600, 1, true),
    )
    .await
    .expect_err("future cleanup cutoff must be rejected");
    assert_eq!(future.kind, PortErrorKind::Validation);
    assert_eq!(future.code, "social_graph.receipt_cleanup_cutoff_future");
}

#[tokio::test]
async fn cleanup_stops_before_deleting_when_one_candidate_is_corrupt() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let source_user_id = Uuid::new_v4();
    let target_user_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    insert_user(&db, tenant_id, source_user_id).await;
    insert_user(&db, tenant_id, target_user_id).await;

    let relation_service = support::write_service(db.clone());
    SocialGraphCommandPort::set_relation(
        &relation_service,
        write_context(tenant_id, source_user_id, "valid-cleanup-receipt"),
        command(source_user_id, target_user_id, true, None),
    )
    .await
    .expect("valid receipt should commit");
    age_completed_receipts(&db, tenant_id).await;
    insert_corrupt_completed_receipt(&db, tenant_id).await;

    let maintenance = SocialGraphReceiptMaintenanceService::new(db.clone());
    let error = SocialGraphReceiptMaintenancePort::cleanup_completed_receipts(
        &maintenance,
        maintenance_context(tenant_id, "cleanup-corrupt-batch"),
        cleanup_command(CLEANUP_CUTOFF, 100, false),
    )
    .await
    .expect_err("corrupt candidate must stop the entire cleanup batch");
    assert_eq!(error.kind, PortErrorKind::InvariantViolation);
    assert_eq!(error.code, "social_graph.command_receipt_corrupt");
    assert_eq!(receipt_count(&db, tenant_id, "completed").await, 2);
}

fn command(
    source_user_id: Uuid,
    target_user_id: Uuid,
    active: bool,
    expected_revision: Option<i64>,
) -> SetSocialRelationCommand {
    SetSocialRelationCommand {
        source_user_id,
        target_user_id,
        relation_kind: SocialRelationKind::Follow,
        active,
        expected_revision,
    }
}

fn cleanup_command(
    completed_before_unix_seconds: i64,
    limit: u32,
    dry_run: bool,
) -> SocialGraphReceiptCleanupCommand {
    SocialGraphReceiptCleanupCommand {
        completed_before_unix_seconds,
        limit,
        dry_run,
    }
}

async fn setup() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite should connect");
    db.execute_unprepared(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL);
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        "#,
    )
    .await
    .expect("identity fixture should migrate");

    support::migrate_outbox(&db).await;
    let manager = SchemaManager::new(&db);
    for migration in SocialGraphModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("social graph migration should apply");
    }
    db
}

async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await
    .expect("tenant fixture should insert");
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        [user_id.into(), tenant_id.into()],
    ))
    .await
    .expect("user fixture should insert");
}

async fn age_completed_receipts(db: &DatabaseConnection, tenant_id: Uuid) {
    let aged = DateTime::<Utc>::from_timestamp(AGED_COMPLETION, 0)
        .expect("aged completion should be valid")
        .fixed_offset();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE social_graph_command_receipts SET completed_at = ?, updated_at = ? WHERE tenant_id = ? AND status = 'completed'",
        [aged.into(), aged.into(), tenant_id.into()],
    ))
    .await
    .expect("completed receipts should age");
}

async fn insert_processing_receipt(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        r#"
        INSERT INTO social_graph_command_receipts (
            id, tenant_id, idempotency_key, schema_version, request_json, status,
            response_json, created_at, updated_at, completed_at
        ) VALUES (?, ?, ?, 1, '{}', 'processing', NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, NULL)
        "#,
        [
            Uuid::new_v4().into(),
            tenant_id.into(),
            "processing-receipt".into(),
        ],
    ))
    .await
    .expect("processing receipt should insert");
}

async fn insert_corrupt_completed_receipt(db: &DatabaseConnection, tenant_id: Uuid) {
    let aged = DateTime::<Utc>::from_timestamp(AGED_COMPLETION, 0)
        .expect("aged completion should be valid")
        .fixed_offset();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        r#"
        INSERT INTO social_graph_command_receipts (
            id, tenant_id, idempotency_key, schema_version, request_json, status,
            response_json, created_at, updated_at, completed_at
        ) VALUES (?, ?, ?, 1, '{}', 'completed', '{}', ?, ?, ?)
        "#,
        [
            Uuid::new_v4().into(),
            tenant_id.into(),
            "corrupt-completed-receipt".into(),
            aged.into(),
            aged.into(),
            aged.into(),
        ],
    ))
    .await
    .expect("corrupt completed receipt should insert");
}

async fn receipt_count(db: &DatabaseConnection, tenant_id: Uuid, status: &str) -> i64 {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS count FROM social_graph_command_receipts WHERE tenant_id = ? AND status = ?",
        [tenant_id.into(), status.into()],
    ))
    .await
    .expect("receipt count should query")
    .expect("receipt count row should exist")
    .try_get::<i64>("", "count")
    .expect("receipt count should decode")
}

fn write_context(tenant_id: Uuid, source_user_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(source_user_id.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(key)
}

fn maintenance_context(tenant_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("receipt-cleanup-test"),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(key)
}

fn user_maintenance_context(tenant_id: Uuid, user_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(key)
}
