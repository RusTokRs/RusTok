use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::entity::{self, SysEventStatus};
use rustok_outbox::{
    OutboxConsumer, OutboxRelay, OutboxRelayPort, OutboxRelayRunOnceRequest, RelayConfig, SysEventsMigration,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, EntityTrait, QueryFilter, Set, Statement, TransactionTrait,
};
use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
use tokio::sync::Semaphore;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
struct MockTransport {
    delivered: Mutex<Vec<Uuid>>,
    remaining_failures: Mutex<HashMap<Uuid, usize>>,
}

#[derive(Default)]
struct TrackingTransport {
    active: AtomicUsize,
    max_active: AtomicUsize,
}

impl TrackingTransport {
    fn max_active(&self) -> usize {
        self.max_active.load(Ordering::SeqCst)
    }
}

impl MockTransport {
    async fn fail_n_times_for(&self, event_id: Uuid, n: usize) {
        self.remaining_failures.lock().await.insert(event_id, n);
    }

    async fn delivered(&self) -> Vec<Uuid> {
        self.delivered.lock().await.clone()
    }
}

#[async_trait]
impl EventTransport for MockTransport {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        let mut remaining_failures = self.remaining_failures.lock().await;
        if let Some(remaining) = remaining_failures.get_mut(&envelope.id)
            && *remaining > 0
        {
            *remaining -= 1;
            return Err(Error::External("temporary transport error".to_string()));
        }

        self.delivered.lock().await.push(envelope.id);
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl EventTransport for TrackingTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

type TestResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct AcceptedThenLocalFailure {
    accepted: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl EventTransport for AcceptedThenLocalFailure {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        let mut accepted = self.accepted.lock().await;
        accepted.push(envelope.id);
        if accepted.len() == 1 {
            return Err(Error::Validation("local projection failed after primary acceptance".to_string()));
        }
        Ok(())
    }
    fn reliability_level(&self) -> ReliabilityLevel { ReliabilityLevel::Streaming }
    fn as_any(&self) -> &dyn Any { self }
}

async fn assert_event_envelope_integrity(db: &DatabaseConnection) -> TestResult<()> {
    let invalid_insert = entity::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(None),
        event_type: Set("forged.failed".to_string()),
        schema_version: Set(1),
        payload: Set(serde_json::json!({"forged": true})),
        status: Set(SysEventStatus::Failed),
        retry_count: Set(1),
        next_attempt_at: Set(None),
        last_error: Set(Some("forged".to_string())),
        claimed_by: Set(None),
        claimed_at: Set(None),
        created_at: Set(Utc::now()),
        dispatched_at: Set(None),
    }.insert(db).await;
    assert!(invalid_insert.is_err(), "events must enter through the pristine pending state");
    let event = seed_event(db).await?;
    let before = entity::Entity::find_by_id(event.id).one(db).await?.expect("event");
    for sql in [
        "UPDATE sys_events SET tenant_id = NULL",
        "UPDATE sys_events SET event_type = 'other.event'",
        "UPDATE sys_events SET schema_version = schema_version + 1",
        "UPDATE sys_events SET payload = '{}'",
        "UPDATE sys_events SET created_at = '2000-01-01T00:00:00+00:00'",
        "UPDATE sys_events SET id = tenant_id",
        "DELETE FROM sys_events",
        "UPDATE sys_events SET retry_count = retry_count + 1",
        "UPDATE sys_events SET next_attempt_at = CURRENT_TIMESTAMP",
        "UPDATE sys_events SET last_error = 'forged'",
        "UPDATE sys_events SET dispatched_at = CURRENT_TIMESTAMP",
        "UPDATE sys_events SET status = 'failed'",
    ] {
        assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    if db.get_database_backend() == DatabaseBackend::Sqlite {
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await?;
        let changed = db.execute_unprepared(
            "INSERT OR REPLACE INTO sys_events (id, tenant_id, event_type, schema_version, payload, status, retry_count, next_attempt_at, last_error, claimed_by, claimed_at, created_at, dispatched_at) SELECT id, tenant_id, 'changed.event', schema_version, payload, status, retry_count, next_attempt_at, last_error, claimed_by, claimed_at, created_at, dispatched_at FROM sys_events"
        ).await?;
        assert_eq!(changed.rows_affected(), 0);
        assert!(db.execute_unprepared(&format!(
            "INSERT OR REPLACE INTO sys_events (id, tenant_id, event_type, schema_version, payload, status, retry_count, created_at) SELECT CASE WHEN typeof(id) = 'blob' THEN lower(hex(id)) ELSE X'{}' END, tenant_id, event_type, schema_version, payload, status, retry_count, created_at FROM sys_events",
            event.id.simple(),
        )).await.is_err());
        assert!(db.execute_unprepared("UPDATE sys_events SET rowid = rowid + 100").await.is_err());
        assert!(db.execute_unprepared(&format!(
            "INSERT OR REPLACE INTO sys_events (rowid, id, event_type, schema_version, payload, status, retry_count, created_at) SELECT rowid, '{}', event_type, schema_version, payload, status, retry_count, created_at FROM sys_events",
            Uuid::new_v4(),
        )).await.is_err());
    }
    assert_eq!(entity::Entity::find_by_id(event.id).one(db).await?, Some(before.clone()));
    let consumer = rustok_outbox::OutboxConsumer::new(db.clone());
    consumer.register_required_event_types("integrity.consumer", &[before.event_type.clone()]).await?;
    let relay = OutboxRelay::new(db.clone(), Arc::new(MockTransport::default()));
    assert_eq!(relay.process_pending_once(None).await?, 1);
    assert!(db.execute_unprepared("DELETE FROM sys_events").await.is_err());
    consumer.acknowledge_existing("integrity.consumer", event.id).await?;
    if db.get_database_backend() == DatabaseBackend::Sqlite {
        let changed = db.execute_unprepared(
            "INSERT OR REPLACE INTO sys_events SELECT * FROM sys_events"
        ).await?;
        assert_eq!(changed.rows_affected(), 0);
        for table in ["outbox_dispatch_attempts", "outbox_delivery_checkpoints", "sys_event_consumer_receipts"] {
            let count: i64 = db.query_one_raw(Statement::from_string(
                db.get_database_backend(), format!("SELECT COUNT(*) AS count FROM {table}"),
            )).await?.expect("history count").try_get("", "count")?;
            assert_eq!(count, 1, "{table}");
        }
    }
    for sql in [
        "UPDATE sys_events SET status = 'pending'",
        "UPDATE sys_events SET retry_count = retry_count + 1",
        "UPDATE sys_events SET next_attempt_at = CURRENT_TIMESTAMP",
        "UPDATE sys_events SET last_error = 'forged'",
        "UPDATE sys_events SET dispatched_at = CURRENT_TIMESTAMP",
    ] {
        assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    let retention = rustok_outbox::OutboxRetention::new(db.clone()).with_config(
        rustok_outbox::OutboxRetentionConfig {
            retention: chrono::Duration::milliseconds(1), ..Default::default()
        },
    );
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(retention.prune_once().await?.pruned, 1);
    Ok(())
}

async fn assert_durable_event_retention_policy(db: &DatabaseConnection) -> TestResult<()> {
    let event = seed_event(db).await?;
    let relay = OutboxRelay::new(db.clone(), Arc::new(MockTransport::default()));
    assert_eq!(relay.process_pending_once(None).await?, 1);
    assert!(db.execute_unprepared("DELETE FROM sys_events").await.is_err(), "unbound policy");
    let retention = rustok_outbox::OutboxRetention::new(db.clone());
    assert_eq!(retention.prune_once().await?.pruned, 0);
    assert!(db.execute_unprepared("DELETE FROM sys_events").await.is_err(), "fresh event");
    let changed = rustok_outbox::OutboxRetention::new(db.clone()).with_config(
        rustok_outbox::OutboxRetentionConfig {
            retention: chrono::Duration::milliseconds(1), ..Default::default()
        },
    );
    assert!(changed.prune_once().await.is_err());
    for sql in [
        "DELETE FROM outbox_event_retention_policy",
        "UPDATE outbox_event_retention_policy SET delivered_retention_ms = NULL",
        "UPDATE outbox_event_retention_policy SET delivered_retention_ms = 0",
        "INSERT INTO outbox_event_retention_policy (id) VALUES (1) ON CONFLICT (id) DO NOTHING",
    ] {
        assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    if db.get_database_backend() == DatabaseBackend::Sqlite {
        for sql in [
            "INSERT OR REPLACE INTO outbox_event_retention_policy VALUES (1, 1, NULL)",
            "UPDATE outbox_event_retention_policy SET delivered_retention_ms = 1.5",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
    } else {
        assert!(db.execute_unprepared("TRUNCATE outbox_event_retention_policy").await.is_err());
    }
    // Coordinated maintenance changes the durable policy, not local config.
    db.execute_unprepared("UPDATE outbox_event_retention_policy SET delivered_retention_ms = 1 WHERE id = 1").await?;
    assert!(retention.prune_once().await.is_err());
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(changed.prune_once().await?.pruned, 1);
    Ok(())
}

#[tokio::test]
async fn sqlite_durable_event_retention_policy() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    assert_durable_event_retention_policy(&db).await
}

#[tokio::test]
async fn postgres_durable_event_retention_policy() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres { return Ok(()) }
    assert_durable_event_retention_policy(&db).await
}

#[tokio::test]
async fn postgres_retention_policy_writer_fences_direct_delete_without_waiting() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres { return Ok(()) }

    let schema = db.query_one_raw(Statement::from_string(
        DatabaseBackend::Postgres, "SELECT current_schema() AS schema_name",
    )).await?.expect("schema").try_get::<String>("", "schema_name")?;
    assert!(schema.starts_with("rustok_outbox_test_"));
    assert!(schema.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
    let database_url = std::env::var("RUSTOK_OUTBOX_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))?;
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let second = Database::connect(options).await?;
    second.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#)).await?;

    let event = seed_event(&db).await?;
    let relay = OutboxRelay::new(db.clone(), Arc::new(MockTransport::default()));
    assert_eq!(relay.process_pending_once(None).await?, 1);
    let retention = rustok_outbox::OutboxRetention::new(db.clone());
    assert_eq!(retention.prune_once().await?.pruned, 0);
    let writer = db.begin().await?;
    writer.execute_unprepared(
        "UPDATE outbox_event_retention_policy SET delivered_retention_ms = 5184000000 WHERE id = 1"
    ).await?;
    let denied = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        second.execute_unprepared("DELETE FROM sys_events"),
    ).await.expect("direct DELETE must not wait on the retention policy writer");
    assert!(denied.is_err());
    writer.commit().await?;
    assert!(second.execute_unprepared("DELETE FROM sys_events").await.is_err(),
        "the committed sixty-day window still holds the fresh event");
    assert!(retention.prune_once().await.is_err(), "old thirty-day configuration");
    assert_eq!(rustok_outbox::OutboxRetention::new(second.clone()).with_config(
        rustok_outbox::OutboxRetentionConfig {
            retention: chrono::Duration::days(60), ..Default::default()
        },
    ).prune_once().await?.pruned, 0);
    assert!(entity::Entity::find_by_id(event.id).one(&second).await?.is_some());
    Ok(())
}

#[tokio::test]
async fn postgres_owner_receipt_integrity_and_retention_lock_order() -> TestResult<()> {
    use rustok_outbox::idempotency::{self, Admission, OwnerOperationScope};
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres { return Ok(()) }
    let schema = db.query_one_raw(Statement::from_string(
        DatabaseBackend::Postgres, "SELECT current_schema() AS schema_name",
    )).await?.expect("schema").try_get::<String>("", "schema_name")?;
    assert!(schema.starts_with("rustok_outbox_test_"));
    assert!(schema.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
    let database_url = std::env::var("RUSTOK_OUTBOX_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))?;
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let second = Database::connect(options).await?;
    second.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#)).await?;

    let scope = OwnerOperationScope::Tenant(Uuid::new_v4());
    let request = serde_json::json!({"request": true});
    let lease = match idempotency::admit(&db, scope, "owner", "terminal", "write", &request).await.unwrap() {
        Admission::Run(lease) => lease,
        _ => panic!("fresh receipt"),
    };
    let pending = match idempotency::admit(&db, scope, "owner", "nonterminal", "write", &request).await.unwrap() {
        Admission::Run(lease) => lease,
        _ => panic!("fresh receipt"),
    };
    assert!(db.execute_unprepared("DELETE FROM owner_operation_receipts").await.is_err());
    assert!(db.execute_unprepared("UPDATE owner_operation_receipts SET request_hash = repeat('a', 64)").await.is_err());
    idempotency::complete(&db, lease, &serde_json::json!({"ok": true})).await.unwrap();
    idempotency::release_retryable(&db, pending,
        &rustok_api::PortError::unavailable("test.retry", "retry")).await.unwrap();

    for sql in [
        "UPDATE owner_operation_receipts SET response_json = '{}' WHERE status = 'completed'",
        "UPDATE owner_operation_receipts SET completed_at = completed_at - interval '1 day' WHERE status = 'completed'",
        "UPDATE owner_operation_receipts SET status = 'processing' WHERE status = 'completed'",
        "TRUNCATE owner_operation_receipts",
        "TRUNCATE outbox_owner_receipt_retention_policy",
        "DELETE FROM owner_operation_receipts",
    ] {
        assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    idempotency::verify_receipt_retention_policy(&db, None).await.unwrap();
    assert!(idempotency::prune_terminal_receipts(&db, chrono::Duration::days(1), 10).await.is_err());
    assert!(db.execute_unprepared("DELETE FROM owner_operation_receipts WHERE status = 'completed'").await.is_err());

    // Simulate coordinated reconfiguration. The direct DELETE obtains the
    // receipt row before its trigger sees the policy; it must not wait.
    let writer = db.begin().await?;
    writer.execute_unprepared(
        "UPDATE outbox_owner_receipt_retention_policy SET retention_ms = 86400000 WHERE id = 1"
    ).await?;
    let denied = tokio::time::timeout(std::time::Duration::from_secs(2),
        second.execute_unprepared("DELETE FROM owner_operation_receipts WHERE status = 'completed'")
    ).await.expect("reverse-order direct DELETE must not wait");
    assert!(denied.is_err());
    writer.commit().await?;
    assert!(second.execute_unprepared("DELETE FROM owner_operation_receipts WHERE status = 'completed'").await.is_err());
    assert!(idempotency::verify_receipt_retention_policy(&second, None).await.is_err());
    assert_eq!(idempotency::prune_terminal_receipts(&second, chrono::Duration::days(1), 10).await.unwrap(), 0);
    assert!(matches!(idempotency::admit(&second, scope, "owner", "terminal", "write", &request).await.unwrap(),
        Admission::Replay(value) if value == serde_json::json!({"ok": true})));
    assert!(matches!(idempotency::admit(&second, scope, "owner", "nonterminal", "write", &request).await.unwrap(),
        Admission::Run(next) if next.operation_id == pending.operation_id && next.token != pending.token));
    Ok(())
}

#[tokio::test]
async fn postgres_consumer_receipt_integrity_fences_direct_registry_and_receipt_dml() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres { return Ok(()) }

    let event = seed_event(&db).await?;
    let consumer = OutboxConsumer::new(db.clone());
    let key = "consumer.database-guard";
    consumer.register_required_event_types(key, &[event.event_type.clone()]).await?;

    assert!(db.execute_unprepared(
        "UPDATE sys_event_consumers SET created_at = CURRENT_TIMESTAMP"
    ).await.is_err(), "registry identity/history cannot be rewritten");
    assert!(db.execute_unprepared(&format!(
        "DELETE FROM sys_event_consumers WHERE consumer_key = '{key}'"
    )).await.is_err(), "raw SQL cannot retire an unacknowledged consumer");
    assert!(db.execute_unprepared("TRUNCATE sys_event_consumers").await.is_err());

    let forged_unregistered = format!(
        "INSERT INTO sys_event_consumer_receipts (outbox_event_id, consumer_key, event_type, acknowledged_at) VALUES ('{}', 'unregistered.consumer', '{}', CURRENT_TIMESTAMP)",
        event.id, event.event_type,
    );
    let forged_wrong_type = format!(
        "INSERT INTO sys_event_consumer_receipts (outbox_event_id, consumer_key, event_type, acknowledged_at) VALUES ('{}', '{key}', 'wrong.event', CURRENT_TIMESTAMP)",
        event.id,
    );
    assert!(db.execute_unprepared(&forged_unregistered).await.is_err());
    assert!(db.execute_unprepared(&forged_wrong_type).await.is_err());

    consumer.acknowledge_existing(key, event.id).await?;
    assert!(db.execute_unprepared(
        "UPDATE sys_event_consumer_receipts SET event_type = 'wrong.event'"
    ).await.is_err(), "terminal ACK identity is immutable");
    assert!(db.execute_unprepared(
        "DELETE FROM sys_event_consumer_receipts"
    ).await.is_err(), "receipt deletion is reserved for source-event retention");
    assert!(db.execute_unprepared("TRUNCATE sys_event_consumer_receipts").await.is_err());

    let retired = db.execute_unprepared(&format!(
        "DELETE FROM sys_event_consumers WHERE consumer_key = '{key}'"
    )).await?;
    assert_eq!(retired.rows_affected(), 1, "a fully drained registration may retire");
    let count = db.query_one_raw(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT COUNT(*) AS count FROM sys_event_consumer_receipts",
    )).await?.expect("receipt count").try_get::<i64>("", "count")?;
    assert_eq!(count, 1, "retirement preserves historical terminal receipts");
    Ok(())
}

#[tokio::test]
async fn sqlite_event_envelope_and_consumer_hold_are_immutable() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    assert_event_envelope_integrity(&db).await
}

#[tokio::test]
async fn postgres_event_envelope_and_consumer_hold_are_immutable() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres { return Ok(()) }
    assert_event_envelope_integrity(&db).await
}

#[tokio::test]
async fn post_acceptance_validation_failure_retries_original_identity() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    assert_dispatch_attempt_history(&db).await
}

#[tokio::test]
async fn postgres_dispatch_attempt_history_and_retention() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres { return Ok(()) }
    assert_dispatch_attempt_history(&db).await
}

async fn assert_dispatch_attempt_history(db: &DatabaseConnection) -> TestResult<()> {
    let event = seed_event(db).await?;
    let transport = Arc::new(AcceptedThenLocalFailure { accepted: Mutex::new(Vec::new()) });
    let relay = OutboxRelay::new(db.clone(), transport.clone()).with_config(RelayConfig {
        max_attempts: 3,
        backoff_base: std::time::Duration::from_millis(1),
        backoff_max: std::time::Duration::from_millis(2),
        ..Default::default()
    });
    assert_eq!(relay.process_pending_once(None).await?, 1);
    let row = entity::Entity::find_by_id(event.id).one(db).await?.expect("event");
    assert_eq!(row.status, SysEventStatus::Pending);
    assert_eq!(row.retry_count, 1);
    assert_eq!(relay.metrics().dlq_total, 0);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(relay.process_pending_once(None).await?, 1);
    let row = entity::Entity::find_by_id(event.id).one(db).await?.expect("event");
    assert_eq!(row.status, SysEventStatus::Dispatched);
    assert_eq!(*transport.accepted.lock().await, vec![event.id, event.id]);
    async fn count_state(db: &DatabaseConnection, state: &str) -> TestResult<i64> {
        let sql = match state {
            "confirmed" => "SELECT COUNT(*) AS count FROM outbox_dispatch_attempts WHERE state = 'confirmed'",
            "uncertain" => "SELECT COUNT(*) AS count FROM outbox_dispatch_attempts WHERE state = 'uncertain'",
            "open" => "SELECT COUNT(*) AS count FROM outbox_dispatch_attempts WHERE state = 'open'",
            _ => unreachable!(),
        };
        Ok(db.query_one_raw(Statement::from_string(db.get_database_backend(), sql))
            .await?.expect("attempt count").try_get("", "count")?)
    }
    assert_eq!(count_state(db, "confirmed").await?, 1);
    assert_eq!(count_state(db, "uncertain").await?, 1);
    assert_eq!(count_state(db, "open").await?, 0);
    // Closed evidence cannot be reopened or erased while its parent exists.
    for sql in [
        "UPDATE outbox_dispatch_attempts SET state = 'open', finished_at = NULL WHERE state = 'uncertain'",
        "UPDATE outbox_dispatch_attempts SET claim_token = 'forged'",
        "DELETE FROM outbox_dispatch_attempts",
        "UPDATE outbox_delivery_checkpoints SET route_key = 'other-route'",
        "DELETE FROM outbox_delivery_checkpoints",
    ] {
        assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    if db.get_database_backend() == DatabaseBackend::Postgres {
        for sql in ["TRUNCATE outbox_dispatch_attempts", "TRUNCATE outbox_delivery_checkpoints", "TRUNCATE sys_events CASCADE"] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
    } else {
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await?;
        for sql in [
            "INSERT OR REPLACE INTO outbox_dispatch_attempts SELECT * FROM outbox_dispatch_attempts",
            "INSERT OR REPLACE INTO outbox_delivery_checkpoints SELECT * FROM outbox_delivery_checkpoints",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
    }
    assert_eq!(count_state(db, "confirmed").await?, 1);
    assert_eq!(count_state(db, "uncertain").await?, 1);
    let retention = rustok_outbox::OutboxRetention::new(db.clone()).with_config(
        rustok_outbox::OutboxRetentionConfig {
            retention: chrono::Duration::milliseconds(1), ..Default::default()
        },
    );
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(retention.prune_once().await?.pruned, 1);
    assert_eq!(count_state(db, "confirmed").await?, 0);
    assert_eq!(count_state(db, "uncertain").await?, 0);
    Ok(())
}

#[tokio::test]
async fn concurrent_consumer_ack_retries_without_waiting_for_the_source_row() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }

    let schema = db.query_one_raw(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT current_schema() AS schema_name",
    )).await?.expect("schema").try_get::<String>("", "schema_name")?;
    assert!(schema.starts_with("rustok_outbox_test_"));
    assert!(schema.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
    let database_url = std::env::var("RUSTOK_OUTBOX_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))?;
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let second = Database::connect(options).await?;
    second.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#)).await?;

    let envelope = seed_event(&db).await?;
    let consumer = rustok_outbox::OutboxConsumer::new(db.clone());
    let types = [envelope.event_type.clone()];
    consumer.register_required_event_types("consumer.first", &types).await?;
    consumer.register_required_event_types("consumer.second", &types).await?;
    let first_txn = db.begin().await?;
    assert!(rustok_outbox::OutboxConsumer::reserve_effect_in_transaction(
        &first_txn, "consumer.first", envelope.id, &envelope.event_type,
    ).await?);

    let second_txn = second.begin().await?;
    let blocked = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        rustok_outbox::OutboxConsumer::acknowledge_in_transaction(
            &second_txn, "consumer.second", envelope.id, &envelope.event_type,
        ),
    ).await.expect("late ACK must return without waiting on the source row");
    assert!(blocked.is_err());
    second_txn.rollback().await?;
    first_txn.commit().await?;

    let retry_txn = second.begin().await?;
    rustok_outbox::OutboxConsumer::acknowledge_in_transaction(
        &retry_txn, "consumer.second", envelope.id, &envelope.event_type,
    ).await?;
    retry_txn.commit().await?;
    let duplicate = second.begin().await?;
    assert!(!rustok_outbox::OutboxConsumer::reserve_effect_in_transaction(
        &duplicate, "consumer.first", envelope.id, &envelope.event_type,
    ).await?);
    duplicate.commit().await?;
    let receipts = db.query_one_raw(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT COUNT(*) AS count FROM sys_event_consumer_receipts",
    )).await?.expect("receipt count").try_get::<i64>("", "count")?;
    assert_eq!(receipts, 2);
    Ok(())
}

#[tokio::test]
async fn relay_delivers_successfully() -> TestResult<()> {
    let _guard = test_lock().lock().await;

    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    let envelope = seed_event(&db).await?;
    let transport = Arc::new(MockTransport::default());

    let relay = OutboxRelay::new(db.clone(), transport.clone()).with_config(RelayConfig {
        batch_size: 10,
        max_attempts: 3,
        ..Default::default()
    });

    let processed = relay.process_pending_once(None).await?;
    assert_eq!(processed, 1);

    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Dispatched);
    assert_eq!(record.retry_count, 0);

    let delivered = transport.delivered().await;
    assert_eq!(delivered, vec![envelope.id]);
    Ok(())
}

#[tokio::test]
async fn relay_retries_then_succeeds() -> TestResult<()> {
    let _guard = test_lock().lock().await;

    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    let envelope = seed_event(&db).await?;
    let transport = Arc::new(MockTransport::default());
    transport.fail_n_times_for(envelope.id, 1).await;

    let relay = OutboxRelay::new(db.clone(), transport.clone()).with_config(RelayConfig {
        batch_size: 10,
        max_attempts: 3,
        backoff_base: std::time::Duration::from_millis(1),
        backoff_max: std::time::Duration::from_millis(2),
        ..Default::default()
    });

    let processed_first = relay.process_pending_once(None).await?;
    assert_eq!(processed_first, 1);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let processed_second = relay.process_pending_once(None).await?;
    assert_eq!(processed_second, 1);

    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Dispatched);
    assert_eq!(record.retry_count, 1);

    let metrics = relay.metrics();
    assert_eq!(metrics.retry_total, 1);
    assert_eq!(metrics.success_total, 1);
    Ok(())
}

#[tokio::test]
async fn relay_moves_to_dlq_on_max_retry() -> TestResult<()> {
    let _guard = test_lock().lock().await;

    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    let envelope = seed_event(&db).await?;
    let transport = Arc::new(MockTransport::default());
    transport.fail_n_times_for(envelope.id, 10).await;

    let relay = OutboxRelay::new(db.clone(), transport).with_config(RelayConfig {
        batch_size: 10,
        max_attempts: 2,
        backoff_base: std::time::Duration::from_millis(1),
        backoff_max: std::time::Duration::from_millis(1),
        ..Default::default()
    });

    let _ = relay.process_pending_once(None).await?;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let _ = relay.process_pending_once(None).await?;

    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Failed);
    assert_eq!(record.retry_count, 2);
    assert!(record.next_attempt_at.is_none());
    assert!(record.last_error.is_some());

    let metrics = relay.metrics();
    assert_eq!(metrics.dlq_total, 1);
    assert_eq!(metrics.failure_total, 2);
    Ok(())
}

async fn assert_execution_policy_integrity(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared("UPDATE outbox_execution_policy SET max_in_flight = 1 WHERE id = 1").await?;
    let first = seed_event(db).await?;
    let second = seed_event(db).await?;
    async fn claim(db: &DatabaseConnection, id: Uuid) -> TestResult<()> {
        let mut row: entity::ActiveModel = entity::Entity::find_by_id(id)
            .one(db).await?.expect("event").into();
        row.claimed_by = Set(Some(format!("test-worker:{}", Uuid::new_v4())));
        row.claimed_at = Set(Some(rustok_outbox_storage::database_now(db).await?));
        row.update(db).await?;
        Ok(())
    }
    assert!(claim(db, first.id).await.is_err(), "unbound policy");
    RelayConfig::default().verify_execution_admission(db).await?;
    db.execute_unprepared("UPDATE outbox_execution_policy SET claim_ttl_ms = 5 WHERE id = 1").await?;
    let short_lease = RelayConfig {
        claim_ttl: std::time::Duration::from_millis(5),
        publish_timeout: std::time::Duration::from_millis(1),
        ..Default::default()
    };
    short_lease.verify_execution_admission(db).await?;
    for sql in [
        "DELETE FROM outbox_execution_policy",
        "INSERT INTO outbox_execution_policy VALUES (1, 2, 60000) ON CONFLICT (id) DO NOTHING",
        "UPDATE outbox_execution_policy SET claim_ttl_ms = NULL",
        "UPDATE outbox_execution_policy SET id = 2",
    ] {
        assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    if db.get_database_backend() == DatabaseBackend::Sqlite {
        for sql in [
            "INSERT OR REPLACE INTO outbox_execution_policy VALUES (1, 2, 60000)",
            "UPDATE outbox_execution_policy SET max_in_flight = 1.5",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
    } else {
        assert!(db.execute_unprepared("TRUNCATE outbox_execution_policy").await.is_err());
    }

    claim(db, first.id).await?;
    let mut backdated: entity::ActiveModel = entity::Entity::find_by_id(first.id)
        .one(db).await?.expect("claimed event").into();
    backdated.claimed_at = Set(Some(
        backdated.claimed_at.clone().expect("claim timestamp") - chrono::Duration::seconds(1)
    ));
    assert!(backdated.update(db).await.is_err(), "a live claim cannot be backdated");
    assert!(claim(db, second.id).await.is_err(), "live capacity");
    assert!(db.execute_unprepared("UPDATE outbox_execution_policy SET max_in_flight = 2").await.is_err());
    assert!(db.execute_unprepared("UPDATE outbox_execution_policy SET claim_ttl_ms = 120000").await.is_err());

    // Let the first lease expire naturally; never forge a backdated claim.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    claim(db, second.id).await?;
    assert!(claim(db, first.id).await.is_err(), "second live claim occupies capacity");
    assert!(db.execute_unprepared("UPDATE outbox_execution_policy SET max_in_flight = 2").await.is_err());
    db.execute_unprepared("UPDATE sys_events SET claimed_by = NULL, claimed_at = NULL").await?;
    db.execute_unprepared("UPDATE outbox_execution_policy SET max_in_flight = 2, claim_ttl_ms = 120000").await?;
    assert!(RelayConfig::default().verify_execution_admission(db).await.is_err());
    RelayConfig { claim_ttl: std::time::Duration::from_secs(120), ..Default::default() }.verify_execution_admission(db).await?;
    claim(db, first.id).await?;
    claim(db, second.id).await?;
    Ok(())
}

#[tokio::test]
async fn sqlite_execution_policy_integrity_rejects_reset_and_overbooking() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    assert_execution_policy_integrity(&db).await
}

#[tokio::test]
async fn postgres_execution_policy_integrity_rejects_reset_and_overbooking() -> TestResult<()> {
    let _guard = test_lock().lock().await;
    let Some(db) = setup_db().await? else { return Ok(()) };
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }
    assert_execution_policy_integrity(&db).await
}

#[tokio::test]
async fn relay_reclaims_stale_claims() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    let config = RelayConfig {
        claim_ttl: std::time::Duration::from_millis(5),
        publish_timeout: std::time::Duration::from_millis(1),
        ..Default::default()
    };
    config.verify_execution_admission(&db).await?;
    let envelope = seed_event(&db).await?;
    let mut claimed: entity::ActiveModel = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record")
        .into();
    claimed.claimed_by = Set(Some("dead-worker".to_string()));
    claimed.claimed_at = Set(Some(rustok_outbox_storage::database_now(&db).await?));
    claimed.update(&db).await?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let relay = OutboxRelay::new(db.clone(), Arc::new(MockTransport::default())).with_config(config);

    assert_eq!(relay.process_pending_once(None).await?, 1);
    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Dispatched);
    assert!(record.claimed_by.is_none());
    assert!(record.claimed_at.is_none());
    Ok(())
}

#[tokio::test]
async fn relay_bounds_parallel_dispatch() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    for _ in 0..4 {
        seed_event(&db).await?;
    }
    let transport = Arc::new(TrackingTransport::default());
    let relay = OutboxRelay::new(db, transport.clone()).with_config(RelayConfig {
        batch_size: 4,
        max_concurrency: 2,
        ..Default::default()
    });

    assert_eq!(relay.process_pending_once(None).await?, 4);
    assert_eq!(transport.max_active(), 2);
    Ok(())
}

struct GatedTransport {
    entered: Semaphore,
    release: Semaphore,
}

#[async_trait]
impl EventTransport for GatedTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
        self.entered.add_permits(1);
        self.release.acquire().await.expect("gate open").forget();
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel { ReliabilityLevel::Outbox }
    fn as_any(&self) -> &dyn Any { self }
}

fn relay_context() -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        "tenant-1", rustok_api::PortActor::service("outbox-test"), "en", "outbox-test",
    ).with_idempotency_key("relay-pass").with_deadline(std::time::Duration::from_secs(10))
}

#[tokio::test]
async fn relay_claims_only_available_slots_and_reports_each_clone_pass() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    for _ in 0..6 {
        seed_event(&db).await?;
    }
    let target = Arc::new(GatedTransport {
        entered: Semaphore::new(0),
        release: Semaphore::new(0),
    });
    let relay = OutboxRelay::new(db.clone(), target.clone()).with_config(RelayConfig {
        batch_size: 4, max_concurrency: 2, ..Default::default()
    });
    let clone = relay.clone();
    let first = tokio::spawn(async move {
        OutboxRelayPort::process_pending_once(&relay, relay_context(),
            OutboxRelayRunOnceRequest { max_batch_hint: Some(4) }).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), target.entered.acquire_many(2))
        .await?.expect("gate open").forget();
    let second = tokio::spawn(async move {
        OutboxRelayPort::process_pending_once(&clone, relay_context(),
            OutboxRelayRunOnceRequest { max_batch_hint: Some(4) }).await
    });
    let claimed = entity::Entity::find()
        .filter(entity::Column::ClaimedBy.is_not_null()).all(&db).await?;
    assert_eq!(claimed.len(), 2);
    target.release.add_permits(6);
    let first = first.await??;
    let second = second.await??;
    assert_eq!((first.claimed_count, first.success_total, first.processed_total), (4, 4, 4));
    assert_eq!((second.claimed_count, second.success_total, second.processed_total), (2, 2, 2));
    assert_eq!(first.failure_total + second.failure_total, 0);
    Ok(())
}

#[tokio::test]
async fn independent_relays_share_durable_capacity_and_ttl() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    db.execute_unprepared("UPDATE outbox_execution_policy SET max_in_flight = 2 WHERE id = 1").await?;
    for _ in 0..4 { seed_event(&db).await?; }
    let target = Arc::new(GatedTransport { entered: Semaphore::new(0), release: Semaphore::new(0) });
    let config = RelayConfig { batch_size: 4, max_concurrency: 2, ..Default::default() };
    let first = OutboxRelay::new(db.clone(), target.clone()).with_config(config.clone());
    let other = OutboxRelay::new(db.clone(), Arc::new(MockTransport::default())).with_config(config.clone());
    let mismatched = OutboxRelay::new(db.clone(), Arc::new(MockTransport::default())).with_config(RelayConfig {
        claim_ttl: std::time::Duration::from_secs(120), ..config
    });
    let pass = tokio::spawn(async move { first.process_pending_once(None).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), target.entered.acquire_many(2))
        .await?.expect("gate open").forget();
    assert_eq!(other.process_pending_once(None).await?, 0);
    assert!(mismatched.process_pending_once(None).await.is_err());
    target.release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(5), target.entered.acquire())
        .await?.expect("sliding-window refill").forget();
    let claimed = entity::Entity::find()
        .filter(entity::Column::ClaimedBy.is_not_null()).all(&db).await?;
    assert_eq!(claimed.len(), 2);
    target.release.add_permits(3);
    assert_eq!(pass.await??, 4);
    assert_eq!(other.process_pending_once(None).await?, 0);
    Ok(())
}

#[tokio::test]
async fn relay_processes_baseline_batch_with_bounded_latency() -> TestResult<()> {
    let db = setup_sqlite_db().await?;
    for _ in 0..32 {
        seed_event(&db).await?;
    }
    let transport = Arc::new(TrackingTransport::default());
    let relay = OutboxRelay::new(db.clone(), transport.clone()).with_config(RelayConfig {
        batch_size: 32,
        max_concurrency: 8,
        ..Default::default()
    });

    let started = std::time::Instant::now();
    assert_eq!(relay.process_pending_once(None).await?, 32);
    let elapsed = started.elapsed();

    assert_eq!(transport.max_active(), 8);
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "relay baseline batch took too long: {elapsed:?}"
    );

    let remaining_pending = entity::Entity::find()
        .filter(entity::Column::Status.eq(SysEventStatus::Pending))
        .all(&db)
        .await?;
    assert!(
        remaining_pending.is_empty(),
        "baseline batch left pending events"
    );
    Ok(())
}

async fn setup_db() -> TestResult<Option<DatabaseConnection>> {
    let database_url = match std::env::var("RUSTOK_OUTBOX_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
    {
        Ok(url) => url,
        Err(_) => return Ok(None),
    };

    let mut options = ConnectOptions::new(database_url.clone());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(options).await?;

    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        let schema_name = format!("rustok_outbox_test_{}", Uuid::new_v4().simple());
        db.execute_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(r#"CREATE SCHEMA "{schema_name}""#),
        ))
        .await?;
        db.execute_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(r#"SET search_path TO "{schema_name}""#),
        ))
        .await?;
    }

    let schema_manager = SchemaManager::new(&db);

    SysEventsMigration.up(&schema_manager).await?;

    Ok(Some(db))
}

async fn setup_sqlite_db() -> TestResult<DatabaseConnection> {
    let db = Database::connect("sqlite::memory:").await?;
    let schema_manager = SchemaManager::new(&db);
    SysEventsMigration.up(&schema_manager).await?;
    Ok(db)
}

async fn seed_event(db: &DatabaseConnection) -> TestResult<EventEnvelope> {
    let envelope = EventEnvelope::new(
        Uuid::new_v4(),
        None,
        DomainEvent::UserAccountRegistered {
            user_id: Uuid::new_v4(),
        },
    );

    let payload = serde_json::to_value(&envelope)?;
    let model = entity::ActiveModel {
        id: Set(envelope.id),
        tenant_id: Set(Some(envelope.tenant_id)),
        event_type: Set(envelope.event_type.clone()),
        schema_version: Set(envelope.schema_version as i16),
        payload: Set(payload),
        status: Set(SysEventStatus::Pending),
        retry_count: Set(0),
        next_attempt_at: Set(None),
        last_error: Set(None),
        claimed_by: Set(None),
        claimed_at: Set(None),
        created_at: Set(rustok_outbox_storage::database_now(db).await?),
        dispatched_at: Set(None),
    };
    entity::Entity::insert(model)
        .exec_without_returning(db)
        .await?;

    Ok(envelope)
}

fn test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}
