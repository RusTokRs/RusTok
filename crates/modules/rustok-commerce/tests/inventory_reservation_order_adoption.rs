//! Persistence regression sources for the registered inventory_service_test target.
//! These are deliberately not executed by the agent; the maintainer runs this target.

use super::*;
use rustok_inventory::{
    InventoryIdentityReservationAdoptionItem, InventoryIdentityReservationAdoptionRequest,
    InventoryReservationIdentityPort,
};
use sea_orm::{ActiveModelTrait, QueryOrder, Set, TransactionTrait};
use std::sync::Arc;

struct AdoptionStock {
    db: DatabaseConnection,
    port: Arc<dyn InventoryReservationIdentityPort>,
    tenant_id: Uuid,
    request: InventoryIdentityReservationAdoptionRequest,
}

impl AdoptionStock {
    fn context(&self) -> PortContext {
        inventory_port_context(self.tenant_id, "immutable-checkout-adoption", true)
    }

    async fn rows(&self) -> Vec<entities::reservation_item::Model> {
        entities::reservation_item::Entity::find()
            .filter(
                entities::reservation_item::Column::Id
                    .is_in(self.request.items.iter().map(|item| item.reservation_id)),
            )
            .order_by_asc(entities::reservation_item::Column::Id)
            .all(&self.db)
            .await
            .unwrap()
    }

    async fn levels(&self) -> Vec<entities::inventory_level::Model> {
        let item = entities::inventory_item::Entity::find()
            .filter(
                entities::inventory_item::Column::VariantId.eq(self.request.items[0].variant_id),
            )
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        entities::inventory_level::Entity::find()
            .filter(entities::inventory_level::Column::InventoryItemId.eq(item.id))
            .order_by_asc(entities::inventory_level::Column::Id)
            .all(&self.db)
            .await
            .unwrap()
    }
}

async fn adoption_stock(count: usize) -> AdoptionStock {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_, variant_id) = create_test_product(&catalog, tenant_id).await;
    service
        .set_inventory(tenant_id, Uuid::new_v4(), variant_id, 50)
        .await
        .unwrap();
    let port = in_process_inventory_reservation_identity_port(db.clone());
    let operation_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let mut items = Vec::new();
    for index in 0..count {
        let reservation_id = Uuid::new_v4();
        let cart_line_item_id = Uuid::new_v4();
        let external_id = format!("checkout:{operation_id}:line:{cart_line_item_id}");
        let quantity = i32::try_from(index).unwrap() + 3;
        let saved = port
            .reserve_inventory_by_identity(
                inventory_port_context(tenant_id, &external_id, true),
                InventoryIdentityReservationRequest {
                    reservation_id,
                    external_id: external_id.clone(),
                    variant_id,
                    quantity,
                    line_item_id: Some(cart_line_item_id),
                    metadata: serde_json::json!({
                        "source": "checkout_operation",
                        "checkout_operation_id": operation_id,
                        "cart_id": cart_id,
                        "cart_line_item_id": cart_line_item_id,
                        "snapshot_hash": "frozen-checkout-snapshot",
                    }),
                },
            )
            .await
            .unwrap();
        items.push(InventoryIdentityReservationAdoptionItem {
            reservation_id,
            external_id,
            variant_id,
            quantity,
            location_id: saved.location_id,
            cart_line_item_id,
            order_line_item_id: Uuid::new_v4(),
        });
    }
    AdoptionStock {
        db,
        port,
        tenant_id,
        request: InventoryIdentityReservationAdoptionRequest {
            checkout_operation_id: operation_id,
            cart_id,
            order_id: Uuid::new_v4(),
            items,
        },
    }
}

#[tokio::test]
async fn owner_adoption_and_exact_replay_preserve_allocations_quantities_and_timestamps() {
    let stock = adoption_stock(2).await;
    let before_levels = stock.levels().await;
    let before_rows = stock.rows().await;
    let first = stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    assert_eq!(
        first.checkout_operation_id,
        stock.request.checkout_operation_id
    );
    assert_eq!(first.cart_id, stock.request.cart_id);
    assert_eq!(first.order_id, stock.request.order_id);
    assert_eq!(first.items, stock.request.items);
    let adopted = stock.rows().await;
    assert_eq!(adopted.len(), before_rows.len());
    for (before, after) in before_rows.iter().zip(&adopted) {
        let binding = stock
            .request
            .items
            .iter()
            .find(|item| item.reservation_id == after.id)
            .unwrap();
        assert_eq!(after.line_item_id, Some(binding.order_line_item_id));
        assert_eq!(after.id, before.id);
        assert_eq!(after.inventory_item_id, before.inventory_item_id);
        assert_eq!(after.location_id, before.location_id);
        assert_eq!(after.external_id, before.external_id);
        assert_eq!(after.quantity, before.quantity);
        assert_eq!(after.created_at, before.created_at);
        assert_eq!(after.deleted_at, before.deleted_at);
        for key in [
            "source",
            "checkout_operation_id",
            "cart_id",
            "cart_line_item_id",
            "snapshot_hash",
            "allocated_channel_slug",
        ] {
            assert_eq!(after.metadata.get(key), before.metadata.get(key));
        }
        assert_eq!(
            after.metadata["order_id"],
            serde_json::json!(stock.request.order_id)
        );
        assert_eq!(
            after.metadata["order_line_item_id"],
            serde_json::json!(binding.order_line_item_id)
        );
    }
    let replay = stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    assert_eq!(replay, first);
    assert_eq!(stock.rows().await, adopted);
    assert_eq!(stock.levels().await, before_levels);
}

#[tokio::test]
async fn invalid_binding_or_foreign_tenant_never_partially_adopts_the_batch() {
    let stock = adoption_stock(2).await;
    let before_rows = stock.rows().await;
    let before_levels = stock.levels().await;
    for field in 0..7 {
        let mut bad = stock.request.clone();
        match field {
            0 => bad.items[1].external_id.push_str(":different"),
            1 => bad.items[1].quantity += 1,
            2 => bad.items[1].location_id = Uuid::new_v4(),
            3 => bad.items[1].variant_id = Uuid::new_v4(),
            4 => bad.items[1].cart_line_item_id = Uuid::new_v4(),
            5 => bad.checkout_operation_id = Uuid::new_v4(),
            6 => bad.cart_id = Uuid::new_v4(),
            _ => unreachable!(),
        }
        assert!(
            stock
                .port
                .adopt_inventory_by_identity(stock.context(), bad)
                .await
                .is_err()
        );
        assert_eq!(stock.rows().await, before_rows);
        assert_eq!(stock.levels().await, before_levels);
    }
    let foreign = inventory_port_context(Uuid::new_v4(), "foreign-adoption", true);
    let error = stock
        .port
        .adopt_inventory_by_identity(foreign, stock.request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "inventory.variant_not_found");
    assert_eq!(stock.rows().await, before_rows);
    assert_eq!(stock.levels().await, before_levels);
}

#[tokio::test]
async fn owner_adoption_retains_hidden_archived_allocation_without_channel_reselection() {
    let stock = adoption_stock(1).await;
    let location = entities::stock_location::Entity::find_by_id(stock.request.items[0].location_id)
        .one(&stock.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: entities::stock_location::ActiveModel = location.into();
    active.metadata =
        Set(serde_json::json!({"channel_visibility": {"allowed_channel_slugs": ["mobile"]}}));
    active.deleted_at = Set(Some(chrono::Utc::now().into()));
    active.update(&stock.db).await.unwrap();
    let levels = stock.levels().await;
    let mut context = stock.context();
    context.channel = Some("web".to_string());
    let first = stock
        .port
        .adopt_inventory_by_identity(context.clone(), stock.request.clone())
        .await
        .unwrap();
    assert_eq!(
        first.items[0].location_id,
        stock.request.items[0].location_id
    );
    let replay = stock
        .port
        .adopt_inventory_by_identity(context, stock.request.clone())
        .await
        .unwrap();
    assert_eq!(replay, first);
    assert_eq!(stock.levels().await, levels);
}

#[tokio::test]
async fn adopted_identity_cannot_be_rebound_to_another_order_or_line() {
    let stock = adoption_stock(2).await;
    stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    let adopted = stock.rows().await;
    let levels = stock.levels().await;
    for field in 0..2 {
        let mut bad = stock.request.clone();
        if field == 0 {
            bad.order_id = Uuid::new_v4();
        } else {
            bad.items[1].order_line_item_id = Uuid::new_v4();
        }
        let error = stock
            .port
            .adopt_inventory_by_identity(stock.context(), bad)
            .await
            .unwrap_err();
        assert_eq!(error.code, "inventory.adoption_identity_conflict");
        assert_eq!(stock.rows().await, adopted);
        assert_eq!(stock.levels().await, levels);
    }
}

#[tokio::test]
async fn released_binding_refuses_the_entire_adoption_without_reallocating() {
    let stock = adoption_stock(2).await;
    let released = &stock.request.items[1];
    stock
        .port
        .release_inventory_by_identity(
            stock.context(),
            InventoryIdentityReservationReleaseRequest {
                reservation_id: released.reservation_id,
                external_id: released.external_id.clone(),
            },
        )
        .await
        .unwrap();
    let before_rows = stock.rows().await;
    let before_levels = stock.levels().await;
    let error = stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "inventory.adoption_identity_conflict");
    assert_eq!(stock.rows().await, before_rows);
    assert_eq!(stock.levels().await, before_levels);
}

#[tokio::test]
async fn partially_shipped_reservation_is_not_restored_by_an_adoption_replay() {
    let stock = adoption_stock(2).await;
    stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    let txn = stock.db.begin().await.unwrap();
    rustok_inventory::shipment_stock::consume_shipped_reservation(
        &txn,
        &stock
            .context()
            .with_idempotency_key("native-shipment-before-adoption-replay"),
        &rustok_inventory::shipment_stock::ShipmentStockBatch {
            order_id: stock.request.order_id,
            fulfillment_id: Uuid::new_v4(),
            lines: vec![rustok_inventory::shipment_stock::ShipmentStockLine {
                fulfillment_item_id: Uuid::new_v4(),
                order_line_item_id: stock.request.items[0].order_line_item_id,
                variant_id: Some(stock.request.items[0].variant_id),
                fulfillment_quantity: stock.request.items[0].quantity,
                shipped_quantity: 0,
                delivered_quantity: 0,
                quantity: 1,
            }],
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    let before_rows = stock.rows().await;
    let before_levels = stock.levels().await;
    let error = stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "inventory.adoption_identity_conflict");
    assert_eq!(stock.rows().await, before_rows);
    assert_eq!(stock.levels().await, before_levels);
}

#[tokio::test]
async fn original_release_identity_still_releases_only_its_allocation_after_adoption() {
    let stock = adoption_stock(2).await;
    stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    let first = &stock.request.items[0];
    let released = stock
        .port
        .release_inventory_by_identity(
            stock.context(),
            InventoryIdentityReservationReleaseRequest {
                reservation_id: first.reservation_id,
                external_id: first.external_id.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(released.released_quantity, first.quantity);
    let second = &stock.request.items[1];
    let surviving = stock
        .rows()
        .await
        .into_iter()
        .find(|row| row.id == second.reservation_id)
        .unwrap();
    assert_eq!(surviving.quantity, second.quantity);
    assert_eq!(surviving.line_item_id, Some(second.order_line_item_id));
    assert_eq!(
        stock
            .levels()
            .await
            .iter()
            .map(|level| level.reserved_quantity)
            .sum::<i32>(),
        second.quantity
    );
}

#[tokio::test]
async fn inconsistent_saved_ledger_refuses_adoption_without_repair_or_stock_movement() {
    let stock = adoption_stock(2).await;
    let level = stock.levels().await.remove(0);
    let mut active: entities::inventory_level::ActiveModel = level.into();
    active.reserved_quantity = Set(0);
    active.update(&stock.db).await.unwrap();
    let before_rows = stock.rows().await;
    let before_levels = stock.levels().await;
    let error = stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "inventory.adoption_ledger_inconsistent");
    assert_eq!(stock.rows().await, before_rows);
    assert_eq!(stock.levels().await, before_levels);
}

#[tokio::test]
async fn metadata_capacity_refuses_the_whole_batch_without_truncation_or_retry_advice() {
    for filler in ["x", "\u{1f642}"] {
        let stock = adoption_stock(2).await;
        let mut row = stock.rows().await.pop().unwrap();
        row.metadata["retained_payload"] = serde_json::json!("");
        let empty_bytes = serde_json::to_vec(&row.metadata).unwrap().len();
        // The saved metadata fits; adding both target binding UUIDs will not. UTF-8 must be
        // measured in bytes, not characters. Leave less space than one target UUID field.
        let payload = filler.repeat((32 * 1024 - empty_bytes - 32) / filler.len());
        row.metadata["retained_payload"] = serde_json::json!(payload);
        assert!(serde_json::to_vec(&row.metadata).unwrap().len() < 32 * 1024);
        let mut active: entities::reservation_item::ActiveModel = row.clone().into();
        active.metadata = Set(row.metadata);
        active.update(&stock.db).await.unwrap();
        let before_rows = stock.rows().await;
        let before_levels = stock.levels().await;
        let error = stock
            .port
            .adopt_inventory_by_identity(stock.context(), stock.request.clone())
            .await
            .unwrap_err();
        assert_eq!(error.code, "inventory.adoption_metadata_too_large");
        assert_eq!(error.kind, rustok_api::PortErrorKind::Conflict);
        assert!(!error.retryable);
        assert_eq!(stock.rows().await, before_rows);
        assert_eq!(stock.levels().await, before_levels);
    }
}

#[tokio::test]
async fn metadata_exact_sqlite_byte_boundary_adopts_and_replays_without_rewriting_payload() {
    let stock = adoption_stock(1).await;
    let mut row = stock.rows().await.pop().unwrap();
    row.metadata["retained_payload"] = serde_json::json!("");
    let mut target = row.metadata.clone();
    target["order_id"] = serde_json::json!(stock.request.order_id);
    target["order_line_item_id"] = serde_json::json!(stock.request.items[0].order_line_item_id);
    let payload = "x".repeat(32 * 1024 - serde_json::to_vec(&target).unwrap().len());
    row.metadata["retained_payload"] = serde_json::json!(payload);
    let mut active: entities::reservation_item::ActiveModel = row.clone().into();
    active.metadata = Set(row.metadata);
    active.update(&stock.db).await.unwrap();
    stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    let adopted = stock.rows().await;
    assert_eq!(
        serde_json::to_vec(&adopted[0].metadata).unwrap().len(),
        32 * 1024
    );
    stock
        .port
        .adopt_inventory_by_identity(stock.context(), stock.request.clone())
        .await
        .unwrap();
    assert_eq!(stock.rows().await, adopted);
}
