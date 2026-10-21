//! Composition of the fulfillment shipment-stock port with the inventory owner.
//!
//! `rustok-fulfillment` declares [`ShipmentStockPort`] but depends on neither order nor
//! inventory. This umbrella crate depends on all three, so the adapter lives here: it decides
//! whether an order line is stock-tracked (it has a variant) and delegates the movement to
//! `rustok_inventory::shipment_stock` on the fulfillment transaction. It replaces the former
//! `fulfillment_inventory_consume_on_ship` database trigger (`m20260713_000007`).

use std::sync::Arc;

use async_trait::async_trait;
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{
    FulfillmentAdminCommandRuntime, FulfillmentService, ShipmentStockError, ShipmentStockPort,
    ShipmentStockRequest,
};
use rustok_inventory::shipment_stock::{
    ShipmentStockMovementError, consume_shipped_reservation, reconsume_restored_stock,
    restore_unshipped_stock,
};
use rustok_order::entities::order_line_item;
use sea_orm::{DatabaseConnection, DatabaseTransaction, EntityTrait};

/// Inventory-backed [`ShipmentStockPort`].
#[derive(Debug, Default, Clone, Copy)]
pub struct InventoryShipmentStockPort;

impl InventoryShipmentStockPort {
    /// Whether the line is tracked in inventory. A line without a variant (digital, custom)
    /// never has a reservation, so shipping it moves no stock.
    async fn is_stock_tracked(
        txn: &DatabaseTransaction,
        request: &ShipmentStockRequest,
    ) -> Result<bool, ShipmentStockError> {
        let line = order_line_item::Entity::find_by_id(request.order_line_item_id)
            .one(txn)
            .await?
            .ok_or_else(|| {
                ShipmentStockError::Rejected(format!(
                    "fulfillment item references missing order line {}",
                    request.order_line_item_id
                ))
            })?;
        Ok(line.variant_id.is_some())
    }
}

fn map_movement_error(error: ShipmentStockMovementError) -> ShipmentStockError {
    match error {
        ShipmentStockMovementError::Rejected(message) => ShipmentStockError::Rejected(message),
        ShipmentStockMovementError::Database(error) => ShipmentStockError::Database(error),
    }
}

#[async_trait]
impl ShipmentStockPort for InventoryShipmentStockPort {
    async fn consume_shipped(
        &self,
        txn: &DatabaseTransaction,
        request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        if !Self::is_stock_tracked(txn, &request).await? {
            return Ok(());
        }
        consume_shipped_reservation(txn, request.order_line_item_id, request.quantity)
            .await
            .map_err(map_movement_error)
    }

    async fn restore_unshipped(
        &self,
        txn: &DatabaseTransaction,
        request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        if !Self::is_stock_tracked(txn, &request).await? {
            return Ok(());
        }
        restore_unshipped_stock(txn, request.order_line_item_id, request.quantity)
            .await
            .map_err(map_movement_error)
    }

    async fn reconsume_restored(
        &self,
        txn: &DatabaseTransaction,
        request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        if !Self::is_stock_tracked(txn, &request).await? {
            return Ok(());
        }
        reconsume_restored_stock(txn, request.order_line_item_id, request.quantity)
            .await
            .map_err(map_movement_error)
    }
}

/// The production stock port: shipment moves inventory.
pub fn inventory_shipment_stock_port() -> Arc<dyn ShipmentStockPort> {
    Arc::new(InventoryShipmentStockPort)
}

/// A [`FulfillmentService`] that moves stock on ship / cancel-shipped / reopen-cancelled.
///
/// Every commerce path that may ship, cancel or reopen a fulfillment must build its service
/// here; a bare `FulfillmentService::new` fails those operations on purpose.
pub fn fulfillment_service(db: DatabaseConnection) -> FulfillmentService {
    FulfillmentService::new(db).with_shipment_stock_port(inventory_shipment_stock_port())
}

/// In-process admin command runtime wired to the inventory-backed stock port.
pub fn fulfillment_admin_command_runtime(
    db: DatabaseConnection,
    provider_registry: FulfillmentProviderRegistry,
) -> FulfillmentAdminCommandRuntime {
    FulfillmentAdminCommandRuntime::in_process_with_shipment_stock(
        db,
        provider_registry,
        inventory_shipment_stock_port(),
    )
}
