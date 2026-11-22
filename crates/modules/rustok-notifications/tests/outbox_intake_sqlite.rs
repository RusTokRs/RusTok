use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_notifications::api::{
    NotificationSourceEventRef, NotificationSourceSlug, NotificationTypeKey,
};
use rustok_notifications::entities::source_inbox;
use rustok_notifications::{
    MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, NotificationError,
    NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    NotificationOutboxIntakeOutcome, NotificationOutboxIntakeReceiptPort,
    NotificationOutboxIntakeWorker, NotificationResult,
    NotificationsModule,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use serde::Deserialize;
use uuid::Uuid;

const EVENT_TYPE: &str = "forum.topic.created";
const SOURCE: &str = "forum";

#[derive(Debug, Deserialize)]
struct FakeEnvelopePayload {
    tenant_id: Option<Uuid>,
    source_event_id: Option<Uuid>,
    source_revision: Option<u64>,
    permanent_invalid: Option<bool>,
    retryable: Option<bool>,
}

#[derive(Debug, Default)]
struct FakeDecoder;

impl NotificationOutboxEnvelopeDecoder for FakeDecoder {
    fn decode(
        &self,
        envelope: &NotificationOutboxEnvelopeRecord,
    ) -> NotificationResult<NotificationSourceEventRef> {
        let payload: FakeEnvelopePayload = serde_json::from_value(envelope.payload.clone())?;
        if payload.retryable.unwrap_or(false) {
            return Err(NotificationError::SourceUnavailable);
        }
        if payload.permanent_invalid.unwrap_or(false) {
            return Err(NotificationError::InvalidEvent);
        }
        NotificationSourceEventRef::new(
            payload.tenant_id.ok_or(NotificationError::InvalidEvent)?,
            payload
                .source_event_id
                .ok_or(NotificationError::InvalidEvent)?,
            NotificationSourceSlug::new(SOURCE).map_err(|_| NotificationError::InvalidEvent)?,
            NotificationTypeKey::new(envelope.event_type.clone())
                .map_err(|_| NotificationError::InvalidEvent)?,
            payload
                .source_revision
                .ok_or(NotificationError::InvalidEvent)?,
        )
        .map_err(|_| NotificationError::InvalidEvent)
    }
}

#[derive(Debug)]
struct TestOutboxReceiptPort {
    fail: bool,
}

#[async_trait::async_trait]
impl NotificationOutboxIntakeReceiptPort for TestOutboxReceiptPort {
    async fn acknowledge_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        consumer_key: &str,
        outbox_event_id: Uuid,
        event_type: &str,
    ) -> NotificationResult<()> {
        if self.fail {
            return Err(NotificationError::Database(sea_orm::DbErr::Custom(
                "simulated outbox acknowledgement failure".to_string(),
            )));
        }
        transaction
            .execute_unprepared(&format!(
                "INSERT OR IGNORE INTO test_outbox_consumer_receipts \
                 (outbox_event_id, consumer_key, event_type) \
                 VALUES ('{outbox_event_id}', '{consumer_key}', '{event_type}')"
            ))
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn terminal_intake_receipt_and_outbox_ack_commit_atomically() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;
    let accepted_id = Uuid::new_v4();
    let source_event_id = Uuid::new_v4();
    insert_outbox_event(
        &db,
        accepted_id,
        1,
        serde_json::json!({
            "tenant_id": tenant_id,
            "source_event_id": source_event_id,
            "source_revision": 1,
        }),
        1,
    )
    .await;
    let worker = NotificationOutboxIntakeWorker::new(db.clone(), Arc::new(FakeDecoder), 8)
        .expect("worker")
        .with_receipt_port(
            "test.notifications",
            Arc::new(TestOutboxReceiptPort { fail: false }),
        )
        .expect("receipt port");
    let accepted = worker
        .process_outbox_event(accepted_id)
        .await
        .expect("accepted receipt and outbox acknowledgement commit together");
    assert!(matches!(accepted, NotificationOutboxIntakeOutcome::Accepted(_)));
    let ack_count: i64 = db
        .query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM test_outbox_consumer_receipts".to_string(),
        ))
        .await
        .expect("ack query")
        .expect("ack row")
        .try_get("", "count")
        .expect("ack count");
    assert_eq!(ack_count, 1);

    let rejected_id = Uuid::new_v4();
    insert_outbox_event(
        &db,
        rejected_id,
        1,
        serde_json::json!({"permanent_invalid": true}),
        2,
    )
    .await;
    let failing_worker = NotificationOutboxIntakeWorker::new(db.clone(), Arc::new(FakeDecoder), 8)
        .expect("worker")
        .with_receipt_port(
            "test.notifications",
            Arc::new(TestOutboxReceiptPort { fail: true }),
        )
        .expect("receipt port");
    let error = failing_worker
        .process_outbox_event(rejected_id)
        .await
        .expect_err("ack failure must roll back the permanent rejection");
    assert!(error.is_retryable());
    assert_eq!(
        db.query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*) AS count FROM notification_outbox_intake_rejections".to_string(),
        ))
        .await
        .expect("rejection query")
        .expect("rejection row")
        .try_get::<i64>("", "count")
        .expect("rejection count"),
        0,
        "the notification rejection must not commit without its Outbox acknowledgement"
    );
}

#[tokio::test]
async fn accepted_and_permanent_invalid_envelopes_leave_no_head_of_line_blocker() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    insert_tenant(&db, tenant_id).await;

    let permanent_id = Uuid::new_v4();
    insert_outbox_event(
        &db,
        permanent_id,
        1,
        serde_json::json!({"permanent_invalid": true}),
        0,
    )
    .await;

    let mut valid = Vec::new();
    for sequence in 1..=33 {
        let outbox_event_id = Uuid::new_v4();
        let source_event_id = Uuid::new_v4();
        insert_outbox_event(
            &db,
            outbox_event_id,
            1,
            serde_json::json!({
                "tenant_id": tenant_id,
                "source_event_id": source_event_id,
                "source_revision": sequence,
            }),
            sequence,
        )
        .await;
        valid.push((outbox_event_id, source_event_id, sequence));
    }

    let worker = NotificationOutboxIntakeWorker::new(db.clone(), Arc::new(FakeDecoder), 32)
        .expect("bounded intake worker should compose");
    let first = worker
        .process_next_batch()
        .await
        .expect("first intake page should process");
    assert_eq!(first.selected, 32);
    assert_eq!(first.accepted, 31);
    assert_eq!(first.rejected, 1);
    assert!(first.failures.is_empty());

    let second = worker
        .process_next_batch()
        .await
        .expect("second intake page should process");
    assert_eq!(second.selected, 2);
    assert_eq!(second.accepted, 2);
    assert_eq!(second.rejected, 0);
    assert!(second.failures.is_empty());
    assert!(
        worker
            .pending_outbox_event_ids()
            .await
            .expect("terminal intake outcomes should be excluded")
            .is_empty()
    );

    assert_eq!(
        source_inbox::Entity::find()
            .filter(source_inbox::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("source inbox count should succeed"),
        33
    );

    let (accepted_outbox_id, accepted_source_id, accepted_revision) = valid[0];
    let accepted_row = source_inbox::Entity::find()
        .filter(source_inbox::Column::TenantId.eq(tenant_id))
        .filter(source_inbox::Column::SourceEventId.eq(accepted_source_id))
        .one(&db)
        .await
        .expect("accepted source inbox query should succeed")
        .expect("accepted source inbox row should exist");
    assert_eq!(accepted_row.event_type, EVENT_TYPE);
    assert_eq!(accepted_row.source_revision, accepted_revision as i64);
    assert_ne!(accepted_outbox_id, accepted_row.source_event_id);

    let replay = worker
        .process_outbox_event(accepted_outbox_id)
        .await
        .expect("accepted outbox event should replay its receipt");
    let NotificationOutboxIntakeOutcome::Accepted(replay) = replay else {
        panic!("accepted event replayed as rejection");
    };
    assert!(replay.replayed);
    assert_eq!(replay.source_inbox_id, accepted_row.id);

    db.execute_unprepared(&format!(
        "UPDATE sys_events SET payload = '{{\"permanent_invalid\":true}}' WHERE id = X'{}'",
        accepted_outbox_id.simple()
    ))
    .await
    .expect("accepted outbox payload should mutate for conflict evidence");
    let changed_replay = worker
        .process_outbox_event(accepted_outbox_id)
        .await
        .expect_err("changed accepted envelope must fail semantic replay");
    assert_eq!(
        changed_replay.stable_code(),
        "NOTIFICATION_SOURCE_IDENTITY_CONFLICT"
    );

    let rejection = worker
        .process_outbox_event(permanent_id)
        .await
        .expect("permanent invalid event should replay quarantine");
    let NotificationOutboxIntakeOutcome::Rejected(rejection) = rejection else {
        panic!("permanent invalid event replayed as acceptance");
    };
    assert!(rejection.replayed);
    assert_eq!(rejection.error_code, "NOTIFICATION_SOURCE_EVENT_INVALID");

    let retryable_id = Uuid::new_v4();
    insert_outbox_event(
        &db,
        retryable_id,
        1,
        serde_json::json!({"retryable": true}),
        100,
    )
    .await;
    let retryable = worker
        .process_outbox_event(retryable_id)
        .await
        .expect_err("retryable decoder failure must not become terminal");
    assert!(retryable.is_retryable());
    assert_eq!(
        worker
            .pending_outbox_event_ids()
            .await
            .expect("retryable event remains claimable"),
        vec![retryable_id]
    );

    let oversized = NotificationOutboxIntakeWorker::new(
        db,
        Arc::new(FakeDecoder),
        MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE + 1,
    )
    .expect_err("intake batches above the hard maximum must fail");
    assert_eq!(oversized.stable_code(), "NOTIFICATION_VALIDATION_ERROR");
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:notification_outbox_intake_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("outbox intake sqlite database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should enable");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL);
        CREATE TABLE users (
            id BLOB PRIMARY KEY NOT NULL,
            tenant_id BLOB NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
            UNIQUE (tenant_id, id)
        );
        CREATE TABLE sys_events (
            id BLOB PRIMARY KEY NOT NULL,
            event_type TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE test_outbox_consumer_receipts (
            outbox_event_id TEXT NOT NULL,
            consumer_key TEXT NOT NULL,
            event_type TEXT NOT NULL,
            PRIMARY KEY (outbox_event_id, consumer_key)
        );
        "#,
    )
    .await
    .expect("platform and outbox fixtures should apply");
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
    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES (X'{}')",
        tenant_id.simple()
    ))
    .await
    .expect("tenant fixture should persist");
}

async fn insert_outbox_event(
    db: &DatabaseConnection,
    outbox_event_id: Uuid,
    schema_version: i16,
    payload: serde_json::Value,
    sequence: u32,
) {
    let payload = payload.to_string().replace('\'', "''");
    let minute = (sequence / 60) % 60;
    let second = sequence % 60;
    db.execute_unprepared(&format!(
        "INSERT INTO sys_events (id, event_type, schema_version, payload, created_at) VALUES (X'{}', '{EVENT_TYPE}', {schema_version}, '{payload}', '2026-07-23T12:{minute:02}:{second:02}Z')",
        outbox_event_id.simple()
    ))
    .await
    .expect("outbox fixture should persist");
}
