use rustok_core::events::EventTransport;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration};
use sea_orm::{
    ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait, TransactionTrait,
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
