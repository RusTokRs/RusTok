//! Definitions only: canonical storage/relay contracts, not live broker or Index evidence.
use async_trait::async_trait;
use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_events::{ContractEventEnvelope, EventEnvelope, InventorySourceEvent};
use rustok_outbox::{
    ContractEventWriteOnceError, OutboxRelay, RelayConfig, SysEvents, SysEventsMigration,
    TransactionalEventBus, entity::SysEventStatus,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::{
    any::Any,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

async fn database() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .unwrap();
    db
}
fn changed(version: u64) -> InventorySourceEvent {
    InventorySourceEvent::Changed {
        baseline_id: Uuid::new_v4(),
        variant_id: Uuid::new_v4(),
        source_version: version,
    }
}
async fn persist(
    db: &DatabaseConnection,
    id: Uuid,
    tenant: Uuid,
    actor: Option<Uuid>,
    event: InventorySourceEvent,
) {
    let txn = db.begin().await.unwrap();
    assert_eq!(
        TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
            &txn, id, tenant, actor, event
        )
        .await
        .unwrap(),
        id
    );
    txn.commit().await.unwrap();
}
async fn stored(db: &DatabaseConnection, id: Uuid) -> rustok_outbox::SysEvent {
    SysEvents::find_by_id(id).one(db).await.unwrap().unwrap()
}

struct Target {
    calls: Mutex<Vec<ContractEventEnvelope>>,
    fail: AtomicBool,
}
#[async_trait]
impl EventTransport for Target {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        Err(rustok_core::Error::Validation(
            "source facts cannot use root fallback".into(),
        ))
    }
    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
        self.calls.lock().unwrap().push(envelope);
        if self.fail.load(Ordering::SeqCst) {
            return Err(rustok_core::Error::External(
                "required source handler unavailable".into(),
            ));
        }
        Ok(())
    }
    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Streaming
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
fn relay(db: &DatabaseConnection, target: Arc<dyn EventTransport>, attempts: i32) -> OutboxRelay {
    OutboxRelay::new(db.clone(), target).with_config(RelayConfig {
        max_attempts: attempts,
        max_concurrency: 1,
        backoff_base: Duration::ZERO,
        backoff_max: Duration::ZERO,
        worker_id: "inventory-source-delivery-regression".into(),
        ..RelayConfig::default()
    })
}

#[tokio::test]
async fn inventory_source_write_once_exact_replay_preserves_first_frame_and_mismatch_refuses() {
    let db = database().await;
    let id = Uuid::new_v4();
    let tenant = Uuid::new_v4();
    let actor = Some(Uuid::new_v4());
    let event = changed(2);
    persist(&db, id, tenant, actor, event.clone()).await;
    let first = stored(&db, id).await;
    persist(&db, id, tenant, actor, event.clone()).await;
    assert_eq!(stored(&db, id).await, first);
    let InventorySourceEvent::Changed {
        baseline_id,
        variant_id,
        ..
    } = event
    else {
        unreachable!()
    };
    let txn = db.begin().await.unwrap();
    let result = TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        &txn,
        id,
        tenant,
        actor,
        InventorySourceEvent::Changed {
            baseline_id,
            variant_id,
            source_version: 3,
        },
    )
    .await;
    assert_eq!(result, Err(ContractEventWriteOnceError::Conflict));
    txn.rollback().await.unwrap();
    assert_eq!(stored(&db, id).await, first);
}

#[tokio::test]
async fn inventory_source_relay_dispatches_both_registered_facts_without_root_fallback() {
    for event in [
        changed(2),
        InventorySourceEvent::BaselineActivated {
            baseline_id: Uuid::new_v4(),
        },
    ] {
        let db = database().await;
        let id = match &event {
            InventorySourceEvent::BaselineActivated { baseline_id } => *baseline_id,
            InventorySourceEvent::Changed { .. } => Uuid::new_v4(),
        };
        persist(&db, id, Uuid::new_v4(), Some(Uuid::new_v4()), event).await;
        let before = stored(&db, id).await;
        let envelope: ContractEventEnvelope = serde_json::from_value(before.payload).unwrap();
        let target = Arc::new(Target {
            calls: Mutex::new(vec![]),
            fail: AtomicBool::new(false),
        });
        assert_eq!(
            relay(&db, target.clone(), 3)
                .process_pending_once(Some(1))
                .await
                .unwrap(),
            1
        );
        assert_eq!(*target.calls.lock().unwrap(), vec![envelope]);
        assert_eq!(stored(&db, id).await.status, SysEventStatus::Dispatched);
    }
}

#[tokio::test]
async fn inventory_source_required_delivery_failure_retries_the_unchanged_canonical_frame() {
    let db = database().await;
    let id = Uuid::new_v4();
    persist(&db, id, Uuid::new_v4(), None, changed(2)).await;
    let original = stored(&db, id).await;
    let target = Arc::new(Target {
        calls: Mutex::new(vec![]),
        fail: AtomicBool::new(true),
    });
    let relay = relay(&db, target.clone(), 3);
    assert_eq!(relay.process_pending_once(Some(1)).await.unwrap(), 1);
    let pending = stored(&db, id).await;
    assert_eq!(pending.status, SysEventStatus::Pending);
    assert_eq!(pending.retry_count, 1);
    assert_eq!(pending.payload, original.payload);
    assert!(pending.claimed_by.is_none() && pending.claimed_at.is_none());
    assert!(pending.dispatched_at.is_none());
    target.fail.store(false, Ordering::SeqCst);
    assert_eq!(relay.process_pending_once(Some(1)).await.unwrap(), 1);
    let dispatched = stored(&db, id).await;
    assert_eq!(dispatched.status, SysEventStatus::Dispatched);
    assert_eq!(dispatched.id, original.id);
    assert_eq!(dispatched.payload, original.payload);
    let calls = target.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], calls[1]);
}

#[tokio::test]
async fn inventory_source_unhandled_delivery_exhaustion_is_failed_not_dispatched() {
    let db = database().await;
    let id = Uuid::new_v4();
    persist(&db, id, Uuid::new_v4(), None, changed(2)).await;
    let target = Arc::new(Target {
        calls: Mutex::new(vec![]),
        fail: AtomicBool::new(true),
    });
    assert_eq!(
        relay(&db, target, 1)
            .process_pending_once(Some(1))
            .await
            .unwrap(),
        1
    );
    let failed = stored(&db, id).await;
    assert_eq!(failed.status, SysEventStatus::Failed);
    assert_eq!(failed.retry_count, 1);
    assert!(failed.dispatched_at.is_none());
    assert!(failed.claimed_by.is_none() && failed.claimed_at.is_none());
}
