//! Operator-decided restock of returned goods.
//!
//! Completing a return does not touch stock by itself: whether a returned unit is sellable
//! again is a physical-inspection decision. When the operator decides to restock, the commerce
//! return-completion orchestration calls [`restock_returned_items`], which puts the units back
//! on the shelf at the location the order line's reservation was taken from.
//!
//! Idempotency is owned here: [`ledger`] holds one row per return item (primary key), written in
//! the same transaction as the stock increase, so replays, retries after a crash and concurrent
//! completions never add stock twice.

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{inventory_level, reservation_item};

/// Inventory-owned ledger of restocked return items.
pub mod ledger {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "inventory_return_restocks")]
    pub struct Model {
        /// One restock per return item: the primary key is the idempotency key.
        #[sea_orm(primary_key, auto_increment = false)]
        pub return_item_id: Uuid,
        pub tenant_id: Uuid,
        pub return_id: Uuid,
        pub order_line_item_id: Uuid,
        pub inventory_item_id: Uuid,
        pub location_id: Uuid,
        pub quantity: i32,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// One return item the operator decided to put back on the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnRestockLine {
    pub return_item_id: Uuid,
    pub return_id: Uuid,
    pub order_line_item_id: Uuid,
    /// Strictly positive number of units to restock.
    pub quantity: i32,
}

/// What [`restock_returned_items`] did with each requested line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReturnRestockOutcome {
    /// Newly restocked in this call: `(return_item_id, quantity)`.
    pub restocked: Vec<(Uuid, i32)>,
    /// Already restocked by an earlier call; nothing changed.
    pub already_restocked: Vec<Uuid>,
    /// The order line has no inventory reservation (no variant, or never stock-tracked).
    pub untracked: Vec<Uuid>,
}

#[derive(Debug, Error)]
pub enum ReturnRestockError {
    #[error("{0}")]
    Rejected(String),
    #[error(transparent)]
    Database(#[from] DbErr),
}

/// Restocks the given return items in one transaction. Safe to call repeatedly.
pub async fn restock_returned_items(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lines: &[ReturnRestockLine],
) -> Result<ReturnRestockOutcome, ReturnRestockError> {
    for line in lines {
        if line.quantity <= 0 {
            return Err(ReturnRestockError::Rejected(format!(
                "restock quantity for return item {} must be positive",
                line.return_item_id
            )));
        }
    }
    let txn = db.begin().await?;
    let mut outcome = ReturnRestockOutcome::default();
    for line in lines {
        restock_line(&txn, tenant_id, line, &mut outcome).await?;
    }
    txn.commit().await?;
    Ok(outcome)
}

async fn restock_line(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    line: &ReturnRestockLine,
    outcome: &mut ReturnRestockOutcome,
) -> Result<(), ReturnRestockError> {
    // The reservation row survives consumption (soft-deleted), so it still names the item and
    // location the units came from.
    let Some(reservation) = reservation_item::Entity::find()
        .filter(reservation_item::Column::LineItemId.eq(line.order_line_item_id))
        .order_by_asc(reservation_item::Column::CreatedAt)
        .order_by_asc(reservation_item::Column::Id)
        .one(txn)
        .await?
    else {
        outcome.untracked.push(line.return_item_id);
        return Ok(());
    };

    // Serialize on the level, then re-check the ledger: a concurrent completion that already
    // restocked this return item is visible once we hold the level.
    let mut level_query = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(reservation.inventory_item_id))
        .filter(inventory_level::Column::LocationId.eq(reservation.location_id));
    if txn.get_database_backend() != DbBackend::Sqlite {
        level_query = level_query.lock_exclusive();
    }
    let level = level_query.one(txn).await?.ok_or_else(|| {
        ReturnRestockError::Rejected(format!(
            "inventory level is missing for order line {}",
            line.order_line_item_id
        ))
    })?;

    if ledger::Entity::find_by_id(line.return_item_id)
        .one(txn)
        .await?
        .is_some()
    {
        outcome.already_restocked.push(line.return_item_id);
        return Ok(());
    }

    let stocked = level
        .stocked_quantity
        .checked_add(line.quantity)
        .ok_or_else(|| {
            ReturnRestockError::Rejected(format!(
                "restocked quantity overflows the level of order line {}",
                line.order_line_item_id
            ))
        })?;
    let now = Utc::now();
    let mut level_active: inventory_level::ActiveModel = level.into();
    level_active.stocked_quantity = Set(stocked);
    level_active.updated_at = Set(now.into());
    level_active.update(txn).await?;

    ledger::ActiveModel {
        return_item_id: Set(line.return_item_id),
        tenant_id: Set(tenant_id),
        return_id: Set(line.return_id),
        order_line_item_id: Set(line.order_line_item_id),
        inventory_item_id: Set(reservation.inventory_item_id),
        location_id: Set(reservation.location_id),
        quantity: Set(line.quantity),
        created_at: Set(now.into()),
    }
    .insert(txn)
    .await?;
    outcome.restocked.push((line.return_item_id, line.quantity));
    Ok(())
}
