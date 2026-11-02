//! Native persistence regression definitions, registered by inventory_service_test.
//! The maintainer runs them; this source continuation executes neither tests nor SQL.

use super::*;
use rustok_inventory::{
    InventoryReservationReleaseRequest, InventoryReservationRequest,
    in_process_inventory_reservation_port,
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, Set, Statement};

struct Stock {
    db: DatabaseConnection,
    service: InventoryService,
    tenant: Uuid,
    variant: Uuid,
    item: Uuid,
    default_location: Uuid,
    other_location: Uuid,
}

async fn stock() -> Stock {
    let (db, service, catalog) = setup().await;
    let tenant = Uuid::new_v4();
    let (_, variant) = create_test_product(&catalog, tenant).await;
    service
        .set_inventory(tenant, Uuid::new_v4(), variant, 20)
        .await
        .unwrap();
    let item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let default = entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(item.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let saved_location = entities::stock_location::Entity::find_by_id(default.location_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut location: entities::stock_location::ActiveModel = saved_location.into();
    location.metadata =
        Set(serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["web"]}}));
    location.update(&db).await.unwrap();
    let other_location = Uuid::new_v4();
    let now = chrono::Utc::now();
    entities::stock_location::ActiveModel {
        id: Set(other_location),
        tenant_id: Set(tenant),
        code: Set(Some(format!("quantity-other-{other_location}"))),
        metadata: Set(
            serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["mobile"]}}),
        ),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    entities::inventory_level::ActiveModel {
        id: Set(Uuid::new_v4()),
        inventory_item_id: Set(item.id),
        location_id: Set(other_location),
        stocked_quantity: Set(100),
        reserved_quantity: Set(0),
        incoming_quantity: Set(0),
        low_stock_threshold: Set(None),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .unwrap();
    Stock {
        db,
        service,
        tenant,
        variant,
        item: item.id,
        default_location: default.location_id,
        other_location,
    }
}

async fn level(stock: &Stock, location: Uuid) -> entities::inventory_level::Model {
    entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(stock.item))
        .filter(entities::inventory_level::Column::LocationId.eq(location))
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap()
}

async fn allocation(stock: &Stock, id: Uuid) -> entities::reservation_item::Model {
    entities::reservation_item::Entity::find_by_id(id)
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap()
}

async fn anonymous(
    stock: &Stock,
    location: Uuid,
    quantity: i32,
    sequence: i64,
) -> entities::reservation_item::Model {
    let existing = level(stock, location).await;
    let reserved = existing.reserved_quantity.checked_add(quantity).unwrap();
    let mut active: entities::inventory_level::ActiveModel = existing.into();
    active.reserved_quantity = Set(reserved);
    active.update(&stock.db).await.unwrap();
    let now = chrono::DateTime::from_timestamp(1_700_000_000 + sequence, 0).unwrap();
    entities::reservation_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        inventory_item_id: Set(stock.item),
        location_id: Set(location),
        quantity: Set(quantity),
        line_item_id: Set(None),
        description: Set(Some("Retained anonymous quantity fixture".to_string())),
        external_id: Set(None),
        metadata: Set(serde_json::json!({"source": "legacy_inventory_service", "variant_id": stock.variant, "retained": "opaque"})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        deleted_at: Set(None),
    }.insert(&stock.db).await.unwrap()
}

async fn keyed(stock: &Stock) -> entities::reservation_item::Model {
    let id = Uuid::new_v4();
    in_process_inventory_reservation_identity_port(stock.db.clone())
        .reserve_inventory_by_identity(
            inventory_port_context(stock.tenant, "quantity-keyed-fixture", true)
                .with_channel("web"),
            InventoryIdentityReservationRequest {
                reservation_id: id,
                external_id: format!("quantity-keyed:{id}"),
                variant_id: stock.variant,
                quantity: 4,
                line_item_id: Some(Uuid::new_v4()),
                metadata: serde_json::json!({"source": "quantity_release_regression"}),
            },
        )
        .await
        .unwrap();
    allocation(stock, id).await
}

#[tokio::test]
async fn quantity_release_cannot_consume_another_consumers_keyed_reservation() {
    let stock = stock().await;
    let saved = keyed(&stock).await;
    let before = level(&stock, stock.default_location).await;
    let error = stock
        .service
        .release_reservation_quantity(
            &inventory_port_context(stock.tenant, "native-quantity-release", true),
            stock.variant,
            2,
        )
        .await
        .expect_err("keyed quantity is not an anonymous pool");
    assert!(matches!(error, CommerceError::Validation(_)));
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(level(&stock, stock.default_location).await, before);
}

#[tokio::test]
async fn anonymous_release_uses_original_hidden_location_and_preserves_identity_row() {
    let stock = stock().await;
    let saved = keyed(&stock).await;
    let anonymous = anonymous(&stock, stock.other_location, 3, 1).await;
    let keyed_level = level(&stock, stock.default_location).await;
    let other = level(&stock, stock.other_location).await;
    let result = stock
        .service
        .release_reservation_quantity(
            &inventory_port_context(stock.tenant, "native-quantity-release", true),
            stock.variant,
            2,
        )
        .await
        .unwrap();
    assert_eq!(result.released_quantity, 2);
    assert_eq!(result.available_quantity, 115);
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(level(&stock, stock.default_location).await, keyed_level);
    let remaining = allocation(&stock, anonymous.id).await;
    assert_eq!(remaining.quantity, 1);
    assert_eq!(remaining.metadata, anonymous.metadata);
    assert_eq!(remaining.location_id, anonymous.location_id);
    assert_eq!(remaining.created_at, anonymous.created_at);
    assert!(remaining.deleted_at.is_none());
    let changed = level(&stock, stock.other_location).await;
    assert_eq!(changed.reserved_quantity, 1);
    assert_eq!(changed.stocked_quantity, other.stocked_quantity);
    assert_eq!(changed.incoming_quantity, other.incoming_quantity);
}

#[tokio::test]
async fn multi_location_anonymous_release_updates_each_original_level_and_preserves_metadata() {
    let stock = stock().await;
    let first = anonymous(&stock, stock.other_location, 3, 1).await;
    let second = anonymous(&stock, stock.default_location, 2, 2).await;
    stock
        .service
        .release_reservation_quantity(
            &inventory_port_context(stock.tenant, "native-quantity-release", true),
            stock.variant,
            4,
        )
        .await
        .unwrap();
    let first_after = allocation(&stock, first.id).await;
    let second_after = allocation(&stock, second.id).await;
    assert_eq!(first_after.quantity, 0);
    assert!(first_after.deleted_at.is_some());
    assert_eq!(first_after.metadata, first.metadata);
    assert_eq!(first_after.created_at, first.created_at);
    assert_eq!(second_after.quantity, 1);
    assert!(second_after.deleted_at.is_none());
    assert_eq!(second_after.metadata, second.metadata);
    assert_eq!(
        level(&stock, stock.other_location).await.reserved_quantity,
        0
    );
    assert_eq!(
        level(&stock, stock.default_location)
            .await
            .reserved_quantity,
        1
    );
}

#[tokio::test]
async fn missing_original_level_refuses_without_releasing_another_warehouse() {
    let stock = stock().await;
    let saved = anonymous(&stock, stock.other_location, 3, 1).await;
    let missing = level(&stock, stock.other_location).await;
    entities::inventory_level::Entity::delete_by_id(missing.id)
        .exec(&stock.db)
        .await
        .unwrap();
    let default = level(&stock, stock.default_location).await;
    assert!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(stock.tenant, "native-quantity-release", true),
                stock.variant,
                2
            )
            .await
            .is_err()
    );
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(level(&stock, stock.default_location).await, default);
    assert!(
        entities::inventory_level::Entity::find_by_id(missing.id)
            .one(&stock.db)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn all_terminal_markers_including_false_and_zero_refuse_before_partial_release() {
    let stock = stock().await;
    let first = anonymous(&stock, stock.default_location, 3, 1).await;
    let terminal = anonymous(&stock, stock.other_location, 3, 2).await;
    let before = level(&stock, stock.default_location).await;
    for marker in [
        "inventory_disposition",
        "superseded_by",
        "released_at",
        "disposed_at",
        "superseded_at",
    ] {
        for value in [serde_json::json!(false), serde_json::json!(0)] {
            let mut metadata = terminal.metadata.clone();
            metadata[marker] = value;
            let mut active: entities::reservation_item::ActiveModel = terminal.clone().into();
            active.metadata = Set(metadata);
            let saved = active.update(&stock.db).await.unwrap();
            assert!(
                stock
                    .service
                    .release_reservation_quantity(
                        &inventory_port_context(stock.tenant, "native-quantity-release", true),
                        stock.variant,
                        1
                    )
                    .await
                    .is_err()
            );
            assert_eq!(allocation(&stock, first.id).await, first);
            assert_eq!(allocation(&stock, terminal.id).await, saved);
            assert_eq!(level(&stock, stock.default_location).await, before);
            assert_eq!(
                level(&stock, stock.other_location).await.reserved_quantity,
                3
            );
        }
    }
}

#[tokio::test]
async fn positive_quantity_with_unknown_provenance_or_only_counters_is_not_release_authority() {
    let stock = stock().await;
    let original = anonymous(&stock, stock.other_location, 3, 1).await;
    let mut active: entities::reservation_item::ActiveModel = original.into();
    active.metadata = Set(serde_json::json!({"variant_id": stock.variant}));
    let unverified = active.update(&stock.db).await.unwrap();
    let before = level(&stock, stock.other_location).await;
    assert!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(stock.tenant, "native-quantity-release", true),
                stock.variant,
                1
            )
            .await
            .is_err()
    );
    assert_eq!(allocation(&stock, unverified.id).await, unverified);
    assert_eq!(level(&stock, stock.other_location).await, before);
    entities::reservation_item::Entity::delete_by_id(unverified.id)
        .exec(&stock.db)
        .await
        .unwrap();
    assert!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(stock.tenant, "native-quantity-release", true),
                stock.variant,
                1
            )
            .await
            .is_err()
    );
    assert_eq!(level(&stock, stock.other_location).await, before);
}

#[tokio::test]
async fn insufficient_shared_counter_does_not_spend_other_live_ledger_claims() {
    let stock = stock().await;
    let saved = keyed(&stock).await;
    let anonymous = anonymous(&stock, stock.default_location, 3, 1).await;
    let existing = level(&stock, stock.default_location).await;
    let mut active: entities::inventory_level::ActiveModel = existing.into();
    active.reserved_quantity = Set(5); // Seven live claims; the requested two still fit numerically.
    let inconsistent = active.update(&stock.db).await.unwrap();
    assert!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(stock.tenant, "native-quantity-release", true),
                stock.variant,
                2
            )
            .await
            .is_err()
    );
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(allocation(&stock, anonymous.id).await, anonymous);
    assert_eq!(level(&stock, stock.default_location).await, inconsistent);
}

#[tokio::test]
async fn continue_backorders_keep_negative_availability_through_anonymous_partial_release() {
    let stock = stock().await;
    let variant = entities::product_variant::Entity::find_by_id(stock.variant)
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: entities::product_variant::ActiveModel = variant.into();
    active.inventory_policy = Set("CoNtInUe".to_string());
    active.update(&stock.db).await.unwrap();
    for location in [stock.default_location, stock.other_location] {
        let mut active: entities::inventory_level::ActiveModel =
            level(&stock, location).await.into();
        active.stocked_quantity = Set(0);
        active.update(&stock.db).await.unwrap();
    }
    let reserved = stock
        .service
        .reserve(
            &inventory_port_context(stock.tenant, "native-quantity-reserve", true),
            stock.variant,
            5,
        )
        .await
        .unwrap();
    assert_eq!(reserved.available_quantity, -5);
    assert!(reserved.in_stock);
    let released = stock
        .service
        .release_reservation_quantity(
            &inventory_port_context(stock.tenant, "native-quantity-release", true),
            stock.variant,
            2,
        )
        .await
        .unwrap();
    assert_eq!(released.available_quantity, -3);
    assert_eq!(released.released_quantity, 2);
    assert!(released.in_stock);
    assert_eq!(
        level(&stock, stock.default_location)
            .await
            .reserved_quantity,
        3
    );
}

#[tokio::test]
async fn response_overflow_rolls_back_anonymous_reserve_and_release_effects() {
    let stock = stock().await;
    let saved = anonymous(&stock, stock.other_location, 1, 1).await;
    for location in [stock.default_location, stock.other_location] {
        let mut active: entities::inventory_level::ActiveModel =
            level(&stock, location).await.into();
        active.stocked_quantity = Set(i32::MAX);
        active.update(&stock.db).await.unwrap();
    }
    let default = level(&stock, stock.default_location).await;
    let other = level(&stock, stock.other_location).await;
    assert!(
        stock
            .service
            .reserve(
                &inventory_port_context(stock.tenant, "native-quantity-reserve", true),
                stock.variant,
                1
            )
            .await
            .is_err()
    );
    assert!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(stock.tenant, "native-quantity-release", true),
                stock.variant,
                1
            )
            .await
            .is_err()
    );
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(level(&stock, stock.default_location).await, default);
    assert_eq!(level(&stock, stock.other_location).await, other);
    assert_eq!(
        entities::reservation_item::Entity::find()
            .filter(entities::reservation_item::Column::InventoryItemId.eq(stock.item))
            .count(&stock.db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn suppressed_exact_level_update_cannot_acknowledge_or_retain_a_partial_release() {
    let stock = stock().await;
    assert_eq!(
        stock.db.get_database_backend(),
        DbBackend::Sqlite,
        "this fault definition targets the canonical SQLite fixture"
    );
    let saved = anonymous(&stock, stock.other_location, 3, 1).await;
    let before = level(&stock, stock.other_location).await;
    stock.db.execute_raw(Statement::from_string(DbBackend::Sqlite,
        "CREATE TRIGGER suppress_quantity_release BEFORE UPDATE OF reserved_quantity ON inventory_levels WHEN NEW.reserved_quantity < OLD.reserved_quantity BEGIN SELECT RAISE(IGNORE); END"
    )).await.unwrap();
    assert!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(stock.tenant, "native-quantity-release", true),
                stock.variant,
                2
            )
            .await
            .is_err()
    );
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(level(&stock, stock.other_location).await, before);
}

#[tokio::test]
async fn quantity_selectors_refuse_before_owner_lookup_through_both_public_entrypaths() {
    let (db, service, _) = setup().await;
    let tenant = Uuid::new_v4();
    let variant = Uuid::new_v4(); // Intentionally absent: selector refusal must precede DB lookup.
    let wrapper = in_process_inventory_reservation_port(db.clone(), mock_transactional_event_bus());
    for port in [&service as &dyn InventoryReservationPort, wrapper.as_ref()] {
        for quantity in [0, 2] {
            for (parent, line) in [(Some(Uuid::new_v4()), None), (None, Some(Uuid::new_v4()))] {
                let error = port
                    .reserve_inventory(
                        inventory_port_context(tenant, "anonymous-scope-reserve", true),
                        InventoryReservationRequest {
                            variant_id: variant,
                            quantity,
                            cart_id: parent,
                            line_item_id: line,
                        },
                    )
                    .await
                    .expect_err("scoped reservation must use durable identity");
                assert_eq!(error.code, "inventory.reservation_identity_required");
                assert!(!error.retryable);
                let error = port
                    .release_inventory_reservation(
                        inventory_port_context(tenant, "anonymous-scope-release", true),
                        InventoryReservationReleaseRequest {
                            variant_id: variant,
                            quantity,
                            order_id: parent,
                            line_item_id: line,
                        },
                    )
                    .await
                    .expect_err("scoped release must use durable identity");
                assert_eq!(error.code, "inventory.reservation_identity_required");
                assert!(!error.retryable);
            }
        }
    }
    assert_eq!(
        entities::reservation_item::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn foreign_tenant_refuses_before_item_mutation_and_zero_without_item_creates_no_state() {
    let stock = stock().await;
    let saved = anonymous(&stock, stock.other_location, 3, 1).await;
    let before = level(&stock, stock.other_location).await;
    assert!(matches!(
        stock
            .service
            .release_reservation_quantity(
                &inventory_port_context(Uuid::new_v4(), "native-quantity-release", true),
                stock.variant,
                1
            )
            .await,
        Err(CommerceError::VariantNotFound(_))
    ));
    assert_eq!(allocation(&stock, saved.id).await, saved);
    assert_eq!(level(&stock, stock.other_location).await, before);
    let (db, service, catalog) = setup().await;
    let tenant = Uuid::new_v4();
    let (_, variant) = create_test_product(&catalog, tenant).await;
    let item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    entities::inventory_level::Entity::delete_many()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(item.id))
        .exec(&db)
        .await
        .unwrap();
    entities::inventory_item::Entity::delete_by_id(item.id)
        .exec(&db)
        .await
        .unwrap();
    let location_count = entities::stock_location::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(
        service
            .reserve(
                &inventory_port_context(tenant, "native-quantity-reserve", true),
                variant,
                0
            )
            .await
            .unwrap()
            .reserved_quantity,
        0
    );
    assert_eq!(
        service
            .release_reservation_quantity(
                &inventory_port_context(tenant, "native-quantity-release", true),
                variant,
                0
            )
            .await
            .unwrap()
            .released_quantity,
        0
    );
    assert_eq!(
        entities::inventory_item::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::inventory_level::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::reservation_item::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        entities::stock_location::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        location_count
    );
}

#[tokio::test]
async fn suppressed_anonymous_row_insert_rolls_back_reserved_counter_without_fake_success() {
    let stock = stock().await;
    assert_eq!(
        stock.db.get_database_backend(),
        DbBackend::Sqlite,
        "this fault definition targets the canonical SQLite fixture"
    );
    let before = level(&stock, stock.default_location).await;
    stock.db.execute_raw(Statement::from_string(DbBackend::Sqlite,
        "CREATE TRIGGER suppress_anonymous_reservation BEFORE INSERT ON reservation_items WHEN NEW.external_id IS NULL AND NEW.line_item_id IS NULL BEGIN SELECT RAISE(IGNORE); END"
    )).await.unwrap();
    assert!(
        stock
            .service
            .reserve(
                &inventory_port_context(stock.tenant, "native-quantity-reserve", true),
                stock.variant,
                2
            )
            .await
            .is_err()
    );
    assert_eq!(level(&stock, stock.default_location).await, before);
    assert_eq!(
        entities::reservation_item::Entity::find()
            .filter(entities::reservation_item::Column::InventoryItemId.eq(stock.item))
            .count(&stock.db)
            .await
            .unwrap(),
        0
    );
}
