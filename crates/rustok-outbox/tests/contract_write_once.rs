use rustok_events::{
    BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION, BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY,
    BlogCommentsDelegationScheduleAuditEvent, ContractEventEnvelope,
};
use rustok_outbox::{
    ContractEventWriteOnceError, SysEvents, SysEventsMigration, TransactionalEventBus,
};
use sea_orm::{Database, EntityTrait, PaginatorTrait, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn database() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .expect("sys_events migration");
    db
}

fn event(request_id: Uuid, candidate_generation: i64) -> BlogCommentsDelegationScheduleAuditEvent {
    BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded {
        audit_schema_version: BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
        request_id,
        state_key: BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY.to_string(),
        occurred_at_unix_ms: 1,
        principal_kind: "service".to_string(),
        operation: "replace_host_schedule".to_string(),
        source: "host_provided".to_string(),
        previous_generation: 1,
        candidate_generation,
    }
}

#[tokio::test]
async fn exact_replay_returns_the_same_envelope_and_keeps_one_row() {
    let db = database().await;
    let request_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let first_tx = db.begin().await.expect("first transaction");
    let first = TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        &first_tx,
        request_id,
        tenant_id,
        Some(actor_id),
        event(request_id, 2),
    )
    .await
    .expect("first canonical write");
    first_tx.commit().await.expect("first commit");

    let replay_tx = db.begin().await.expect("replay transaction");
    let replay = TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        &replay_tx,
        request_id,
        tenant_id,
        Some(actor_id),
        event(request_id, 2),
    )
    .await
    .expect("exact replay");
    replay_tx.commit().await.expect("replay commit");

    assert_eq!(first, request_id);
    assert_eq!(replay, request_id);
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn mismatched_request_id_reuse_returns_conflict_and_preserves_the_first_row() {
    let db = database().await;
    let request_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let first_tx = db.begin().await.expect("first transaction");
    TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        &first_tx,
        request_id,
        tenant_id,
        Some(actor_id),
        event(request_id, 2),
    )
    .await
    .expect("first canonical write");
    first_tx.commit().await.expect("first commit");

    let conflict_tx = db.begin().await.expect("conflict transaction");
    let error = TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        &conflict_tx,
        request_id,
        tenant_id,
        Some(actor_id),
        event(request_id, 3),
    )
    .await
    .expect_err("mismatched payload must conflict");
    conflict_tx.rollback().await.expect("conflict rollback");

    assert_eq!(error, ContractEventWriteOnceError::Conflict);
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn exact_caused_replay_keeps_one_row_and_preserves_causation() {
    let db = database().await;
    let request_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    for label in ["first", "replay"] {
        let txn = db.begin().await.expect("caused write transaction");
        let envelope_id =
            TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation(
                &txn,
                request_id,
                tenant_id,
                Some(actor_id),
                root_event_id,
                event(request_id, 2),
            )
            .await
            .unwrap_or_else(|error| panic!("{label} caused write failed: {error:?}"));
        assert_eq!(envelope_id, request_id);
        txn.commit().await.expect("caused write commit");
    }

    let row = SysEvents::find_by_id(request_id)
        .one(&db)
        .await
        .expect("stored row read")
        .expect("stored row");
    let envelope: ContractEventEnvelope =
        serde_json::from_value(row.payload).expect("stored contract envelope");

    assert_eq!(envelope.id(), request_id);
    assert_eq!(envelope.correlation_id(), request_id);
    assert_eq!(envelope.causation_id(), Some(root_event_id));
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn caused_write_once_rejects_causation_reuse_conflict() {
    let db = database().await;
    let request_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let first_tx = db.begin().await.expect("first caused transaction");
    TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation(
        &first_tx,
        request_id,
        tenant_id,
        Some(actor_id),
        Uuid::new_v4(),
        event(request_id, 2),
    )
    .await
    .expect("first caused write");
    first_tx.commit().await.expect("first caused commit");

    let conflict_tx = db.begin().await.expect("causation conflict transaction");
    let error =
        TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation(
            &conflict_tx,
            request_id,
            tenant_id,
            Some(actor_id),
            Uuid::new_v4(),
            event(request_id, 2),
        )
        .await
        .expect_err("different causation identity must conflict");
    conflict_tx
        .rollback()
        .await
        .expect("causation conflict rollback");

    assert_eq!(error, ContractEventWriteOnceError::Conflict);
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 1);
}
