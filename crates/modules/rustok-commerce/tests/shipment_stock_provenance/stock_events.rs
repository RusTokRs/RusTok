//! Native stock/outbox coupling. Included by the registered parent target, not a new binary.
//! These regression definitions have not been compiled or executed by the authoring agent.
use super::*;
use rustok_api::PortActor;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_inventory::shipment_stock::{
    ShipmentStockAction, ShipmentStockBatch, ShipmentStockLine, validate_shipment_stock_preparation,
};
use rustok_outbox::entity::{self as outbox, SysEventStatus};

async fn stock_events(f: &Fixture) -> Vec<EventEnvelope> {
    outbox::Entity::find()
        .filter(outbox::Column::EventType.eq("inventory.updated"))
        .all(&f.db)
        .await
        .expect("canonical outbox rows")
        .into_iter()
        .map(|row| {
            assert_eq!(row.status, SysEventStatus::Pending);
            let envelope = serde_json::from_value::<EventEnvelope>(row.payload)
                .expect("registered root envelope");
            envelope.validate_registered_schema().expect("v1 stock schema");
            assert_eq!(envelope.id, row.id);
            assert_eq!(envelope.event_type, row.event_type);
            envelope
        })
        .filter(|envelope| {
            envelope.tenant_id == f.tenant
                && matches!(&envelope.event, DomainEvent::InventoryUpdated { variant_id, .. } if *variant_id == f.variant)
        })
        .collect()
}

fn assert_stock_event(
    envelope: &EventEnvelope,
    f: &Fixture,
    location_id: Uuid,
    old: i32,
    new: i32,
) {
    assert_eq!(envelope.tenant_id, f.tenant);
    match &envelope.event {
        DomainEvent::InventoryUpdated {
            variant_id,
            product_id,
            location_id: saved_location,
            old_quantity,
            new_quantity,
        } => {
            assert_eq!(*variant_id, f.variant);
            assert!(!product_id.is_nil());
            assert_eq!(*saved_location, location_id);
            assert_eq!((*old_quantity, *new_quantity), (old, new));
        }
        other => panic!("expected Inventory-owned stock event, got {other:?}"),
    }
}

fn assert_owner_code(error: rustok_fulfillment::FulfillmentError, expected: &str) {
    match error {
        rustok_fulfillment::FulfillmentError::OwnerPort(cause) => assert_eq!(cause.code, expected),
        other => panic!("expected typed owner refusal, got {other:?}"),
    }
}

async fn remove_outbox(f: &Fixture) {
    f.db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "DROP TABLE sys_events",
    ))
    .await
    .expect("missing-outbox failure fixture");
}

fn pending_batch(f: &Fixture) -> ShipmentStockBatch {
    let item = &f.fulfillment.items[0];
    ShipmentStockBatch {
        order_id: f.fulfillment.order_id,
        fulfillment_id: f.fulfillment.id,
        lines: vec![ShipmentStockLine {
            fulfillment_item_id: item.id,
            order_line_item_id: item.order_line_item_id,
            variant_id: Some(f.variant),
            fulfillment_quantity: item.quantity,
            shipped_quantity: 0,
            delivered_quantity: 0,
            quantity: 1,
        }],
    }
}

#[tokio::test]
async fn shipment_enqueues_current_actor_event_even_when_available_quantity_is_unchanged() {
    let f = Fixture::new().await;
    let before = stock_events(&f).await;
    let actor_id = Uuid::new_v4();
    let mut context = support::shipment_context(f.tenant);
    context.actor = PortActor::user(actor_id.to_string());
    f.service
        .ship_fulfillment(context, f.fulfillment.id, f.ship_input(2))
        .await
        .expect("atomic native shipment");
    assert_eq!(f.stock().await, (3, 1));
    let events = stock_events(&f).await;
    let new: Vec<_> = events
        .iter()
        .filter(|event| !before.iter().any(|saved| saved.id == event.id))
        .collect();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].actor_id, Some(actor_id));
    assert_stock_event(new[0], &f, f.allocation().await.location_id, 2, 2);
}

#[tokio::test]
async fn cancellation_restoration_and_reconsumption_publish_original_location_snapshots() {
    let f = Fixture::new().await;
    f.ship(2).await.expect("ship");
    let shipped = stock_events(&f).await;
    f.cancel().await.expect("restore undelivered stock");
    let cancelled = stock_events(&f).await;
    let restored: Vec<_> = cancelled
        .iter()
        .filter(|event| !shipped.iter().any(|saved| saved.id == event.id))
        .collect();
    assert_eq!(restored.len(), 1);
    let location = f.allocation().await.location_id;
    assert_stock_event(restored[0], &f, location, 2, 4);
    f.reopen().await.expect("reconsume exact restored stock");
    let reopened = stock_events(&f).await;
    let reconsumed: Vec<_> = reopened
        .iter()
        .filter(|event| !cancelled.iter().any(|saved| saved.id == event.id))
        .collect();
    assert_eq!(reconsumed.len(), 1);
    assert_stock_event(reconsumed[0], &f, location, 4, 2);
    assert_eq!(f.stock().await, (3, 1));
}

#[tokio::test]
async fn preparation_never_publishes_or_requires_outbox_storage() {
    let f = Fixture::new().await;
    let original = f.allocation().await;
    remove_outbox(&f).await;
    validate_shipment_stock_preparation(
        &f.db,
        &support::shipment_context(f.tenant),
        &pending_batch(&f),
        ShipmentStockAction::Ship,
    )
    .await
    .expect("rolled-back probe needs no notification persistence");
    assert_eq!(f.allocation().await, original);
    assert!(f.proof().await.is_none());
    assert_eq!(f.stock().await, (5, 3));
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn late_fulfillment_failure_rolls_back_stock_proof_and_queued_event() {
    let f = Fixture::new().await;
    let before: Vec<_> = stock_events(&f)
        .await
        .into_iter()
        .map(|event| event.id)
        .collect();
    f.db.execute_raw(Statement::from_string(DbBackend::Sqlite,
        "CREATE TRIGGER stock_event_late_f_failure BEFORE UPDATE OF shipped_quantity ON fulfillment_items \
         WHEN NEW.shipped_quantity > 0 BEGIN SELECT RAISE(ABORT, 'stock event atomicity fixture'); END",
    )).await.expect("late owner failure fixture");
    assert!(f.ship(1).await.is_err());
    let after: Vec<_> = stock_events(&f)
        .await
        .into_iter()
        .map(|event| event.id)
        .collect();
    assert_eq!(after, before);
    assert_eq!(f.stock().await, (5, 3));
    assert!(f.proof().await.is_none());
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn missing_outbox_rolls_back_shipment_instead_of_acknowledging_eventless_stock() {
    let f = Fixture::new().await;
    let original = f.allocation().await;
    remove_outbox(&f).await;
    let error = f
        .ship(1)
        .await
        .expect_err("canonical outbox is mandatory for a fresh effect");
    assert_owner_code(error, "inventory.stock_event_persistence_unavailable");
    assert_eq!(f.allocation().await, original);
    assert_eq!(f.stock().await, (5, 3));
    assert!(f.proof().await.is_none());
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn suppressed_outbox_insert_is_not_a_successful_shipment() {
    let f = Fixture::new().await;
    let original = f.allocation().await;
    let before = stock_events(&f).await;
    f.db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TRIGGER suppress_stock_event BEFORE INSERT ON sys_events \
         WHEN NEW.event_type = 'inventory.updated' BEGIN SELECT RAISE(IGNORE); END",
    ))
    .await
    .expect("zero inserted rows fixture");
    assert!(f.ship(1).await.is_err());
    assert_eq!(stock_events(&f).await.len(), before.len());
    assert_eq!(f.allocation().await, original);
    assert_eq!(f.stock().await, (5, 3));
    assert!(f.proof().await.is_none());
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn exact_return_replay_does_not_enqueue_another_event_or_require_outbox() {
    let f = Fixture::new().await;
    f.ship(2).await.expect("native consumed history");
    let line = f.returned_line();
    let before = stock_events(&f).await;
    restock_returned_items(&f.db, support::shipment_context(f.tenant), &[line])
        .await
        .expect("new inspected stock");
    let saved = rustok_inventory::return_restock::ledger::Entity::find_by_id(line.return_item_id)
        .one(&f.db)
        .await
        .expect("receipt query")
        .expect("receipt");
    let after = stock_events(&f).await;
    let returned: Vec<_> = after
        .iter()
        .filter(|event| !before.iter().any(|old| old.id == event.id))
        .collect();
    assert_eq!(returned.len(), 1);
    assert_stock_event(returned[0], &f, f.allocation().await.location_id, 2, 3);
    let replay = restock_returned_items(&f.db, support::shipment_context(f.tenant), &[line])
        .await
        .expect("exact replay");
    assert_eq!(replay.already_restocked, vec![line.return_item_id]);
    assert_eq!(stock_events(&f).await.len(), after.len());
    remove_outbox(&f).await;
    restock_returned_items(&f.db, support::shipment_context(f.tenant), &[line])
        .await
        .expect("already durable movement does not republish");
    assert_eq!(
        rustok_inventory::return_restock::ledger::Entity::find_by_id(line.return_item_id)
            .one(&f.db)
            .await
            .expect("receipt query"),
        Some(saved)
    );
    assert_eq!(f.stock().await, (4, 1));
}

#[tokio::test]
async fn missing_outbox_rolls_back_new_return_receipt_and_stock() {
    let f = Fixture::new().await;
    f.ship(2).await.expect("native consumed history");
    let line = f.returned_line();
    let original = f.allocation().await;
    remove_outbox(&f).await;
    let error = restock_returned_items(&f.db, support::shipment_context(f.tenant), &[line])
        .await
        .expect_err("fresh returned stock needs a durable invalidation");
    assert_eq!(error.code, "inventory.stock_event_persistence_unavailable");
    assert_eq!(f.allocation().await, original);
    assert_eq!(f.stock().await, (3, 1));
    assert!(
        rustok_inventory::return_restock::ledger::Entity::find_by_id(line.return_item_id)
            .one(&f.db)
            .await
            .expect("receipt query")
            .is_none()
    );
}

#[tokio::test]
async fn global_event_counter_overflow_refuses_before_any_native_movement() {
    use rustok_inventory::entities::stock_location;
    let f = Fixture::new().await;
    let original = f.allocation().await;
    let location_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    stock_location::ActiveModel {
        id: Set(location_id),
        tenant_id: Set(f.tenant),
        code: Set(Some(format!("event-bound-{location_id}"))),
        address_line1: Set(None),
        address_line2: Set(None),
        city: Set(None),
        province: Set(None),
        postal_code: Set(None),
        country_code: Set(None),
        phone: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
    }
    .insert(&f.db)
    .await
    .expect("another owned location");
    inventory_level::ActiveModel {
        id: Set(Uuid::new_v4()),
        inventory_item_id: Set(original.inventory_item_id),
        location_id: Set(location_id),
        stocked_quantity: Set(i32::MAX),
        reserved_quantity: Set(0),
        incoming_quantity: Set(0),
        low_stock_threshold: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(&f.db)
    .await
    .expect("representable location, unrepresentable global sum");
    let before = stock_events(&f).await;
    let error = f
        .ship(1)
        .await
        .expect_err("canonical i32 event cannot silently truncate");
    assert_owner_code(error, "inventory.stock_event_quantity_unrepresentable");
    assert!(f.proof().await.is_none());
    assert_eq!(f.current().await.items[0].shipped_quantity, 0);
    assert_eq!(stock_events(&f).await.len(), before.len());
    assert_eq!(f.allocation().await, original);
}
