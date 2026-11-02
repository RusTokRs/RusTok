//! Native physical movement and retained allocation provenance, in Fulfillment's transaction.
//! Preparation rolls back proof-key probes and never changes physical stock; it is not a receipt.
//! Legacy progress/zero reservations/cancelled status never manufacture movement history.

use crate::entities::{inventory_level, reservation_item};
use crate::ports_impl::{parse_port_tenant_id, storage_unavailable_with_context};
use crate::stock_events::{StockEventPlan, StockLevelChange};
use chrono::Utc;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, DbErr, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
    sea_query::OnConflict,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(crate) mod proof;
mod scope;

const OPERATION: &str = "shipment_stock";
pub const MAX_SHIPMENT_STOCK_ITEMS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipmentStockAction {
    Ship,
    Restore,
    Reconsume,
}

/// Consumer-owned locked progress and the selected Order's saved nullable SKU, not fresh eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShipmentStockLine {
    pub fulfillment_item_id: Uuid,
    pub order_line_item_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub fulfillment_quantity: i32,
    pub shipped_quantity: i32,
    pub delivered_quantity: i32,
    pub quantity: i32,
}

#[derive(Clone, Debug)]
pub struct ShipmentStockBatch {
    pub order_id: Uuid,
    pub fulfillment_id: Uuid,
    pub lines: Vec<ShipmentStockLine>,
}

/// Early physical/history refusal before carrier execution, in a short Inventory-owned
/// transaction which is rolled back. No plan/receipt/stock is persisted. All checks repeat
/// on Fulfillment's transaction, so this is not a stock-availability promise across a carrier.
pub async fn validate_shipment_stock_preparation(
    db: &DatabaseConnection,
    context: &PortContext,
    batch: &ShipmentStockBatch,
    action: ShipmentStockAction,
) -> Result<(), PortError> {
    let started = std::time::Instant::now();
    let bounded = admitted_context(context)?;
    scope::validate_batch(batch)?;
    let txn = db.begin().await.map_err(|error| storage(&bounded, error))?;
    let result = apply(&txn, &bounded, batch, action, Application::Prepare, started).await;
    let rollback = txn.rollback().await;
    if let Err(cause) = result {
        if rollback.is_err() {
            tracing::error!(
                operation = OPERATION,
                preparation_rollback_failed = true,
                "shipment preparation rollback needs owner attention"
            );
        }
        return Err(cause);
    }
    rollback.map_err(|error| storage(&bounded, error))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Application {
    Prepare,
    Move,
}

fn admitted_context(context: &PortContext) -> Result<PortContext, PortError> {
    context.require_policy(PortCallPolicy::write())?;
    let mut bounded = context.clone();
    bounded.deadline_ms = Some(
        context
            .deadline_ms
            .filter(|deadline| *deadline > 0)
            .ok_or_else(|| {
                PortError::timeout(
                    "inventory.shipment_stock_deadline_required",
                    "shipment stock requires a positive deadline",
                )
            })?
            .min(30_000),
    );
    Ok(bounded)
}

pub async fn consume_shipped_reservation(
    txn: &DatabaseTransaction,
    context: &PortContext,
    batch: &ShipmentStockBatch,
) -> Result<(), PortError> {
    let started = std::time::Instant::now();
    let bounded = admitted_context(context)?;
    apply(
        txn,
        &bounded,
        batch,
        ShipmentStockAction::Ship,
        Application::Move,
        started,
    )
    .await
}
pub async fn restore_unshipped_stock(
    txn: &DatabaseTransaction,
    context: &PortContext,
    batch: &ShipmentStockBatch,
) -> Result<(), PortError> {
    let started = std::time::Instant::now();
    let bounded = admitted_context(context)?;
    apply(
        txn,
        &bounded,
        batch,
        ShipmentStockAction::Restore,
        Application::Move,
        started,
    )
    .await
}
pub async fn reconsume_restored_stock(
    txn: &DatabaseTransaction,
    context: &PortContext,
    batch: &ShipmentStockBatch,
) -> Result<(), PortError> {
    let started = std::time::Instant::now();
    let bounded = admitted_context(context)?;
    apply(
        txn,
        &bounded,
        batch,
        ShipmentStockAction::Reconsume,
        Application::Move,
        started,
    )
    .await
}

/// Every error requires the consumer to drop/roll back its transaction; never commit a partial batch.
async fn apply(
    txn: &DatabaseTransaction,
    context: &PortContext,
    batch: &ShipmentStockBatch,
    action: ShipmentStockAction,
    application: Application,
    started: std::time::Instant,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write())?;
    scope::validate_batch(batch)?;
    require_remaining_budget(context, started)?;
    let tenant_id = parse_port_tenant_id(context, OPERATION)?;
    // Nullable SKU lines have no item row to lock. Serialize the line before reading its
    // immutable binding, then use the same item locks as return restock for physical overlap.
    scope::lock_lines(txn, context, tenant_id, batch).await?;
    require_remaining_budget(context, started)?;
    // Lock all proved SKU items before tenant-less allocations and before provenance/level work.
    let scope::LockedItems { items, products } =
        scope::lock_items(txn, context, tenant_id, batch).await?;
    require_remaining_budget(context, started)?;
    let saved = scope::lock_proofs(txn, context, tenant_id, batch).await?;
    require_remaining_budget(context, started)?;
    let mut progress = BTreeMap::new();
    for line in &batch.lines {
        require_remaining_budget(context, started)?;
        if line.variant_id.is_none() {
            scope::read_line_binding(
                txn,
                context,
                tenant_id,
                batch.order_id,
                line.order_line_item_id,
                None,
            )
            .await?;
            scope::assert_sku_less_has_no_allocation(txn, context, line.order_line_item_id).await?;
            assert_no_returned_stock(txn, context, tenant_id, line.order_line_item_id).await?;
        }
        progress.insert(
            line.fulfillment_item_id,
            scope::next_progress(
                tenant_id,
                batch,
                line,
                saved.get(&line.fulfillment_item_id),
                action,
            )?,
        );
        if line.variant_id.is_some() && action != ShipmentStockAction::Ship {
            assert_no_returned_stock(txn, context, tenant_id, line.order_line_item_id).await?;
        }
    }
    let allocations =
        scope::lock_allocations(txn, context, batch, tenant_id, &items, &saved).await?;
    require_remaining_budget(context, started)?;
    let levels = scope::lock_levels(txn, context, tenant_id, &allocations).await?;
    require_remaining_budget(context, started)?;
    let mut level_deltas = BTreeMap::<(Uuid, Uuid), (i32, i32)>::new();
    let mut consumed = BTreeMap::<Uuid, i32>::new();
    for line in &batch.lines {
        if line.variant_id.is_none() {
            continue;
        }
        let allocation = allocations
            .get(&line.order_line_item_id)
            .ok_or_else(history_unverified)?;
        let delta = level_deltas
            .entry((allocation.inventory_item_id, allocation.location_id))
            .or_default();
        match action {
            ShipmentStockAction::Ship => {
                if allocation.deleted_at.is_some() || allocation.quantity <= 0 {
                    return Err(history_unverified());
                }
                let total = consumed.entry(allocation.id).or_default();
                *total = total.checked_add(line.quantity).ok_or_else(overflow)?;
                delta.0 = delta.0.checked_sub(line.quantity).ok_or_else(overflow)?;
                delta.1 = delta.1.checked_sub(line.quantity).ok_or_else(overflow)?;
            }
            ShipmentStockAction::Restore => {
                delta.0 = delta.0.checked_add(line.quantity).ok_or_else(overflow)?
            }
            ShipmentStockAction::Reconsume => {
                delta.0 = delta.0.checked_sub(line.quantity).ok_or_else(overflow)?
            }
        }
    }
    // Preflight every grouped addition/subtraction before any proof, reservation or stock write.
    let mut level_values = BTreeMap::new();
    for (key, (stock_delta, reserve_delta)) in level_deltas {
        let level = levels.get(&key).ok_or_else(history_unverified)?;
        let stocked = level
            .stocked_quantity
            .checked_add(stock_delta)
            .ok_or_else(overflow)?;
        let reserved = level
            .reserved_quantity
            .checked_add(reserve_delta)
            .ok_or_else(overflow)?;
        if stocked < 0 || reserved < 0 || reserved > stocked {
            return Err(refused(
                "unavailable_units",
                "saved shipment units are not available without consuming another reservation",
            ));
        }
        level_values.insert(key, (stocked, reserved));
    }
    let mut remaining = BTreeMap::new();
    for allocation in allocations.values() {
        if let Some(quantity) = consumed.get(&allocation.id) {
            let quantity = allocation
                .quantity
                .checked_sub(*quantity)
                .ok_or_else(overflow)?;
            if quantity < 0 {
                return Err(refused(
                    "reservation_insufficient",
                    "shipment exceeds its saved remaining reservation",
                ));
            }
            remaining.insert(allocation.id, quantity);
        }
    }
    // Retain the tenant-proved Product identity from item admission; never load Catalog
    // eligibility or ask another owner under Fulfillment's write transaction.
    let item_bindings = items
        .iter()
        .map(|(variant_id, item)| {
            Ok((
                item.id,
                (
                    *variant_id,
                    *products.get(variant_id).ok_or_else(history_unverified)?,
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, PortError>>()?;
    let changes = level_values
        .iter()
        .map(|(key, (stocked, reserved))| {
            let previous = levels.get(key).ok_or_else(history_unverified)?;
            let (variant_id, product_id) =
                *item_bindings.get(&key.0).ok_or_else(history_unverified)?;
            Ok(StockLevelChange {
                inventory_item_id: key.0,
                variant_id,
                product_id,
                location_id: key.1,
                old_stocked: previous.stocked_quantity,
                old_reserved: previous.reserved_quantity,
                new_stocked: *stocked,
                new_reserved: *reserved,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    let stock_events = StockEventPlan::prepare(txn, context, OPERATION, changes).await?;
    // Lock/validation I/O consumes the same admitted budget; positive entry metadata is
    // not permission to perform writes after a long lock wait. SQL timeouts are separate.
    require_remaining_budget(context, started)?;
    let now = Utc::now();
    let ordered_lines = batch
        .lines
        .iter()
        .map(|line| (line.fulfillment_item_id, line))
        .collect::<BTreeMap<_, _>>();
    if application == Application::Prepare {
        // Exercise the global primary-key fence without looking at a foreign tenant's row.
        // The short owner transaction ALWAYS rolls this probe back; no physical stock or
        // reservation value is updated and no durable proof/receipt is manufactured.
        for line in ordered_lines.values() {
            require_remaining_budget(context, started)?;
            if !saved.contains_key(&line.fulfillment_item_id) {
                insert_new_proof(
                    txn,
                    context,
                    new_proof(
                        tenant_id,
                        batch,
                        line,
                        progress[&line.fulfillment_item_id],
                        allocations.get(&line.order_line_item_id),
                        now,
                    ),
                )
                .await?;
            }
        }
        return require_remaining_budget(context, started);
    }
    for line in ordered_lines.into_values() {
        require_remaining_budget(context, started)?;
        let (shipped, restored) = progress[&line.fulfillment_item_id];
        if let Some(row) = saved.get(&line.fulfillment_item_id) {
            let mut active: proof::ActiveModel = row.clone().into();
            active.shipped_quantity = Set(shipped);
            active.restored_quantity = Set(restored);
            active.updated_at = Set(now.into());
            active
                .update(txn)
                .await
                .map_err(|error| storage(context, error))?;
        } else {
            insert_new_proof(
                txn,
                context,
                new_proof(
                    tenant_id,
                    batch,
                    line,
                    (shipped, restored),
                    allocations.get(&line.order_line_item_id),
                    now,
                ),
            )
            .await?;
        }
    }
    for (key, (stocked, reserved)) in level_values {
        require_remaining_budget(context, started)?;
        let mut active: inventory_level::ActiveModel = levels[&key].clone().into();
        active.stocked_quantity = Set(stocked);
        active.reserved_quantity = Set(reserved);
        active.updated_at = Set(now.into());
        active
            .update(txn)
            .await
            .map_err(|error| storage(context, error))?;
    }
    for allocation in allocations.values() {
        require_remaining_budget(context, started)?;
        if let Some(quantity) = remaining.remove(&allocation.id) {
            let mut active: reservation_item::ActiveModel = allocation.clone().into();
            active.quantity = Set(quantity);
            active.deleted_at = Set((quantity == 0).then(|| now.into()));
            active.updated_at = Set(now.into());
            active
                .update(txn)
                .await
                .map_err(|error| storage(context, error))?;
        }
    }
    require_remaining_budget(context, started)?;
    stock_events.publish(txn, context, OPERATION).await?;
    require_remaining_budget(context, started)
}

fn require_remaining_budget(
    context: &PortContext,
    started: std::time::Instant,
) -> Result<(), PortError> {
    let remaining = context
        .deadline_ms
        .filter(|budget| *budget > 0)
        .and_then(|budget| {
            std::time::Duration::from_millis(budget.min(30_000)).checked_sub(started.elapsed())
        });
    if remaining.is_none_or(|remaining| remaining.as_millis() == 0) {
        return Err(PortError::timeout(
            "inventory.shipment_stock_deadline_expired",
            "native shipment budget expired while acquiring locks or applying the atomic batch",
        ));
    }
    Ok(())
}

fn new_proof(
    tenant_id: Uuid,
    batch: &ShipmentStockBatch,
    line: &ShipmentStockLine,
    progress: (i32, i32),
    allocation: Option<&reservation_item::Model>,
    now: chrono::DateTime<Utc>,
) -> proof::ActiveModel {
    proof::ActiveModel {
        fulfillment_item_id: Set(line.fulfillment_item_id),
        tenant_id: Set(tenant_id),
        order_id: Set(batch.order_id),
        fulfillment_id: Set(batch.fulfillment_id),
        order_line_item_id: Set(line.order_line_item_id),
        variant_id: Set(line.variant_id),
        fulfillment_quantity: Set(line.fulfillment_quantity),
        inventory_item_id: Set(allocation.map(|row| row.inventory_item_id)),
        reservation_id: Set(allocation.map(|row| row.id)),
        location_id: Set(allocation.map(|row| row.location_id)),
        shipped_quantity: Set(progress.0),
        restored_quantity: Set(progress.1),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
}

async fn insert_new_proof(
    txn: &DatabaseTransaction,
    context: &PortContext,
    proof: proof::ActiveModel,
) -> Result<(), PortError> {
    let affected = proof::Entity::insert(proof)
        .on_conflict(
            OnConflict::column(proof::Column::FulfillmentItemId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(txn)
        .await
        .map_err(|error| storage(context, error))?;
    if affected != 1 {
        return Err(history_unverified());
    }
    Ok(())
}

pub(crate) async fn assert_no_restored_shipment_stock<C: ConnectionTrait>(
    db: &C,
    context: &PortContext,
    tenant_id: Uuid,
    line_id: Uuid,
) -> Result<(), PortError> {
    let restored = proof::Entity::find()
        .filter(proof::Column::TenantId.eq(tenant_id))
        .filter(proof::Column::OrderLineItemId.eq(line_id))
        .filter(proof::Column::VariantId.is_not_null())
        .filter(proof::Column::RestoredQuantity.gt(0))
        .limit(1)
        .one(db)
        .await
        .map_err(|error| storage(context, error))?;
    if restored.is_some() {
        return Err(refused(
            "return_overlap_unverified",
            "return restock overlaps retained shipment restoration; reconcile original movements",
        ));
    }
    Ok(())
}

async fn assert_no_returned_stock<C: ConnectionTrait>(
    db: &C,
    context: &PortContext,
    tenant_id: Uuid,
    line_id: Uuid,
) -> Result<(), PortError> {
    use crate::return_restock::ledger;
    let returned = ledger::Entity::find()
        .filter(ledger::Column::TenantId.eq(tenant_id))
        .filter(ledger::Column::OrderLineItemId.eq(line_id))
        .limit(1)
        .one(db)
        .await
        .map_err(|error| storage(context, error))?;
    if returned.is_some() {
        return Err(refused(
            "return_overlap_unverified",
            "shipment restoration overlaps retained returned goods; reconcile original movements",
        ));
    }
    Ok(())
}

fn history_unverified() -> PortError {
    refused(
        "history_unverified",
        "shipment allocation or native consumption/restoration history is unverified; reconcile without backfill or identity rotation",
    )
}
fn overflow() -> PortError {
    refused(
        "quantity_overflow",
        "shipment stock arithmetic exceeds the supported range",
    )
}
fn refused(suffix: &str, message: &str) -> PortError {
    PortError::invariant_violation(format!("inventory.shipment_stock_{suffix}"), message)
}
fn storage(context: &PortContext, error: DbErr) -> PortError {
    storage_unavailable_with_context(context, OPERATION, error)
}
