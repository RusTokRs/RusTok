use std::sync::Arc;

use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope, RbacRoleMutationEvent};
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, Set, TransactionTrait,
};
use sea_orm_migration::MigrationTrait;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:outbox_transport_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(opts)
        .await
        .expect("Failed to connect test sqlite database");

    let schema_manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("Failed to run outbox migration");

    db
}

#[tokio::test]
async fn publish_inserts_event_on_sqlite_without_unpack_insert_id_error() {
    let db = setup_test_db().await;
    let transport = OutboxTransport::new(db.clone());

    let envelope = EventEnvelope::new(
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
        DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "post".to_string(),
            author_id: None,
        },
    );

    transport
        .publish(envelope)
        .await
        .expect("publish should succeed on sqlite");

    let count = SysEvents::find().count(&db).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn repeated_publish_of_the_same_envelope_is_idempotent_on_sqlite() {
    let db = setup_test_db().await;
    let transport = OutboxTransport::new(db.clone());
    let envelope = EventEnvelope::new(
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
        DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "post".to_string(),
            author_id: None,
        },
    );

    transport
        .publish(envelope.clone())
        .await
        .expect("first publish should insert the canonical row");
    let mut replay = envelope;
    replay.timestamp += chrono::Duration::seconds(1);
    replay.trace_id = Some("retry-trace".into());
    replay.retry_count += 1;
    transport
        .publish(replay)
        .await
        .expect("retry with the same canonical facts should be accepted");

    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn repeated_event_id_with_different_facts_is_rejected_on_sqlite() {
    let db = setup_test_db().await;
    let transport = OutboxTransport::new(db.clone());
    let mut original = EventEnvelope::new(
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
        DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "post".to_string(),
            author_id: None,
        },
    );
    transport
        .publish(original.clone())
        .await
        .expect("first publish should insert the canonical row");

    original.event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "page".to_string(),
        author_id: None,
    };
    original.event_type = original.event.event_type().to_string();

    let error = transport
        .publish(original)
        .await
        .expect_err("reusing an envelope ID for different facts must fail");
    assert!(error.to_string().contains("different canonical facts"));
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 1);
}

struct ForwardingTransport(OutboxTransport);

#[async_trait::async_trait]
impl EventTransport for ForwardingTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
        Err(Error::Validation(
            "test requires transactional publication".into(),
        ))
    }

    fn supports_transactional_writes(&self) -> bool {
        self.0.supports_transactional_writes()
    }

    async fn write_root_in_transaction(
        &self,
        txn: &DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> Result<()> {
        self.0.write_root_in_transaction(txn, envelope).await
    }

    async fn write_contract_in_transaction(
        &self,
        txn: &DatabaseTransaction,
        envelope: ContractEventEnvelope,
    ) -> Result<()> {
        self.0.write_contract_in_transaction(txn, envelope).await
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        self.0.reliability_level()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn wrapped_transport_writes_root_and_caused_contract_on_the_owner_transaction() {
    let db = setup_test_db().await;
    let bus = TransactionalEventBus::new(Arc::new(ForwardingTransport(OutboxTransport::new(
        db.clone(),
    ))));
    let txn = db.begin().await.expect("start owner transaction");
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let root_id = bus
        .publish_in_tx_with_envelope_id(
            &txn,
            tenant_id,
            None,
            DomainEvent::UserLoggedIn { user_id },
        )
        .await
        .expect("append root fact via transport capability");
    let contract_id = bus
        .publish_contract_in_tx_with_causation_and_envelope_id(
            &txn,
            tenant_id,
            None,
            root_id,
            RbacRoleMutationEvent::user_role_assignment_repaired(user_id, "admin", 1),
        )
        .await
        .expect("append contract fact via transport capability");

    assert_eq!(SysEvents::find().count(&txn).await.unwrap(), 2);
    let row = SysEvents::find_by_id(contract_id)
        .one(&txn)
        .await
        .expect("read contract on the caller transaction")
        .expect("contract must be visible before rollback");
    let envelope: ContractEventEnvelope = serde_json::from_value(row.payload).unwrap();
    assert_eq!(envelope.id(), contract_id);
    assert_eq!(envelope.causation_id(), Some(root_id));
    assert_eq!(envelope.tenant_id(), tenant_id);
    txn.rollback().await.expect("roll back owner transaction");
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 0);
}

#[tokio::test]
async fn write_to_outbox_in_transaction_works_on_sqlite() {
    let db = setup_test_db().await;
    let transport = OutboxTransport::new(db.clone());

    let txn = db.begin().await.unwrap();
    let envelope = EventEnvelope::new(
        Uuid::new_v4(),
        Some(Uuid::new_v4()),
        DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "page".to_string(),
            author_id: None,
        },
    );

    transport
        .write_to_outbox(&txn, envelope)
        .await
        .expect("transactional outbox write should succeed on sqlite");

    txn.commit().await.unwrap();

    let count = SysEvents::find().count(&db).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn unfenced_transport_ack_cannot_steal_a_relay_claim() {
    let db = setup_test_db().await;
    let transport = OutboxTransport::new(db.clone());
    let envelope = EventEnvelope::new(
        Uuid::new_v4(), None,
        DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
    );
    let id = envelope.id;
    transport.publish(envelope).await.expect("event");
    let mut claimed: rustok_outbox::entity::ActiveModel =
        SysEvents::find_by_id(id).one(&db).await.unwrap().unwrap().into();
    claimed.claimed_by = Set(Some("other-relay".to_string()));
    claimed.claimed_at = Set(Some(chrono::Utc::now()));
    claimed.update(&db).await.expect("claim");
    assert!(transport.acknowledge(id).await.is_err());
    let stored = SysEvents::find_by_id(id).one(&db).await.unwrap().unwrap();
    assert_eq!(stored.status, rustok_outbox::entity::SysEventStatus::Pending);
    assert_eq!(stored.claimed_by.as_deref(), Some("other-relay"));
    let mut released: rustok_outbox::entity::ActiveModel = stored.into();
    released.claimed_by = Set(None);
    released.claimed_at = Set(None);
    released.update(&db).await.expect("release");
    transport.acknowledge(id).await.expect("unclaimed ack");
    transport.acknowledge(id).await.expect("idempotent ack");
}
