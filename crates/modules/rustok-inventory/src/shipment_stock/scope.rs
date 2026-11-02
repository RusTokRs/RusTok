use super::*;
use crate::entities::{inventory_item, stock_location};
use crate::ports_impl::{load_inventory_item_for_update, load_tenant_variant};
use crate::reservation_adoption::has_terminal_disposition;
use sea_orm::FromQueryResult;

pub(super) fn validate_batch(batch: &ShipmentStockBatch) -> Result<(), PortError> {
    if batch.order_id.is_nil()
        || batch.fulfillment_id.is_nil()
        || batch.lines.is_empty()
        || batch.lines.len() > MAX_SHIPMENT_STOCK_ITEMS
    {
        return Err(refused(
            "request_invalid",
            "shipment stock batch identity or bounds are invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut variants = BTreeMap::new();
    for line in &batch.lines {
        if line.fulfillment_item_id.is_nil()
            || line.order_line_item_id.is_nil()
            || line.variant_id.is_some_and(|id| id.is_nil())
            || !ids.insert(line.fulfillment_item_id)
            || line.fulfillment_quantity <= 0
            || line.quantity <= 0
            || line.shipped_quantity < 0
            || line.delivered_quantity < 0
            || line.delivered_quantity > line.shipped_quantity
            || line.shipped_quantity > line.fulfillment_quantity
        {
            return Err(refused(
                "request_invalid",
                "shipment stock identities or progress are invalid",
            ));
        }
        if let Some(previous) = variants.insert(line.order_line_item_id, line.variant_id)
            && previous != line.variant_id
        {
            return Err(history_unverified());
        }
    }
    Ok(())
}

pub(super) fn next_progress(
    tenant_id: Uuid,
    batch: &ShipmentStockBatch,
    line: &ShipmentStockLine,
    saved: Option<&proof::Model>,
    action: ShipmentStockAction,
) -> Result<(i32, i32), PortError> {
    if let Some(saved) = saved {
        if saved.tenant_id != tenant_id
            || saved.order_id != batch.order_id
            || saved.fulfillment_id != batch.fulfillment_id
            || saved.fulfillment_item_id != line.fulfillment_item_id
            || saved.order_line_item_id != line.order_line_item_id
            || saved.variant_id != line.variant_id
            || saved.fulfillment_quantity != line.fulfillment_quantity
            || saved.shipped_quantity != line.shipped_quantity
            || saved.shipped_quantity <= 0
            || saved.restored_quantity < 0
            || saved.restored_quantity > saved.shipped_quantity
        {
            return Err(history_unverified());
        }
        match saved.variant_id {
            Some(_)
                if [
                    saved.inventory_item_id,
                    saved.reservation_id,
                    saved.location_id,
                ]
                .iter()
                .any(|id| id.is_none_or(|id| id.is_nil())) =>
            {
                return Err(history_unverified());
            }
            None if saved.inventory_item_id.is_some()
                || saved.reservation_id.is_some()
                || saved.location_id.is_some() =>
            {
                return Err(history_unverified());
            }
            _ => {}
        }
    }
    match action {
        ShipmentStockAction::Ship => {
            if saved.is_none() && (line.shipped_quantity != 0 || line.delivered_quantity != 0)
                || saved.is_some_and(|saved| saved.restored_quantity != 0)
            {
                return Err(history_unverified());
            }
            let shipped = line
                .shipped_quantity
                .checked_add(line.quantity)
                .ok_or_else(overflow)?;
            if shipped > line.fulfillment_quantity {
                return Err(history_unverified());
            }
            Ok((shipped, 0))
        }
        ShipmentStockAction::Restore | ShipmentStockAction::Reconsume => {
            let saved = saved.ok_or_else(history_unverified)?;
            let undelivered = line
                .shipped_quantity
                .checked_sub(line.delivered_quantity)
                .ok_or_else(history_unverified)?;
            if undelivered != line.quantity || undelivered <= 0 {
                return Err(history_unverified());
            }
            match action {
                ShipmentStockAction::Restore if saved.restored_quantity == 0 => {
                    Ok((saved.shipped_quantity, undelivered))
                }
                ShipmentStockAction::Reconsume if saved.restored_quantity == undelivered => {
                    Ok((saved.shipped_quantity, 0))
                }
                _ => Err(history_unverified()),
            }
        }
    }
}

pub(super) struct LockedItems {
    pub(super) items: BTreeMap<Uuid, inventory_item::Model>,
    pub(super) products: BTreeMap<Uuid, Uuid>,
}

pub(super) async fn lock_items(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    batch: &ShipmentStockBatch,
) -> Result<LockedItems, PortError> {
    let variants = batch
        .lines
        .iter()
        .filter_map(|line| line.variant_id)
        .collect::<BTreeSet<_>>();
    let mut items = BTreeMap::new();
    let mut products = BTreeMap::new();
    for variant_id in &variants {
        let variant = load_tenant_variant(context, OPERATION, txn, tenant_id, *variant_id).await?;
        products.insert(*variant_id, variant.product_id);
    }
    for variant_id in variants {
        let item = load_inventory_item_for_update(context, OPERATION, txn, variant_id)
            .await?
            .filter(|item| !item.id.is_nil() && item.variant_id == variant_id)
            .ok_or_else(|| {
                refused(
                    "item_unverified",
                    "saved shipment SKU has no provable inventory item",
                )
            })?;
        items.insert(variant_id, item);
    }
    Ok(LockedItems { items, products })
}

pub(super) async fn lock_proofs(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    batch: &ShipmentStockBatch,
) -> Result<BTreeMap<Uuid, proof::Model>, PortError> {
    let ids = batch
        .lines
        .iter()
        .map(|line| line.fulfillment_item_id)
        .collect::<BTreeSet<_>>();
    let mut saved = BTreeMap::new();
    for id in ids {
        let query = proof::Entity::find_by_id(id).filter(proof::Column::TenantId.eq(tenant_id));
        let query = if txn.get_database_backend() == DbBackend::Sqlite {
            query
        } else {
            query.lock_exclusive()
        };
        if let Some(row) = query
            .one(txn)
            .await
            .map_err(|error| storage(context, error))?
        {
            saved.insert(id, row);
        }
    }
    Ok(saved)
}

pub(super) async fn lock_allocations(
    txn: &DatabaseTransaction,
    context: &PortContext,
    batch: &ShipmentStockBatch,
    tenant_id: Uuid,
    items: &BTreeMap<Uuid, inventory_item::Model>,
    saved: &BTreeMap<Uuid, proof::Model>,
) -> Result<BTreeMap<Uuid, reservation_item::Model>, PortError> {
    let keys = batch
        .lines
        .iter()
        .filter_map(|line| {
            line.variant_id
                .map(|variant| (items[&variant].id, line.order_line_item_id, variant))
        })
        .collect::<BTreeSet<_>>();
    let mut allocations = BTreeMap::new();
    for (item_id, line_id, variant_id) in keys {
        let line_binding = read_line_binding(
            txn,
            context,
            tenant_id,
            batch.order_id,
            line_id,
            Some(variant_id),
        )
        .await?;
        if line_binding
            .as_ref()
            .is_some_and(|row| row.inventory_item_id != Some(item_id))
        {
            return Err(history_unverified());
        }
        let mut retained = saved
            .values()
            .filter(|row| row.order_line_item_id == line_id)
            .map(|row| {
                if row.inventory_item_id != Some(item_id) {
                    return Err(history_unverified());
                }
                row.reservation_id.ok_or_else(history_unverified)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if let Some(binding) = line_binding.as_ref() {
            retained.insert(binding.reservation_id.ok_or_else(history_unverified)?);
        }
        if retained.len() > 1 {
            return Err(history_unverified());
        }
        // The whole saved line must have one retained allocation, including zero/terminal
        // rows and other items. A target-item filter could hide an earlier SKU/allocation.
        let query = reservation_item::Entity::find()
            .filter(reservation_item::Column::LineItemId.eq(line_id))
            .limit(2);
        let query = if txn.get_database_backend() == DbBackend::Sqlite {
            query
        } else {
            query.lock_exclusive()
        };
        let mut rows = query
            .all(txn)
            .await
            .map_err(|error| storage(context, error))?;
        if rows.len() != 1 {
            return Err(refused(
                "allocation_unverified",
                "shipment requires one unambiguous retained allocation on the saved line",
            ));
        }
        let allocation = rows.pop().ok_or_else(history_unverified)?;
        if retained.first().is_some_and(|id| *id != allocation.id) {
            return Err(history_unverified());
        }
        assert_return_binding(
            txn,
            context,
            tenant_id,
            line_id,
            item_id,
            allocation.location_id,
        )
        .await?;
        validate_allocation(&allocation, item_id, line_id, variant_id)?;
        if line_binding
            .as_ref()
            .is_some_and(|row| row.location_id != Some(allocation.location_id))
        {
            return Err(history_unverified());
        }
        for row in saved
            .values()
            .filter(|row| row.order_line_item_id == line_id)
        {
            if row.location_id != Some(allocation.location_id)
                || row.reservation_id != Some(allocation.id)
            {
                return Err(history_unverified());
            }
        }
        allocations.insert(line_id, allocation);
    }
    Ok(allocations)
}

fn validate_allocation(
    allocation: &reservation_item::Model,
    item_id: Uuid,
    line_id: Uuid,
    variant_id: Uuid,
) -> Result<(), PortError> {
    if allocation.id.is_nil()
        || allocation.inventory_item_id != item_id
        || allocation.line_item_id != Some(line_id)
        || allocation.location_id.is_nil()
        || allocation.quantity < 0
        || allocation.deleted_at.is_some() && allocation.quantity != 0
        || !allocation.metadata.is_object()
        || has_terminal_disposition(&allocation.metadata)
    {
        return Err(history_unverified());
    }
    for (key, expected) in [
        ("variant_id", variant_id),
        ("order_line_item_id", line_id),
        ("inventory_item_id", item_id),
        ("location_id", allocation.location_id),
    ] {
        if let Some(value) = allocation
            .metadata
            .get(key)
            .filter(|value| !value.is_null())
            && value.as_str().and_then(|value| Uuid::parse_str(value).ok()) != Some(expected)
        {
            return Err(history_unverified());
        }
    }
    Ok(())
}

pub(super) async fn lock_levels(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    allocations: &BTreeMap<Uuid, reservation_item::Model>,
) -> Result<BTreeMap<(Uuid, Uuid), inventory_level::Model>, PortError> {
    let locations = allocations
        .values()
        .map(|row| row.location_id)
        .collect::<BTreeSet<_>>();
    for location_id in locations {
        let query = stock_location::Entity::find_by_id(location_id)
            .filter(stock_location::Column::TenantId.eq(tenant_id));
        let query = if txn.get_database_backend() == DbBackend::Sqlite {
            query
        } else {
            query.lock_exclusive()
        };
        let location = query
            .one(txn)
            .await
            .map_err(|error| storage(context, error))?
            .ok_or_else(history_unverified)?;
        if !location.is_active() {
            return Err(refused(
                "location_archived",
                "saved shipment location is archived; reconcile without relocating stock",
            ));
        }
    }
    let keys = allocations
        .values()
        .map(|row| (row.inventory_item_id, row.location_id))
        .collect::<BTreeSet<_>>();
    let mut levels = BTreeMap::new();
    for (item_id, location_id) in keys {
        let query = inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(item_id))
            .filter(inventory_level::Column::LocationId.eq(location_id))
            .limit(2);
        let query = if txn.get_database_backend() == DbBackend::Sqlite {
            query
        } else {
            query.lock_exclusive()
        };
        let mut rows = query
            .all(txn)
            .await
            .map_err(|error| storage(context, error))?;
        if rows.len() != 1 {
            return Err(refused(
                "level_unverified",
                "shipment requires one saved inventory level",
            ));
        }
        let level = rows.pop().ok_or_else(history_unverified)?;
        if level.stocked_quantity < 0
            || level.reserved_quantity < 0
            || level.incoming_quantity < 0
            || level.reserved_quantity > level.stocked_quantity
        {
            return Err(history_unverified());
        }
        levels.insert((item_id, location_id), level);
    }
    Ok(levels)
}

/// A bounded distinct allocation projection, including other Fulfillment items for this line.
/// A later fulfillment cannot silently choose a newly-created/oldest replacement allocation.
#[derive(FromQueryResult)]
pub(super) struct LineBinding {
    order_id: Uuid,
    variant_id: Option<Uuid>,
    inventory_item_id: Option<Uuid>,
    reservation_id: Option<Uuid>,
    location_id: Option<Uuid>,
}

pub(super) async fn read_line_binding<C: ConnectionTrait>(
    db: &C,
    context: &PortContext,
    tenant_id: Uuid,
    order_id: Uuid,
    line_id: Uuid,
    variant_id: Option<Uuid>,
) -> Result<Option<LineBinding>, PortError> {
    let mut identities = proof::Entity::find()
        .select_only()
        .columns([
            proof::Column::OrderId,
            proof::Column::VariantId,
            proof::Column::InventoryItemId,
            proof::Column::ReservationId,
            proof::Column::LocationId,
        ])
        .filter(proof::Column::TenantId.eq(tenant_id))
        .filter(proof::Column::OrderLineItemId.eq(line_id))
        .distinct()
        .limit(2)
        .into_model::<LineBinding>()
        .all(db)
        .await
        .map_err(|error| storage(context, error))?;
    if identities.len() > 1 {
        return Err(history_unverified());
    }
    if let Some(row) = identities.pop() {
        if row.order_id != order_id || row.variant_id != variant_id {
            return Err(history_unverified());
        }
        let allocation = [row.inventory_item_id, row.reservation_id, row.location_id];
        if variant_id.is_some() && allocation.iter().any(|id| id.is_none_or(|id| id.is_nil()))
            || variant_id.is_none() && allocation.iter().any(Option::is_some)
        {
            return Err(history_unverified());
        }
        Ok(Some(row))
    } else {
        Ok(None)
    }
}

/// Domain-owned, transaction-scoped line fence. This follows Inventory's existing PostgreSQL
/// advisory-lock pattern; SQLite's zero-row write acquires its writer fence without changing
/// proof data or inventing a persistent lock/receipt/binding record.
pub(super) async fn lock_lines(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    batch: &ShipmentStockBatch,
) -> Result<(), PortError> {
    match txn.get_database_backend() {
        DbBackend::Postgres => {
            for line_id in batch
                .lines
                .iter()
                .map(|line| line.order_line_item_id)
                .collect::<BTreeSet<_>>()
            {
                let key = format!("inventory:shipment_stock_line:{tenant_id}:{line_id}");
                txn.execute_raw(sea_orm::Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                    [key.into()],
                ))
                .await
                .map_err(|error| storage(context, error))?;
            }
        }
        DbBackend::Sqlite => {
            txn.execute_raw(sea_orm::Statement::from_string(DbBackend::Sqlite,
                "UPDATE inventory_shipment_stock_proofs SET fulfillment_item_id = fulfillment_item_id WHERE 1 = 0"))
                .await.map_err(|error| storage(context, error))?;
        }
        _ => {
            return Err(PortError::unavailable(
                "inventory.shipment_stock_backend_unsupported",
                "native shipment provenance requires PostgreSQL or SQLite transaction fencing",
            )
            .with_retryable(false));
        }
    }
    Ok(())
}

pub(super) async fn assert_sku_less_has_no_allocation(
    txn: &DatabaseTransaction,
    context: &PortContext,
    line_id: Uuid,
) -> Result<(), PortError> {
    // Existence only, on the exact owner-correlated line. Do not select a foreign item's
    // identity/metadata, or reinterpret a zero/deleted/terminal tracked row as SKU-less.
    let retained = reservation_item::Entity::find()
        .select_only()
        .column(reservation_item::Column::Id)
        .filter(reservation_item::Column::LineItemId.eq(line_id))
        .limit(1)
        .into_tuple::<Uuid>()
        .one(txn)
        .await
        .map_err(|error| storage(context, error))?;
    if retained.is_some() {
        return Err(history_unverified());
    }
    Ok(())
}

async fn assert_return_binding(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    line_id: Uuid,
    item_id: Uuid,
    location_id: Uuid,
) -> Result<(), PortError> {
    use crate::return_restock::ledger;
    let bindings = ledger::Entity::find()
        .select_only()
        .column(ledger::Column::InventoryItemId)
        .column(ledger::Column::LocationId)
        .filter(ledger::Column::TenantId.eq(tenant_id))
        .filter(ledger::Column::OrderLineItemId.eq(line_id))
        .distinct()
        .limit(2)
        .into_tuple::<(Uuid, Uuid)>()
        .all(txn)
        .await
        .map_err(|error| storage(context, error))?;
    if bindings
        .iter()
        .any(|binding| *binding != (item_id, location_id))
    {
        return Err(history_unverified());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
