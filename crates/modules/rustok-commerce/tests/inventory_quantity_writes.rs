//! Shared native/Product quantity regressions, registered by catalog_service_test. Not run here.

use super::{create_test_product_input, setup};
use chrono::{Duration, Utc};
use rustok_api::Patch;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_inventory::entities::{
    inventory_item, inventory_level, reservation_item, stock_location,
};
use rustok_inventory::{
    BootstrapService, InventoryService, QuantityWriteBlocker as B, QuantityWriteError,
};
use rustok_pricing::entities::price;
use rustok_product::dto::UpdateVariantInput;
use rustok_product::entities::{product, product_variant};
use rustok_product::{CatalogService, CommerceError, ProductCatalogCommandPort};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, Set, Statement, TransactionTrait,
};
use uuid::Uuid;

#[derive(Debug, PartialEq)]
struct Snapshot {
    products: Vec<product::Model>,
    variants: Vec<product_variant::Model>,
    items: Vec<inventory_item::Model>,
    levels: Vec<inventory_level::Model>,
    locations: Vec<stock_location::Model>,
    reservations: Vec<reservation_item::Model>,
    prices: Vec<price::Model>,
    events: Vec<rustok_outbox::entity::Model>,
}

async fn snapshot(db: &DatabaseConnection) -> Snapshot {
    Snapshot {
        products: product::Entity::find()
            .order_by_asc(product::Column::Id)
            .all(db)
            .await
            .unwrap(),
        variants: product_variant::Entity::find()
            .order_by_asc(product_variant::Column::Id)
            .all(db)
            .await
            .unwrap(),
        items: inventory_item::Entity::find()
            .order_by_asc(inventory_item::Column::Id)
            .all(db)
            .await
            .unwrap(),
        levels: inventory_level::Entity::find()
            .order_by_asc(inventory_level::Column::Id)
            .all(db)
            .await
            .unwrap(),
        locations: stock_location::Entity::find()
            .order_by_asc(stock_location::Column::Id)
            .all(db)
            .await
            .unwrap(),
        reservations: reservation_item::Entity::find()
            .order_by_asc(reservation_item::Column::Id)
            .all(db)
            .await
            .unwrap(),
        prices: price::Entity::find()
            .order_by_asc(price::Column::Id)
            .all(db)
            .await
            .unwrap(),
        events: rustok_outbox::SysEvents::find()
            .order_by_asc(rustok_outbox::entity::Column::Id)
            .all(db)
            .await
            .unwrap(),
    }
}

struct Fixture {
    tenant: Uuid,
    actor: Uuid,
    variant: Uuid,
    item: Uuid,
    level: Uuid,
    location: Uuid,
}

async fn fixture(db: &DatabaseConnection, catalog: &CatalogService) -> Fixture {
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = catalog
        .create_product(tenant, actor, create_test_product_input())
        .await
        .unwrap();
    let variant = product.variants[0].id;
    let item = inventory_item::Entity::find()
        .filter(inventory_item::Column::VariantId.eq(variant))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let level = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(item.id))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    Fixture {
        tenant,
        actor,
        variant,
        item: item.id,
        level: level.id,
        location: level.location_id,
    }
}

fn service(db: &DatabaseConnection) -> InventoryService {
    InventoryService::new(db.clone(), mock_transactional_event_bus())
}

async fn counters(
    db: &DatabaseConnection,
    level: Uuid,
    stocked: i32,
    reserved: i32,
    incoming: i32,
) {
    let mut row = inventory_level::Entity::find_by_id(level)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    row.stocked_quantity = Set(stocked);
    row.reserved_quantity = Set(reserved);
    row.incoming_quantity = Set(incoming);
    row.low_stock_threshold = Set(Some(777));
    row.update(db).await.unwrap();
}

async fn another_location(
    db: &DatabaseConnection,
    f: &Fixture,
    tenant: Uuid,
) -> stock_location::Model {
    let mut row = stock_location::Entity::find_by_id(f.location)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model()
        .reset_all();
    row.id = Set(Uuid::new_v4());
    row.tenant_id = Set(tenant);
    row.code = Set(None);
    row.created_at = Set((Utc::now() + Duration::seconds(1)).into());
    row.insert(db).await.unwrap()
}

async fn another_level(
    db: &DatabaseConnection,
    f: &Fixture,
    location: Uuid,
    stocked: i32,
    reserved: i32,
) -> inventory_level::Model {
    inventory_level::ActiveModel {
        id: Set(Uuid::new_v4()),
        inventory_item_id: Set(f.item),
        location_id: Set(location),
        stocked_quantity: Set(stocked),
        reserved_quantity: Set(reserved),
        incoming_quantity: Set(3),
        low_stock_threshold: Set(Some(888)),
        updated_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .unwrap()
}

fn quantity_input(quantity: i32) -> UpdateVariantInput {
    UpdateVariantInput {
        inventory_quantity: Some(quantity),
        ..Default::default()
    }
}

fn peer_changes(quantity: i32) -> UpdateVariantInput {
    let mut prices = create_test_product_input().variants.remove(0).prices;
    prices[0].amount = rust_decimal::Decimal::new(12500, 2);
    UpdateVariantInput {
        barcode: Patch::Set("changed-before-quantity".into()),
        prices: Some(prices),
        inventory_quantity: Some(quantity),
        ..Default::default()
    }
}

async fn native_refused(db: &DatabaseConnection, f: &Fixture, quantity: i32, reason: B) {
    let before = snapshot(db).await;
    let error = service(db)
        .set_inventory(f.tenant, f.actor, f.variant, quantity)
        .await
        .unwrap_err();
    assert!(matches!(error, QuantityWriteError::Rejected(actual) if actual == reason));
    assert_eq!(snapshot(db).await, before);
}

async fn product_refused(
    db: &DatabaseConnection,
    catalog: &CatalogService,
    f: &Fixture,
    quantity: i32,
    reason: B,
) {
    let before = snapshot(db).await;
    let error = catalog
        .update_variant(f.tenant, f.actor, f.variant, peer_changes(quantity))
        .await
        .unwrap_err();
    assert!(matches!(error, CommerceError::InventoryQuantityBlocked(actual) if actual == reason));
    assert_eq!(
        snapshot(db).await,
        before,
        "Product, prices, Inventory, timestamps and prior envelopes must roll back"
    );
}

#[tokio::test]
async fn native_and_product_quantity_use_the_same_global_reserved_multi_location_plan() {
    for product_path in [false, true] {
        let (db, catalog) = setup().await;
        let f = fixture(&db, &catalog).await;
        counters(&db, f.level, 10, 4, 7).await;
        let second = another_location(&db, &f, f.tenant).await;
        let other = another_level(&db, &f, second.id, 7, 5).await;
        let before = snapshot(&db).await;
        if product_path {
            let result = catalog
                .update_variant(f.tenant, f.actor, f.variant, quantity_input(12))
                .await
                .unwrap();
            assert_eq!(result.inventory_quantity, 12);
            assert!(result.in_stock);
        } else {
            let result = service(&db)
                .set_variant_quantity(f.tenant, f.actor, f.variant, 12)
                .await
                .unwrap();
            assert_eq!(result.quantity, 12);
            assert!(result.in_stock);
        }
        let after = snapshot(&db).await;
        let changed = after.levels.iter().find(|row| row.id == f.level).unwrap();
        let mut expected = before
            .levels
            .iter()
            .find(|row| row.id == f.level)
            .unwrap()
            .clone();
        expected.stocked_quantity = 14;
        expected.updated_at = changed.updated_at;
        assert_eq!(*changed, expected);
        assert_eq!(
            after.levels.iter().find(|row| row.id == other.id).unwrap(),
            &other
        );
        assert_eq!(after.items, before.items);
        assert_eq!(after.locations, before.locations);
        assert_eq!(after.reservations, before.reservations);
        let events = after
            .events
            .iter()
            .filter(|row| row.event_type == "inventory.updated")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        let envelope: EventEnvelope = serde_json::from_value(events[0].payload.clone()).unwrap();
        assert_eq!(envelope.tenant_id, f.tenant);
        assert_eq!(envelope.actor_id, Some(f.actor));
        assert!(
            matches!(envelope.event, DomainEvent::InventoryUpdated { variant_id, location_id, old_quantity: 8, new_quantity: 12, .. }
            if variant_id == f.variant && location_id == f.location)
        );
    }
}

#[tokio::test]
async fn quantity_noops_preserve_signed_archived_state_and_do_not_need_outbox_storage() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    counters(&db, f.level, 1, 4, 6).await;
    let mut variant = product_variant::Entity::find_by_id(f.variant)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    variant.inventory_policy = Set("continue".into());
    variant.update(&db).await.unwrap();
    let mut location = stock_location::Entity::find_by_id(f.location)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    location.deleted_at = Set(Some(Utc::now().into()));
    location.update(&db).await.unwrap();
    let before = snapshot(&db).await;
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "DROP TABLE sys_events",
    ))
    .await
    .unwrap();
    assert_eq!(
        service(&db)
            .set_inventory(f.tenant, f.actor, f.variant, -3)
            .await
            .unwrap(),
        -3
    );
    assert_eq!(
        service(&db)
            .adjust_variant_quantity(f.tenant, f.actor, f.variant, 0, None)
            .await
            .unwrap()
            .quantity,
        -3
    );
    assert_eq!(
        inventory_level::Entity::find()
            .order_by_asc(inventory_level::Column::Id)
            .all(&db)
            .await
            .unwrap(),
        before.levels
    );
    assert_eq!(
        inventory_item::Entity::find()
            .order_by_asc(inventory_item::Column::Id)
            .all(&db)
            .await
            .unwrap(),
        before.items
    );
    assert_eq!(
        stock_location::Entity::find()
            .order_by_asc(stock_location::Column::Id)
            .all(&db)
            .await
            .unwrap(),
        before.locations
    );
}

#[tokio::test]
async fn quantity_missing_item_or_level_never_succeeds_or_recreates_identity_even_for_zero() {
    for missing_item in [false, true] {
        let (db, catalog) = setup().await;
        let f = fixture(&db, &catalog).await;
        inventory_level::Entity::delete_many()
            .filter(inventory_level::Column::InventoryItemId.eq(f.item))
            .exec(&db)
            .await
            .unwrap();
        if missing_item {
            inventory_item::Entity::delete_by_id(f.item)
                .exec(&db)
                .await
                .unwrap();
        }
        for quantity in [0, 1] {
            native_refused(&db, &f, quantity, B::IdentityUnavailable).await;
            product_refused(&db, &catalog, &f, quantity, B::IdentityUnavailable).await;
        }
    }
}

#[tokio::test]
async fn quantity_current_tenant_proof_precedes_tenantless_item_and_original_location_reads() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let before = snapshot(&db).await;
    let error = service(&db)
        .set_inventory(Uuid::new_v4(), f.actor, f.variant, 1)
        .await
        .unwrap_err();
    assert!(matches!(error, QuantityWriteError::VariantNotFound(id) if id == f.variant));
    assert_eq!(snapshot(&db).await, before);
    let foreign = another_location(&db, &f, Uuid::new_v4()).await;
    let mut row = inventory_level::Entity::find_by_id(f.level)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    row.location_id = Set(foreign.id);
    row.update(&db).await.unwrap();
    native_refused(&db, &f, 1, B::StateUnverified).await;
    product_refused(&db, &catalog, &f, 1, B::StateUnverified).await;
    // A tenant-labelled Variant cannot authorize a foreign Product parent in the stock event.
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let foreign = fixture(&db, &catalog).await;
    let parent = product_variant::Entity::find_by_id(foreign.variant)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .product_id;
    let mut variant = product_variant::Entity::find_by_id(f.variant)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    variant.product_id = Set(parent);
    variant.update(&db).await.unwrap();
    native_refused(&db, &f, 1, B::IdentityUnavailable).await;
}

#[tokio::test]
async fn quantity_default_fallback_uses_oldest_active_location_with_uuid_ties_not_first_level() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let original = stock_location::Entity::find_by_id(f.location)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut primary = original.clone().into_active_model();
    primary.code = Set(None);
    primary.update(&db).await.unwrap();
    let mut second = original.into_active_model().reset_all();
    second.id = Set(Uuid::from_u128(1));
    second.code = Set(None);
    let second = second.insert(&db).await.unwrap();
    let target = another_level(&db, &f, second.id, 0, 0).await;
    service(&db)
        .set_inventory(f.tenant, f.actor, f.variant, 1)
        .await
        .unwrap();
    assert_eq!(
        inventory_level::Entity::find_by_id(target.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .stocked_quantity,
        1
    );
    assert_eq!(
        inventory_level::Entity::find_by_id(f.level)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .stocked_quantity,
        0
    );
}

#[tokio::test]
async fn quantity_missing_active_target_level_never_rehomes_or_creates_a_level() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let mut location = stock_location::Entity::find_by_id(f.location)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    location.deleted_at = Set(Some(Utc::now().into()));
    location.update(&db).await.unwrap();
    another_location(&db, &f, f.tenant).await;
    native_refused(&db, &f, 1, B::LocationUnavailable).await;
    product_refused(&db, &catalog, &f, 1, B::LocationUnavailable).await;
}

#[tokio::test]
async fn quantity_checked_storage_and_delta_extremes_refuse_before_any_event_or_effect() {
    for (stocked, reserved, delta, reason) in [
        (i32::MAX, 0, 1, B::QuantityUnrepresentable),
        (i32::MAX, i32::MAX, 1, B::QuantityUnrepresentable),
        (2, 0, i32::MIN, B::InsufficientStock),
    ] {
        let (db, catalog) = setup().await;
        let f = fixture(&db, &catalog).await;
        counters(&db, f.level, stocked, reserved, 0).await;
        let before = snapshot(&db).await;
        let error = service(&db)
            .adjust_variant_quantity(f.tenant, f.actor, f.variant, delta, None)
            .await
            .unwrap_err();
        assert!(matches!(error, QuantityWriteError::Rejected(actual) if actual == reason));
        assert_eq!(snapshot(&db).await, before);
    }
}

#[tokio::test]
async fn quantity_complete_level_overflow_refuses_without_picking_a_partial_default() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let location = stock_location::Entity::find_by_id(f.location)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let level = inventory_level::Entity::find_by_id(f.level)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    for _ in 0..64 {
        let mut locations = Vec::new();
        let mut levels = Vec::new();
        for _ in 0..64 {
            let id = Uuid::new_v4();
            let mut loc = location.clone().into_active_model().reset_all();
            loc.id = Set(id);
            loc.code = Set(None);
            let mut row = level.clone().into_active_model().reset_all();
            row.id = Set(Uuid::new_v4());
            row.location_id = Set(id);
            locations.push(loc);
            levels.push(row);
        }
        stock_location::Entity::insert_many(locations)
            .exec(&db)
            .await
            .unwrap();
        inventory_level::Entity::insert_many(levels)
            .exec(&db)
            .await
            .unwrap();
    }
    native_refused(&db, &f, 1, B::LimitExceeded).await;
    product_refused(&db, &catalog, &f, 1, B::LimitExceeded).await;
}

#[tokio::test]
async fn quantity_exact_one_target_effect_failure_rolls_back_product_peer_writes() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    db.execute_raw(Statement::from_string(DbBackend::Sqlite,
        "CREATE TRIGGER refuse_quantity BEFORE UPDATE OF stocked_quantity ON inventory_levels BEGIN SELECT RAISE(IGNORE); END")).await.unwrap();
    native_refused(&db, &f, 1, B::ConcurrentChange).await;
    product_refused(&db, &catalog, &f, 1, B::ConcurrentChange).await;
}

#[tokio::test]
async fn quantity_stock_notification_failure_rolls_back_native_and_product_changes() {
    for ignored in [false, true] {
        for product_path in [false, true] {
            let (db, catalog) = setup().await;
            let f = fixture(&db, &catalog).await;
            let refusal = if ignored {
                "RAISE(IGNORE)"
            } else {
                "RAISE(ABORT, 'private-provider-details')"
            };
            db.execute_raw(Statement::from_string(DbBackend::Sqlite, format!(
                "CREATE TRIGGER refuse_quantity_event BEFORE INSERT ON sys_events WHEN NEW.event_type = 'inventory.updated' BEGIN SELECT {refusal}; END"))).await.unwrap();
            let before = snapshot(&db).await;
            if product_path {
                let error = catalog
                    .update_variant(f.tenant, f.actor, f.variant, peer_changes(1))
                    .await
                    .unwrap_err();
                assert!(matches!(
                    &error,
                    CommerceError::Database(_)
                        | CommerceError::InventoryQuantityBlocked(B::PublicationUnverified)
                ));
                assert!(
                    !rustok_product::map_product_public_error(
                        &error,
                        "update_variant",
                        "quantity_test"
                    )
                    .message
                    .contains("private-provider-details")
                );
            } else {
                assert!(matches!(
                    service(&db)
                        .set_inventory(f.tenant, f.actor, f.variant, 1)
                        .await
                        .unwrap_err(),
                    QuantityWriteError::Publication(_)
                ));
            }
            assert_eq!(snapshot(&db).await, before);
        }
    }
}

#[tokio::test]
async fn quantity_adjust_low_stock_publication_failure_rolls_back_first_stock_event_and_counters() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    counters(&db, f.level, 2, 0, 0).await;
    db.execute_raw(Statement::from_string(DbBackend::Sqlite,
        "CREATE TRIGGER refuse_low_event BEFORE INSERT ON sys_events WHEN NEW.event_type = 'inventory.low' BEGIN SELECT RAISE(ABORT, 'private-provider-details'); END")).await.unwrap();
    let before = snapshot(&db).await;
    assert!(matches!(
        service(&db)
            .adjust_variant_quantity(f.tenant, f.actor, f.variant, 1, None)
            .await
            .unwrap_err(),
        QuantityWriteError::Publication(_)
    ));
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn quantity_later_product_publication_failure_rolls_back_stock_price_and_variant_together() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let event_type = DomainEvent::VariantUpdated {
        variant_id: f.variant,
        product_id: Uuid::new_v4(),
    }
    .event_type();
    db.execute_raw(Statement::from_string(DbBackend::Sqlite, format!(
        "CREATE TRIGGER refuse_product_event BEFORE INSERT ON sys_events WHEN NEW.event_type = '{event_type}' BEGIN SELECT RAISE(ABORT, 'private-provider-details'); END"))).await.unwrap();
    let before = snapshot(&db).await;
    assert!(
        catalog
            .update_variant(f.tenant, f.actor, f.variant, peer_changes(1))
            .await
            .is_err()
    );
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn quantity_bootstrap_hook_never_commits_independently_of_caller_rollback() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let before = snapshot(&db).await;
    let txn = db.begin().await.unwrap();
    BootstrapService::update_initial_quantity_in_tx(&txn, f.tenant, f.actor, f.variant, 3)
        .await
        .unwrap();
    assert_eq!(
        inventory_level::Entity::find_by_id(f.level)
            .one(&txn)
            .await
            .unwrap()
            .unwrap()
            .stocked_quantity,
        3
    );
    txn.rollback().await.unwrap();
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn quantity_product_receipt_replay_preserves_original_result_without_a_new_stock_event() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let context = super::catalog_receipt_context(f.tenant, f.actor, "quantity-result-replay");
    let input = quantity_input(12);
    let first = ProductCatalogCommandPort::update_variant(
        &catalog,
        context.clone(),
        f.variant,
        input.clone(),
    )
    .await
    .unwrap();
    service(&db)
        .set_inventory(f.tenant, f.actor, f.variant, 15)
        .await
        .unwrap();
    let before = snapshot(&db).await;
    let replay = ProductCatalogCommandPort::update_variant(&catalog, context, f.variant, input)
        .await
        .unwrap();
    assert_eq!(first.inventory_quantity, 12);
    assert_eq!(replay.inventory_quantity, 12);
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn quantity_product_terminal_refusal_replays_after_independent_stock_reconciliation() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    let other = another_location(&db, &f, f.tenant).await;
    let other = another_level(&db, &f, other.id, 5, 0).await;
    let context = super::catalog_receipt_context(f.tenant, f.actor, "quantity-refusal-replay");
    let first = ProductCatalogCommandPort::update_variant(
        &catalog,
        context.clone(),
        f.variant,
        quantity_input(0),
    )
    .await
    .unwrap_err();
    assert_eq!(first.code, "product.inventory_quantity_insufficient");
    assert!(rustok_product::product_catalog_command_error_is_terminal(
        &first
    ));
    counters(&db, other.id, 0, 0, 3).await;
    let before = snapshot(&db).await;
    let replay =
        ProductCatalogCommandPort::update_variant(&catalog, context, f.variant, quantity_input(0))
            .await
            .unwrap_err();
    assert_eq!(first, replay);
    assert_eq!(snapshot(&db).await, before);
}

#[tokio::test]
async fn quantity_negative_legacy_event_snapshot_is_explicit_and_rolls_back_product_peer_writes() {
    let (db, catalog) = setup().await;
    let f = fixture(&db, &catalog).await;
    counters(&db, f.level, 1, 4, 0).await;
    let mut row = product_variant::Entity::find_by_id(f.variant)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    row.inventory_policy = Set("continue".into());
    row.update(&db).await.unwrap();
    native_refused(&db, &f, 0, B::EventUnrepresentable).await;
    product_refused(&db, &catalog, &f, 0, B::EventUnrepresentable).await;
}
