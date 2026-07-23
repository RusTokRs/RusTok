use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_notifications::api::{
    NotificationSourceEventRef, NotificationSourceSlug, NotificationTypeKey,
};
use rustok_notifications::entities::source_inbox;
use rustok_notifications::{
    MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, NotificationError,
    NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    NotificationOutboxIntakeOutcome, NotificationOutboxIntakeWorker, NotificationResult,
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
    assert!(worker
        .pending_outbox_event_ids()
        .await
        .expect("terminal intake outcomes should be excluded")
        .is_empty());

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
        CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL);
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
        );
        CREATE TABLE sys_events (
            id TEXT PRIMARY KEY NOT NULL,
            event_type TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
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
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
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
    db.execute_unprepared(&format!(
        "INSERT INTO sys_events (id, event_type, schema_version, payload, created_at) VALUES ('{outbox_event_id}', '{EVENT_TYPE}', {schema_version}, '{payload}', '2026-07-23T12:00:{sequence:02}Z')"
    ))
    .await
    .expect("outbox fixture should persist");
}
