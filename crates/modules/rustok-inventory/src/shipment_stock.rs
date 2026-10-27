//! Stock movements driven by physical shipment, executed inside the caller's transaction.
//!
//! Inventory owns the reservation ledger (`reservation_items`) and the stock levels
//! (`inventory_levels`). The fulfillment owner calls these functions (through the
//! `ShipmentStockPort` adapter composed in `rustok-commerce`) from its own shipment
//! transaction, so the shipped counters and the stock ledger commit or roll back together.
//! They replace the former `fulfillment_inventory_consume_on_ship` database trigger.
//!
//! All functions are keyed by the order line the reservation was taken for. Callers decide
//! whether a line is stock-tracked at all (a line without a variant never has a reservation).

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{inventory_level, reservation_item};

#[derive(Debug, Error)]
pub enum ShipmentStockMovementError {
    /// The ledger refuses the movement; the surrounding transaction must roll back.
    #[error("{0}")]
    Rejected(String),
    #[error(transparent)]
    Database(#[from] DbErr),
}

fn rejected(message: String) -> ShipmentStockMovementError {
    ShipmentStockMovementError::Rejected(message)
}

fn require_positive(quantity: i32) -> Result<(), ShipmentStockMovementError> {
    if quantity <= 0 {
        return Err(rejected(
            "shipment stock quantity must be positive".to_string(),
        ));
    }
    Ok(())
}

/// Consumes `quantity` shipped units from the order line's active reservation: the level loses
/// both stocked and reserved units and the reservation shrinks (soft-deleted at zero).
pub async fn consume_shipped_reservation(
    txn: &DatabaseTransaction,
    order_line_item_id: Uuid,
    quantity: i32,
) -> Result<(), ShipmentStockMovementError> {
    require_positive(quantity)?;
    let reservation = load_reservation_for_update(txn, order_line_item_id, true)
        .await?
        .ok_or_else(|| {
            rejected(format!(
                "active inventory reservation is missing for order line {order_line_item_id}"
            ))
        })?;
    if reservation.quantity < quantity {
        return Err(rejected(format!(
            "shipped quantity {quantity} exceeds remaining reservation {} for order line {order_line_item_id}",
            reservation.quantity
        )));
    }
    let level = load_level_for_update(txn, &reservation)
        .await?
        .filter(|level| level.reserved_quantity >= quantity && level.stocked_quantity >= quantity)
        .ok_or_else(|| {
            rejected(format!(
                "inventory level reservation is inconsistent for order line {order_line_item_id}"
            ))
        })?;

    let now = Utc::now();
    let mut level_active: inventory_level::ActiveModel = level.clone().into();
    level_active.stocked_quantity = Set(level.stocked_quantity - quantity);
    level_active.reserved_quantity = Set(level.reserved_quantity - quantity);
    level_active.updated_at = Set(now.into());
    level_active.update(txn).await?;

    let remaining = reservation.quantity - quantity;
    let mut reservation_active: reservation_item::ActiveModel = reservation.into();
    reservation_active.quantity = Set(remaining);
    reservation_active.deleted_at = Set((remaining == 0).then(|| now.into()));
    reservation_active.updated_at = Set(now.into());
    reservation_active.update(txn).await?;
    Ok(())
}

/// Puts `quantity` units of a cancelled shipment back on the shelf at the location the line's
/// reservation was taken from. The reservation itself stays consumed.
pub async fn restore_unshipped_stock(
    txn: &DatabaseTransaction,
    order_line_item_id: Uuid,
    quantity: i32,
) -> Result<(), ShipmentStockMovementError> {
    require_positive(quantity)?;
    let level = load_line_level_for_update(txn, order_line_item_id).await?;
    let stocked = level.stocked_quantity.checked_add(quantity).ok_or_else(|| {
        rejected(format!(
            "restored stock overflows the level of order line {order_line_item_id}"
        ))
    })?;
    let mut active: inventory_level::ActiveModel = level.into();
    active.stocked_quantity = Set(stocked);
    active.updated_at = Set(Utc::now().into());
    active.update(txn).await?;
    Ok(())
}

/// Takes `quantity` previously restored units off the shelf again (reopen of a cancelled
/// shipment). Fails when they are no longer available.
pub async fn reconsume_restored_stock(
    txn: &DatabaseTransaction,
    order_line_item_id: Uuid,
    quantity: i32,
) -> Result<(), ShipmentStockMovementError> {
    require_positive(quantity)?;
    let level = load_line_level_for_update(txn, order_line_item_id).await?;
    if level.stocked_quantity.saturating_sub(level.reserved_quantity) < quantity {
        return Err(rejected(format!(
            "restored stock for order line {order_line_item_id} is no longer available"
        )));
    }
    let stocked = level.stocked_quantity - quantity;
    let mut active: inventory_level::ActiveModel = level.into();
    active.stocked_quantity = Set(stocked);
    active.updated_at = Set(Utc::now().into());
    active.update(txn).await?;
    Ok(())
}

/// Level of the (inventory item, location) the order line's reservation belongs to. The
/// reservation may already be consumed (soft-deleted); it still identifies the location.
pub(crate) async fn load_line_level_for_update(
    txn: &DatabaseTransaction,
    order_line_item_id: Uuid,
) -> Result<inventory_level::Model, ShipmentStockMovementError> {
    let reservation = load_reservation_for_update(txn, order_line_item_id, false)
        .await?
        .ok_or_else(|| {
            rejected(format!(
                "inventory reservation is missing for order line {order_line_item_id}"
            ))
        })?;
    load_level_for_update(txn, &reservation)
        .await?
        .ok_or_else(|| {
            rejected(format!(
                "inventory level is missing for order line {order_line_item_id}"
            ))
        })
}

async fn load_reservation_for_update(
    txn: &DatabaseTransaction,
    order_line_item_id: Uuid,
    active_only: bool,
) -> Result<Option<reservation_item::Model>, DbErr> {
    let mut query = reservation_item::Entity::find()
        .filter(reservation_item::Column::LineItemId.eq(order_line_item_id));
    if active_only {
        query = query
            .filter(reservation_item::Column::DeletedAt.is_null())
            .filter(reservation_item::Column::Quantity.gt(0));
    }
    query = query
        .order_by_asc(reservation_item::Column::CreatedAt)
        .order_by_asc(reservation_item::Column::Id);
    if txn.get_database_backend() != DbBackend::Sqlite {
        query = query.lock_exclusive();
    }
    query.one(txn).await
}

async fn load_level_for_update(
    txn: &DatabaseTransaction,
    reservation: &reservation_item::Model,
) -> Result<Option<inventory_level::Model>, DbErr> {
    let mut query = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(reservation.inventory_item_id))
        .filter(inventory_level::Column::LocationId.eq(reservation.location_id));
    if txn.get_database_backend() != DbBackend::Sqlite {
        query = query.lock_exclusive();
    }
    query.one(txn).await
}
