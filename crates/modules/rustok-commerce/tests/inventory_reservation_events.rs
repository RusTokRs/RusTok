//! Reservation/outbox regression definitions included by inventory_service_test.
//! Not compiled or executed by the authoring agent. SQLite faults exercise local atomicity,
//! not PostgreSQL/MySQL locks, a real relay, broker acknowledgement or Search convergence.

use super::*;
use rustok_api::PortActor;
use rustok_events::{ContractEventEnvelope, ContractEventPayload, InventoryReservationEvent};
use rustok_inventory::{
    InventoryIdentityReservationReleaseRequest, InventoryIdentityReservationRequest,
};
use rustok_outbox::entity::{self as outbox, SysEventStatus};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, Set, Statement};

struct Fixture {
    db: DatabaseConnection,
    service: InventoryService,
    tenant: Uuid,
    actor: Uuid,
    product: Uuid,
    variant: Uuid,
    item: Uuid,
}

async fn fixture() -> Fixture {
    let (db, service, catalog) = setup().await;
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let (product, variant) = create_test_product(&catalog, tenant).await;
    service
        .set_inventory(tenant, actor, variant, 0)
        .await
        .unwrap();
    let saved = entities::product_variant::Entity::find_by_id(variant)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active: entities::product_variant::ActiveModel = saved.into();
    active.inventory_policy = Set("CoNtInUe".to_owned());
    active.update(&db).await.unwrap();
    let item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant))
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .id;
    Fixture {
        db,
        service,
        tenant,
        actor,
        product,
        variant,
        item,
    }
}

fn context(f: &Fixture, actor: Uuid) -> PortContext {
    let mut context = inventory_port_context(f.tenant, "reservation-event-regression", true);
    context.actor = PortActor::user(actor.to_string());
    context
}

fn request(f: &Fixture) -> InventoryIdentityReservationRequest {
    let id = Uuid::new_v4();
    InventoryIdentityReservationRequest {
        reservation_id: id,
        external_id: format!("reservation-event:{id}"),
        variant_id: f.variant,
        quantity: 5,
        line_item_id: Some(Uuid::new_v4()),
        // Saved actor-like metadata must never replace current command context.
        metadata: serde_json::json!({"actor_id": Uuid::new_v4(), "retained": "opaque"}),
    }
}

fn release(
    request: &InventoryIdentityReservationRequest,
) -> InventoryIdentityReservationReleaseRequest {
    InventoryIdentityReservationReleaseRequest {
        reservation_id: request.reservation_id,
        external_id: request.external_id.clone(),
    }
}

async fn facts(f: &Fixture) -> Vec<ContractEventEnvelope> {
    outbox::Entity::find()
        .filter(outbox::Column::EventType.eq("inventory.reservation.changed"))
        .all(&f.db)
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            assert_eq!(row.status, SysEventStatus::Pending);
            let envelope: ContractEventEnvelope = serde_json::from_value(row.payload).unwrap();
            envelope.validate_registered_schema().unwrap();
            assert_eq!(envelope.id(), row.id);
            assert_eq!(envelope.event_type(), row.event_type);
            assert_eq!(
                i16::try_from(envelope.schema_version()).unwrap(),
                row.schema_version
            );
            assert_eq!(envelope.tenant_id(), f.tenant);
            assert_eq!(
                envelope.payload().unwrap(),
                &ContractEventPayload::InventoryReservation(InventoryReservationEvent::Changed {
                    product_id: f.product,
                    variant_id: f.variant
                })
            );
            envelope
        })
        .collect()
}

async fn level(f: &Fixture) -> entities::inventory_level::Model {
    entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(f.item))
        .one(&f.db)
        .await
        .unwrap()
        .unwrap()
}

async fn saved(
    f: &Fixture,
    request: &InventoryIdentityReservationRequest,
) -> entities::reservation_item::Model {
    entities::reservation_item::Entity::find_by_id(request.reservation_id)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap()
}

async fn sql(f: &Fixture, sql: &str) {
    f.db.execute_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .unwrap();
}

#[tokio::test]
async fn negative_backorder_identity_reserve_persists_one_quantity_free_current_actor_fact() {
    let f = fixture().await;
    let request = request(&f);
    let result = in_process_inventory_reservation_identity_port(f.db.clone())
        .reserve_inventory_by_identity(context(&f, f.actor), request)
        .await
        .unwrap();
    assert_eq!(result.available_quantity, -5);
    assert!(result.in_stock);
    assert_eq!(level(&f).await.reserved_quantity, 5);
    let facts = facts(&f).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].actor_id(), Some(f.actor));
    let wire = serde_json::to_value(&facts[0]).unwrap();
    assert_eq!(wire["event"]["event"]["data"].as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn identity_release_uses_current_releasing_actor_and_exact_replays_emit_nothing() {
    let f = fixture().await;
    let request = request(&f);
    let port = in_process_inventory_reservation_identity_port(f.db.clone());
    port.reserve_inventory_by_identity(context(&f, f.actor), request.clone())
        .await
        .unwrap();
    let before = saved(&f, &request).await;
    let replay = port
        .reserve_inventory_by_identity(context(&f, Uuid::new_v4()), request.clone())
        .await
        .unwrap();
    assert_eq!(replay.reserved_quantity, 5);
    assert_eq!(saved(&f, &request).await, before);
    assert_eq!(facts(&f).await.len(), 1);
    let releasing_actor = Uuid::new_v4();
    port.release_inventory_by_identity(context(&f, releasing_actor), release(&request))
        .await
        .unwrap();
    let terminal = saved(&f, &request).await;
    let facts_before = facts(&f).await;
    assert_eq!(facts_before.len(), 2);
    assert_eq!(
        facts_before
            .iter()
            .filter(|fact| fact.actor_id() == Some(releasing_actor))
            .count(),
        1
    );
    assert_ne!(facts_before[0].id(), facts_before[1].id());
    port.release_inventory_by_identity(context(&f, Uuid::new_v4()), release(&request))
        .await
        .unwrap();
    assert_eq!(saved(&f, &request).await, terminal);
    assert_eq!(facts(&f).await.len(), 2);
}

#[tokio::test]
async fn native_quantity_writes_forward_context_and_still_do_not_deduplicate_transport_keys() {
    let f = fixture().await;
    let context = context(&f, f.actor);
    for _ in 0..2 {
        let result = f.service.reserve(&context, f.variant, 3).await.unwrap();
        assert!(result.available_quantity < 0 && result.in_stock);
    }
    assert_eq!(level(&f).await.reserved_quantity, 6);
    assert_eq!(facts(&f).await.len(), 2);
    f.service
        .release_reservation_quantity(&context, f.variant, 4)
        .await
        .unwrap();
    let facts = facts(&f).await;
    assert_eq!(facts.len(), 3);
    assert!(facts.iter().all(|fact| fact.actor_id() == Some(f.actor)));
    assert_eq!(level(&f).await.reserved_quantity, 2);
}

#[tokio::test]
async fn reserve_replay_has_no_new_outbox_dependency() {
    let f = fixture().await;
    let request = request(&f);
    let port = in_process_inventory_reservation_identity_port(f.db.clone());
    port.reserve_inventory_by_identity(context(&f, f.actor), request.clone())
        .await
        .unwrap();
    let before = saved(&f, &request).await;
    let balance = level(&f).await;
    sql(&f, "DROP TABLE sys_events").await;
    port.reserve_inventory_by_identity(context(&f, f.actor), request.clone())
        .await
        .unwrap();
    assert_eq!(saved(&f, &request).await, before);
    assert_eq!(level(&f).await, balance);
}

#[tokio::test]
async fn released_replay_and_zero_quantity_need_no_outbox() {
    let f = fixture().await;
    let request = request(&f);
    let port = in_process_inventory_reservation_identity_port(f.db.clone());
    port.reserve_inventory_by_identity(context(&f, f.actor), request.clone())
        .await
        .unwrap();
    port.release_inventory_by_identity(context(&f, f.actor), release(&request))
        .await
        .unwrap();
    let before = saved(&f, &request).await;
    let balance = level(&f).await;
    sql(&f, "DROP TABLE sys_events").await;
    assert_eq!(
        port.release_inventory_by_identity(context(&f, f.actor), release(&request))
            .await
            .unwrap()
            .released_quantity,
        0
    );
    f.service
        .reserve(&context(&f, f.actor), f.variant, 0)
        .await
        .unwrap();
    f.service
        .release_reservation_quantity(&context(&f, f.actor), f.variant, 0)
        .await
        .unwrap();
    assert_eq!(saved(&f, &request).await, before);
    assert_eq!(level(&f).await, balance);
}

#[tokio::test]
async fn missing_outbox_rolls_back_new_identity_allocation() {
    let f = fixture().await;
    let request = request(&f);
    let before = level(&f).await;
    sql(&f, "DROP TABLE sys_events").await;
    assert!(
        in_process_inventory_reservation_identity_port(f.db.clone())
            .reserve_inventory_by_identity(context(&f, f.actor), request.clone())
            .await
            .is_err()
    );
    assert_eq!(level(&f).await, before);
    assert!(
        entities::reservation_item::Entity::find_by_id(request.reservation_id)
            .one(&f.db)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn missing_outbox_rolls_back_identity_release_and_preserves_terminal_metadata() {
    let f = fixture().await;
    let request = request(&f);
    let port = in_process_inventory_reservation_identity_port(f.db.clone());
    port.reserve_inventory_by_identity(context(&f, f.actor), request.clone())
        .await
        .unwrap();
    let before = saved(&f, &request).await;
    let balance = level(&f).await;
    sql(&f, "DROP TABLE sys_events").await;
    assert!(
        port.release_inventory_by_identity(context(&f, f.actor), release(&request))
            .await
            .is_err()
    );
    assert_eq!(saved(&f, &request).await, before);
    assert_eq!(level(&f).await, balance);
}

#[tokio::test]
async fn missing_outbox_rolls_back_anonymous_reserve_and_release() {
    let f = fixture().await;
    f.service
        .reserve(&context(&f, f.actor), f.variant, 5)
        .await
        .unwrap();
    let before = level(&f).await;
    let rows = entities::reservation_item::Entity::find()
        .filter(entities::reservation_item::Column::InventoryItemId.eq(f.item))
        .all(&f.db)
        .await
        .unwrap();
    sql(&f, "DROP TABLE sys_events").await;
    assert!(
        f.service
            .reserve(&context(&f, f.actor), f.variant, 1)
            .await
            .is_err()
    );
    assert!(
        f.service
            .release_reservation_quantity(&context(&f, f.actor), f.variant, 1)
            .await
            .is_err()
    );
    assert_eq!(level(&f).await, before);
    assert_eq!(
        entities::reservation_item::Entity::find()
            .filter(entities::reservation_item::Column::InventoryItemId.eq(f.item))
            .all(&f.db)
            .await
            .unwrap(),
        rows
    );
}

#[tokio::test]
async fn suppressed_identity_receipt_insert_cannot_publish_or_acknowledge_a_reservation() {
    let f = fixture().await;
    let request = request(&f);
    let before = level(&f).await;
    sql(&f, "CREATE TRIGGER suppress_identity_insert BEFORE INSERT ON reservation_items BEGIN SELECT RAISE(IGNORE); END").await;
    assert!(
        in_process_inventory_reservation_identity_port(f.db.clone())
            .reserve_inventory_by_identity(context(&f, f.actor), request.clone())
            .await
            .is_err()
    );
    assert_eq!(level(&f).await, before);
    assert!(facts(&f).await.is_empty());
    assert!(
        entities::reservation_item::Entity::find_by_id(request.reservation_id)
            .one(&f.db)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn suppressed_identity_receipt_release_rolls_back_counter_and_emits_no_new_fact() {
    let f = fixture().await;
    let request = request(&f);
    let port = in_process_inventory_reservation_identity_port(f.db.clone());
    port.reserve_inventory_by_identity(context(&f, f.actor), request.clone())
        .await
        .unwrap();
    let before = saved(&f, &request).await;
    let balance = level(&f).await;
    sql(&f, "CREATE TRIGGER suppress_identity_release BEFORE UPDATE ON reservation_items BEGIN SELECT RAISE(IGNORE); END").await;
    assert!(
        port.release_inventory_by_identity(context(&f, f.actor), release(&request))
            .await
            .is_err()
    );
    assert_eq!(saved(&f, &request).await, before);
    assert_eq!(level(&f).await, balance);
    assert_eq!(facts(&f).await.len(), 1);
}

#[tokio::test]
async fn invalid_current_human_actor_refuses_fresh_movement_without_using_saved_metadata() {
    let f = fixture().await;
    let request = request(&f);
    let before = level(&f).await;
    let invalid = context(&f, Uuid::nil());
    assert!(
        in_process_inventory_reservation_identity_port(f.db.clone())
            .reserve_inventory_by_identity(invalid.clone(), request)
            .await
            .is_err()
    );
    assert!(f.service.reserve(&invalid, f.variant, 1).await.is_err());
    assert_eq!(level(&f).await, before);
    assert!(facts(&f).await.is_empty());
}

#[tokio::test]
async fn service_and_system_contexts_never_fabricate_human_actor_ids() {
    let f = fixture().await;
    for actor in [
        PortActor::service("inventory-regression"),
        PortActor::system(),
    ] {
        let mut context = context(&f, f.actor);
        context.actor = actor;
        f.service.reserve(&context, f.variant, 1).await.unwrap();
    }
    let facts = facts(&f).await;
    assert_eq!(facts.len(), 2);
    assert!(facts.iter().all(|fact| fact.actor_id().is_none()));
}

#[tokio::test]
async fn checked_identity_reservation_overflow_refuses_without_a_new_fact() {
    let f = fixture().await;
    let mut active: entities::inventory_level::ActiveModel = level(&f).await.into();
    active.reserved_quantity = Set(i32::MAX);
    active.update(&f.db).await.unwrap();
    let request = request(&f);
    let before = level(&f).await;
    assert!(
        in_process_inventory_reservation_identity_port(f.db.clone())
            .reserve_inventory_by_identity(context(&f, f.actor), request)
            .await
            .is_err()
    );
    assert_eq!(level(&f).await, before);
    assert!(facts(&f).await.is_empty());
}

#[tokio::test]
async fn suppressed_canonical_contract_insert_rolls_back_all_local_reservation_effects() {
    let f = fixture().await;
    let before = level(&f).await;
    sql(&f, "CREATE TRIGGER suppress_reservation_fact BEFORE INSERT ON sys_events WHEN NEW.event_type = 'inventory.reservation.changed' BEGIN SELECT RAISE(IGNORE); END").await;
    assert!(
        f.service
            .reserve(&context(&f, f.actor), f.variant, 2)
            .await
            .is_err()
    );
    assert!(
        in_process_inventory_reservation_identity_port(f.db.clone())
            .reserve_inventory_by_identity(context(&f, f.actor), request(&f))
            .await
            .is_err()
    );
    assert_eq!(level(&f).await, before);
    assert!(facts(&f).await.is_empty());
    assert_eq!(
        entities::reservation_item::Entity::find()
            .filter(entities::reservation_item::Column::InventoryItemId.eq(f.item))
            .count(&f.db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn identity_result_cannot_clamp_unrepresentable_negative_availability_into_success() {
    let f = fixture().await;
    let mut active: entities::inventory_level::ActiveModel = level(&f).await.into();
    active.stocked_quantity = Set(i32::MIN);
    active.update(&f.db).await.unwrap();
    let before = level(&f).await;
    let mut request = request(&f);
    request.quantity = 1;
    assert!(
        in_process_inventory_reservation_identity_port(f.db.clone())
            .reserve_inventory_by_identity(context(&f, f.actor), request.clone())
            .await
            .is_err()
    );
    assert_eq!(level(&f).await, before);
    assert!(facts(&f).await.is_empty());
    assert!(
        entities::reservation_item::Entity::find_by_id(request.reservation_id)
            .one(&f.db)
            .await
            .unwrap()
            .is_none()
    );
}
