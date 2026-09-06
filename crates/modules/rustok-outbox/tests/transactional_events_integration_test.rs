//! Transactional persistence tests owned by the outbox runtime.

mod support;

use rustok_core::events::{DomainEvent, MemoryTransport};
use rustok_outbox::entity::SysEventStatus;
use rustok_outbox::{OutboxTransport, SysEvents, TransactionalEventBus};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use std::sync::Arc;
use uuid::Uuid;

use support::setup_test_db;

fn count_for_tenant(events: &[rustok_outbox::SysEvent], tenant_id: Uuid) -> usize {
    events
        .iter()
        .filter(|event| event.payload["tenant_id"] == tenant_id.to_string())
        .count()
}

#[tokio::test]
async fn test_transactional_event_publishing_rollback() {
    let db = setup_test_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport.clone());

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let txn = db.begin().await.unwrap();

    let event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "post".to_string(),
        author_id: Some(user_id),
    };

    event_bus
        .publish_in_tx(&txn, tenant_id, Some(user_id), event)
        .await
        .expect("Failed to publish event in transaction");

    txn.rollback().await.unwrap();

    let events = SysEvents::find().all(&db).await.unwrap();
    let count = count_for_tenant(&events, tenant_id);
    assert_eq!(count, 0, "Events should not be persisted after rollback");
}

#[tokio::test]
async fn test_transactional_event_publishing_commit() {
    let db = setup_test_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport.clone());

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let txn = db.begin().await.unwrap();

    let event = DomainEvent::NodeCreated {
        node_id,
        kind: "post".to_string(),
        author_id: Some(user_id),
    };

    event_bus
        .publish_in_tx(&txn, tenant_id, Some(user_id), event)
        .await
        .expect("Failed to publish event in transaction");

    txn.commit().await.unwrap();

    let events = SysEvents::find().all(&db).await.unwrap();
    let count = count_for_tenant(&events, tenant_id);
    assert_eq!(count, 1, "Event should be persisted after commit");

    let persisted_event = events
        .into_iter()
        .find(|event| event.payload["tenant_id"] == tenant_id.to_string())
        .expect("Expected tenant event to be persisted");
    assert_eq!(persisted_event.event_type, "node.created");
    assert_eq!(persisted_event.schema_version, 1);
    assert_eq!(persisted_event.status, SysEventStatus::Pending);
}

#[tokio::test]
async fn test_mixed_transactional_and_non_transactional_events() {
    let db = setup_test_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport.clone());

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    event_bus
        .publish(
            tenant_id,
            Some(user_id),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "page".to_string(),
                author_id: Some(user_id),
            },
        )
        .await
        .expect("Failed to publish non-transactional event");

    let txn = db.begin().await.unwrap();
    event_bus
        .publish_in_tx(
            &txn,
            tenant_id,
            Some(user_id),
            DomainEvent::NodeCreated {
                node_id,
                kind: "post".to_string(),
                author_id: Some(user_id),
            },
        )
        .await
        .expect("Failed to publish transactional event");
    txn.commit().await.unwrap();

    let events = SysEvents::find().all(&db).await.unwrap();
    let count = count_for_tenant(&events, tenant_id);
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_multiple_events_in_single_transaction() {
    let db = setup_test_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport.clone());

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let txn = db.begin().await.unwrap();

    let events = vec![
        DomainEvent::NodeCreated {
            node_id,
            kind: "post".to_string(),
            author_id: Some(user_id),
        },
        DomainEvent::NodeUpdated {
            node_id,
            kind: "post".to_string(),
        },
        DomainEvent::NodePublished {
            node_id,
            kind: "post".to_string(),
        },
    ];

    for event in events {
        event_bus
            .publish_in_tx(&txn, tenant_id, Some(user_id), event)
            .await
            .expect("Failed to publish event in transaction");
    }

    txn.commit().await.unwrap();

    let events = SysEvents::find().all(&db).await.unwrap();
    let count = count_for_tenant(&events, tenant_id);
    assert_eq!(count, 3, "All events in transaction should be persisted");
}

#[tokio::test]
async fn test_event_validation_failure_does_not_persist() {
    let db = setup_test_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport.clone());

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let txn = db.begin().await.unwrap();

    let invalid_event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "".to_string(),
        author_id: Some(user_id),
    };

    let result = event_bus
        .publish_in_tx(&txn, tenant_id, Some(user_id), invalid_event)
        .await;

    assert!(result.is_err());
    txn.rollback().await.unwrap();

    let events = SysEvents::find().all(&db).await.unwrap();
    let count = count_for_tenant(&events, tenant_id);
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_pending_status_after_publish() {
    let db = setup_test_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport);

    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    event_bus
        .publish(
            tenant_id,
            Some(user_id),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: Some(user_id),
            },
        )
        .await
        .unwrap();

    let events = SysEvents::find()
        .filter(rustok_outbox::entity::Column::Status.eq(SysEventStatus::Pending))
        .all(&db)
        .await
        .unwrap();

    let pending_count = count_for_tenant(&events, tenant_id);
    assert_eq!(pending_count, 1);
}

#[tokio::test]
async fn test_transactional_publish_rejects_non_outbox_transport() {
    let db = setup_test_db().await;
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    let txn = db.begin().await.unwrap();

    let result = event_bus
        .publish_in_tx(
            &txn,
            Uuid::new_v4(),
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: None,
            },
        )
        .await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("requires OutboxTransport")
    );
    txn.rollback().await.unwrap();
}
