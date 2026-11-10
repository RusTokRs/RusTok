//! Operator-decided restock at a saved Order-line allocation.
//!
//! Order owns the saved SKU and return facts; Commerce supplies those facts after a complete
//! owner-read correlation. Inventory owns allocation identity, stock and the existing ledger.
//! The ledger's return-item primary key is the mutation identity, not a retry/no-effect hint.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait, sea_query::OnConflict,
};
use uuid::Uuid;

use crate::entities::{inventory_level, reservation_item, stock_location};
use crate::ports_impl::{
    load_inventory_item_for_update, load_tenant_variant, parse_port_tenant_id,
    storage_unavailable_with_context, validate_non_nil_uuid,
};
use crate::reservation_adoption::has_terminal_disposition;
use crate::stock_events::{StockEventPlan, StockLevelChange};

const RESTOCK_OPERATION: &str = "restock_returned_items";
pub const MAX_RETURN_RESTOCK_ITEMS: usize = 1024;

/// Inventory-owned ledger of restocked return items. Its applied schema is unchanged.
pub mod ledger {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "inventory_return_restocks")]
    pub struct Model {
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

/// One operator-selected return item, bound to its immutable Order-owned SKU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnRestockLine {
    pub return_item_id: Uuid,
    pub return_id: Uuid,
    pub order_line_item_id: Uuid,
    /// None is an explicit SKU-less saved Order binding, never lookup failure.
    pub variant_id: Option<Uuid>,
    pub quantity: i32,
}

/// Exactly one outcome per requested return item. Correlate by UUID, not vector position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReturnRestockOutcome {
    pub restocked: Vec<(Uuid, i32)>,
    pub already_restocked: Vec<Uuid>,
    pub untracked: Vec<Uuid>,
}

#[derive(Clone, Copy)]
struct Movement {
    line: ReturnRestockLine,
    inventory_item_id: Uuid,
    location_id: Uuid,
}

impl Movement {
    fn level_key(&self) -> (Uuid, Uuid) {
        (self.inventory_item_id, self.location_id)
    }
}

/// Native Inventory command; callers enforce the positive context budget around this future.
/// All new stock/ledger writes commit together. Exact replay never changes a timestamp or
/// consults current reservation selection. Errors do not establish whether an earlier call
/// committed: recovery must retain these same return-item identities and selected quantities.
pub async fn restock_returned_items(
    db: &DatabaseConnection,
    context: PortContext,
    lines: &[ReturnRestockLine],
) -> Result<ReturnRestockOutcome, PortError> {
    context.require_policy(PortCallPolicy::write())?;
    let tenant_id = parse_port_tenant_id(&context, RESTOCK_OPERATION)?;
    validate_request(lines)?;
    let txn = db
        .begin()
        .await
        .map_err(|error| storage_unavailable_with_context(&context, RESTOCK_OPERATION, error))?;

    let variant_ids = lines
        .iter()
        .filter_map(|line| line.variant_id)
        .collect::<BTreeSet<_>>();
    let mut products = BTreeMap::new();
    for variant_id in &variant_ids {
        // Tenant proof only; archived/hidden saved SKUs do not trigger new eligibility checks.
        let variant =
            load_tenant_variant(&context, RESTOCK_OPERATION, &txn, tenant_id, *variant_id).await?;
        products.insert(*variant_id, variant.product_id);
    }
    let mut items = BTreeMap::new();
    for variant_id in variant_ids {
        // INVARIANT: every Variant was proved tenant-scoped above, before any tenant-less
        // item/reservation read. Identity reserve/release/adoption share this stable item lock.
        let item = load_inventory_item_for_update(&context, RESTOCK_OPERATION, &txn, variant_id)
            .await?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "inventory.return_restock_item_missing",
                    "tracked return inventory is missing; reconcile the saved allocation",
                )
            })?;
        items.insert(variant_id, item);
    }

    let mut outcome = ReturnRestockOutcome::default();
    let mut checked_locations = BTreeSet::new();
    let mut movements = BTreeMap::new();
    for line in lines {
        let existing = read_receipt(&txn, &context, tenant_id, line.return_item_id).await?;
        let Some(variant_id) = line.variant_id else {
            if existing.is_some() {
                return Err(identity_conflict());
            }
            outcome.untracked.push(line.return_item_id);
            continue;
        };
        let item = items.get(&variant_id).ok_or_else(identity_conflict)?;
        if let Some(saved) = existing {
            validate_replay(&saved, tenant_id, line, item.id, None)?;
            prove_location(
                &txn,
                &context,
                tenant_id,
                saved.location_id,
                &mut checked_locations,
            )
            .await?;
            outcome.already_restocked.push(line.return_item_id);
            continue;
        }

        // Fresh stock may not credit units already restored by shipment cancellation. Exact
        // receipt replay above stays read-only and bypasses this new provenance dependency.
        crate::shipment_stock::assert_no_restored_shipment_stock(
            &txn,
            &context,
            tenant_id,
            line.order_line_item_id,
        )
        .await?;

        // INVARIANT: the item is tenant-proved. Include consumed history, but never select
        // the oldest/first of several allocations or infer SKU-less from an absent row.
        let reservation = lock_line_reservation(
            &txn,
            &context,
            item.id,
            line.order_line_item_id,
        )
        .await?;
        validate_allocation(&reservation, line, item.id, variant_id)?;
        movements.insert(
            line.return_item_id,
            Movement {
                line: *line,
                inventory_item_id: item.id,
                location_id: reservation.location_id,
            },
        );
    }

    // Read-only replay ownership proof is not permission to write a closed location.
    // Lock fresh receiving locations in stable order before any levels or receipts.
    lock_receiving_locations(&txn, &context, tenant_id, &movements).await?;

    let level_keys = movements
        .values()
        .map(Movement::level_key)
        .collect::<BTreeSet<_>>();
    let mut levels = BTreeMap::new();
    for (item_id, location_id) in level_keys {
        // INVARIANT: both the item and the saved location were tenant-proved above.
        let level = lock_inventory_level(&txn, &context, item_id, location_id).await?;
        levels.insert((item_id, location_id), level);
    }

    let now = Utc::now();
    let mut additions = BTreeMap::<(Uuid, Uuid), i32>::new();
    for movement in movements.values() {
        // The global primary-key fence also covers conflicting concurrent SKU claims. A
        // foreign-tenant collision is refused without reading another tenant's receipt.
        let affected = ledger::Entity::insert(ledger::ActiveModel {
            return_item_id: Set(movement.line.return_item_id),
            tenant_id: Set(tenant_id),
            return_id: Set(movement.line.return_id),
            order_line_item_id: Set(movement.line.order_line_item_id),
            inventory_item_id: Set(movement.inventory_item_id),
            location_id: Set(movement.location_id),
            quantity: Set(movement.line.quantity),
            created_at: Set(now.into()),
        })
        .on_conflict(
            OnConflict::column(ledger::Column::ReturnItemId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&txn)
        .await
        .map_err(|error| storage_unavailable_with_context(&context, RESTOCK_OPERATION, error))?;
        match affected {
            1 => {
                let total = additions.entry(movement.level_key()).or_default();
                *total = total
                    .checked_add(movement.line.quantity)
                    .ok_or_else(quantity_overflow)?;
                outcome
                    .restocked
                    .push((movement.line.return_item_id, movement.line.quantity));
            }
            0 => {
                let saved = read_receipt(&txn, &context, tenant_id, movement.line.return_item_id)
                    .await?
                    .ok_or_else(identity_conflict)?;
                validate_replay(
                    &saved,
                    tenant_id,
                    &movement.line,
                    movement.inventory_item_id,
                    Some(movement.location_id),
                )?;
                outcome.already_restocked.push(movement.line.return_item_id);
            }
            _ => return Err(identity_conflict()),
        }
    }

    // Only successfully admitted new receipts contribute to this physical batch. Exact
    // replay (including a lost primary-key race) must not create another invalidation.
    let item_bindings = items
        .iter()
        .map(|(variant_id, item)| {
            Ok((
                item.id,
                (
                    *variant_id,
                    *products.get(variant_id).ok_or_else(identity_conflict)?,
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, PortError>>()?;
    let changes = additions
        .iter()
        .map(|(key, addition)| {
            let level = levels.get(key).ok_or_else(identity_conflict)?;
            let (variant_id, product_id) =
                *item_bindings.get(&key.0).ok_or_else(identity_conflict)?;
            Ok(StockLevelChange {
                inventory_item_id: key.0,
                variant_id,
                product_id,
                location_id: key.1,
                old_stocked: level.stocked_quantity,
                old_reserved: level.reserved_quantity,
                new_stocked: level
                    .stocked_quantity
                    .checked_add(*addition)
                    .ok_or_else(quantity_overflow)?,
                new_reserved: level.reserved_quantity,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    let stock_events = StockEventPlan::prepare(&txn, &context, RESTOCK_OPERATION, changes).await?;
    for (key, addition) in additions {
        let level = levels.remove(&key).ok_or_else(identity_conflict)?;
        let stocked = level
            .stocked_quantity
            .checked_add(addition)
            .ok_or_else(quantity_overflow)?;
        let mut active: inventory_level::ActiveModel = level.into();
        active.stocked_quantity = Set(stocked);
        active.updated_at = Set(now.into());
        active.update(&txn).await.map_err(|error| {
            storage_unavailable_with_context(&context, RESTOCK_OPERATION, error)
        })?;
    }
    stock_events
        .publish(&txn, &context, RESTOCK_OPERATION)
        .await?;
    txn.commit()
        .await
        .map_err(|error| storage_unavailable_with_context(&context, RESTOCK_OPERATION, error))?;
    Ok(outcome)
}

fn validate_request(lines: &[ReturnRestockLine]) -> Result<(), PortError> {
    if lines.is_empty() || lines.len() > MAX_RETURN_RESTOCK_ITEMS {
        return Err(PortError::validation(
            "inventory.return_restock_batch_invalid",
            "return restock requires between 1 and 1024 distinct return items",
        ));
    }
    let mut seen = BTreeSet::new();
    let return_id = lines[0].return_id;
    for line in lines {
        validate_non_nil_uuid(line.return_id, "return_id")?;
        validate_non_nil_uuid(line.return_item_id, "return_item_id")?;
        validate_non_nil_uuid(line.order_line_item_id, "order_line_item_id")?;
        if let Some(variant_id) = line.variant_id {
            validate_non_nil_uuid(variant_id, "variant_id")?;
        }
        if line.quantity <= 0 || line.return_id != return_id || !seen.insert(line.return_item_id) {
            return Err(PortError::validation(
                "inventory.return_restock_request_invalid",
                "restock requires positive quantities and distinct items belonging to one return",
            ));
        }
    }
    Ok(())
}

fn validate_allocation(
    reservation: &reservation_item::Model,
    line: &ReturnRestockLine,
    item_id: Uuid,
    variant_id: Uuid,
) -> Result<(), PortError> {
    if reservation.id.is_nil()
        || reservation.inventory_item_id != item_id
        || reservation.line_item_id != Some(line.order_line_item_id)
        || reservation.location_id.is_nil()
        || reservation.quantity < 0
        || (reservation.deleted_at.is_some() && reservation.quantity != 0)
        || !reservation.metadata.is_object()
        || has_terminal_disposition(&reservation.metadata)
    {
        return Err(identity_conflict());
    }
    for (key, expected) in [
        ("variant_id", variant_id),
        ("order_line_item_id", line.order_line_item_id),
        ("inventory_item_id", item_id),
        ("location_id", reservation.location_id),
    ] {
        if let Some(value) = reservation
            .metadata
            .get(key)
            .filter(|value| !value.is_null())
            && value.as_str().and_then(|value| Uuid::parse_str(value).ok()) != Some(expected)
        {
            return Err(identity_conflict());
        }
    }
    Ok(())
}

async fn read_receipt(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    return_item_id: Uuid,
) -> Result<Option<ledger::Model>, PortError> {
    ledger::Entity::find_by_id(return_item_id)
        .filter(ledger::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, RESTOCK_OPERATION, error))
}

fn validate_replay(
    saved: &ledger::Model,
    tenant_id: Uuid,
    line: &ReturnRestockLine,
    item_id: Uuid,
    expected_location: Option<Uuid>,
) -> Result<(), PortError> {
    if saved.tenant_id != tenant_id
        || saved.return_item_id != line.return_item_id
        || saved.return_id != line.return_id
        || saved.order_line_item_id != line.order_line_item_id
        || saved.inventory_item_id != item_id
        || saved.quantity != line.quantity
        || saved.location_id.is_nil()
        || expected_location.is_some_and(|location| location != saved.location_id)
    {
        return Err(identity_conflict());
    }
    Ok(())
}

async fn prove_location(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    location_id: Uuid,
    checked: &mut BTreeSet<Uuid>,
) -> Result<(), PortError> {
    if checked.contains(&location_id) {
        return Ok(());
    }
    // Exact replay changes no stock: archived/hidden ownership proof remains valid.
    stock_location::Entity::find_by_id(location_id)
        .filter(stock_location::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, RESTOCK_OPERATION, error))?
        .ok_or_else(identity_conflict)?;
    checked.insert(location_id);
    Ok(())
}

async fn lock_line_reservation(
    txn: &DatabaseTransaction,
    context: &PortContext,
    inventory_item_id: Uuid,
    line_item_id: Uuid,
) -> Result<reservation_item::Model, PortError> {
    let query = reservation_item::Entity::find()
        .filter(reservation_item::Column::InventoryItemId.eq(inventory_item_id))
        .filter(reservation_item::Column::LineItemId.eq(line_item_id))
        .limit(2);
    let query = if txn.get_database_backend() == DbBackend::Sqlite {
        query
    } else {
        query.lock_exclusive()
    };
    let mut reservations = query.all(txn).await.map_err(|error| {
        storage_unavailable_with_context(context, RESTOCK_OPERATION, error)
    })?;
    if reservations.len() != 1 {
        return Err(PortError::invariant_violation(
            "inventory.return_restock_allocation_unverified",
            "tracked return requires exactly one saved allocation; reconcile its history",
        ));
    }
    reservations.pop().ok_or_else(identity_conflict)
}

async fn lock_inventory_level(
    txn: &DatabaseTransaction,
    context: &PortContext,
    item_id: Uuid,
    location_id: Uuid,
) -> Result<inventory_level::Model, PortError> {
    let query = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(item_id))
        .filter(inventory_level::Column::LocationId.eq(location_id))
        .limit(2);
    let query = if txn.get_database_backend() == DbBackend::Sqlite {
        query
    } else {
        query.lock_exclusive()
    };
    let mut matching = query.all(txn).await.map_err(|error| {
        storage_unavailable_with_context(context, RESTOCK_OPERATION, error)
    })?;
    if matching.len() != 1 {
        return Err(PortError::invariant_violation(
            "inventory.return_restock_level_unverified",
            "saved return allocation requires exactly one inventory level",
        ));
    }
    let level = matching.pop().ok_or_else(identity_conflict)?;
    if level.stocked_quantity < 0 || level.reserved_quantity < 0 || level.incoming_quantity < 0 {
        return Err(identity_conflict());
    }
    Ok(level)
}

async fn lock_receiving_locations(
    txn: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    movements: &BTreeMap<Uuid, Movement>,
) -> Result<(), PortError> {
    let location_ids = movements
        .values()
        .map(|movement| movement.location_id)
        .collect::<BTreeSet<_>>();
    for location_id in location_ids {
        let receiving_query = stock_location::Entity::find_by_id(location_id)
            .filter(stock_location::Column::TenantId.eq(tenant_id));
        let receiving_query = if txn.get_database_backend() == DbBackend::Sqlite {
            receiving_query
        } else {
            receiving_query.lock_exclusive()
        };
        let location = receiving_query
            .one(txn)
            .await
            .map_err(|error| storage_unavailable_with_context(context, RESTOCK_OPERATION, error))?
            .ok_or_else(identity_conflict)?;
        // INVARIANT: the existing inactive-location storage guard forbids nonzero stock
        // here. Preserve its schema and the saved identity; do not retry a doomed write or
        // reactivate/relocate the warehouse. PostgreSQL locks serialize concurrent archival;
        // SQLite retains its transaction and the existing storage guard.
        if !location.is_active() {
            return Err(PortError::invariant_violation(
                "inventory.return_restock_location_archived",
                "saved return location is archived; reconcile the original location before same-identity recovery",
            ));
        }
    }
    Ok(())
}

fn identity_conflict() -> PortError {
    PortError::invariant_violation(
        "inventory.return_restock_identity_conflict",
        "saved return stock identities or history disagree; reconcile without changing mutation keys",
    )
}

fn quantity_overflow() -> PortError {
    PortError::invariant_violation(
        "inventory.return_restock_quantity_overflow",
        "return stock arithmetic exceeds its supported range; inventory operator review is required",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortErrorKind;
    use serde_json::{Value, json};

    fn line() -> ReturnRestockLine {
        ReturnRestockLine {
            return_item_id: Uuid::new_v4(),
            return_id: Uuid::new_v4(),
            order_line_item_id: Uuid::new_v4(),
            variant_id: Some(Uuid::new_v4()),
            quantity: 2,
        }
    }

    fn reservation(line: &ReturnRestockLine, item_id: Uuid) -> reservation_item::Model {
        reservation_item::Model {
            id: Uuid::new_v4(),
            inventory_item_id: item_id,
            location_id: Uuid::new_v4(),
            quantity: 0,
            line_item_id: Some(line.order_line_item_id),
            description: None,
            external_id: Some("retained-checkout-identity".into()),
            metadata: json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            deleted_at: Some(Utc::now().into()),
        }
    }

    fn receipt(
        tenant_id: Uuid,
        line: &ReturnRestockLine,
        item_id: Uuid,
        location_id: Uuid,
    ) -> ledger::Model {
        ledger::Model {
            return_item_id: line.return_item_id,
            tenant_id,
            return_id: line.return_id,
            order_line_item_id: line.order_line_item_id,
            inventory_item_id: item_id,
            location_id,
            quantity: line.quantity,
            created_at: Utc::now().into(),
        }
    }

    #[test]
    fn return_stock_request_requires_a_bounded_nonempty_single_return_batch() {
        let value = line();
        assert!(validate_request(&[]).is_err());
        assert!(validate_request(&[value, value]).is_err());
        let mut other = line();
        other.return_id = value.return_id;
        assert!(validate_request(&[value, other]).is_ok());
        other.return_id = Uuid::new_v4();
        assert!(validate_request(&[value, other]).is_err());
        let maximum = (0..MAX_RETURN_RESTOCK_ITEMS)
            .map(|_| ReturnRestockLine {
                return_item_id: Uuid::new_v4(),
                ..value
            })
            .collect::<Vec<_>>();
        assert!(validate_request(&maximum).is_ok());
        let mut oversized = maximum;
        oversized.push(ReturnRestockLine {
            return_item_id: Uuid::new_v4(),
            ..value
        });
        assert!(validate_request(&oversized).is_err());
    }

    #[test]
    fn return_stock_request_rejects_nil_ids_nonpositive_quantities_and_nil_sku() {
        let value = line();
        for changed in [
            ReturnRestockLine {
                return_item_id: Uuid::nil(),
                ..value
            },
            ReturnRestockLine {
                return_id: Uuid::nil(),
                ..value
            },
            ReturnRestockLine {
                order_line_item_id: Uuid::nil(),
                ..value
            },
            ReturnRestockLine {
                variant_id: Some(Uuid::nil()),
                ..value
            },
            ReturnRestockLine {
                quantity: 0,
                ..value
            },
            ReturnRestockLine {
                quantity: -1,
                ..value
            },
        ] {
            assert!(validate_request(&[changed]).is_err());
        }
        assert!(
            validate_request(&[ReturnRestockLine {
                variant_id: None,
                ..value
            }])
            .is_ok()
        );
    }

    #[test]
    fn restock_replay_compares_every_saved_identity_and_selected_quantity() {
        let tenant_id = Uuid::new_v4();
        let value = line();
        let item_id = Uuid::new_v4();
        let location_id = Uuid::new_v4();
        let saved = receipt(tenant_id, &value, item_id, location_id);
        let before = saved.clone();
        validate_replay(&saved, tenant_id, &value, item_id, None).expect("exact receipt");
        let mut bad = Vec::new();
        let mut row = saved.clone();
        row.tenant_id = Uuid::new_v4();
        bad.push(row);
        let mut row = saved.clone();
        row.return_item_id = Uuid::new_v4();
        bad.push(row);
        let mut row = saved.clone();
        row.return_id = Uuid::new_v4();
        bad.push(row);
        let mut row = saved.clone();
        row.order_line_item_id = Uuid::new_v4();
        bad.push(row);
        let mut row = saved.clone();
        row.inventory_item_id = Uuid::new_v4();
        bad.push(row);
        let mut row = saved.clone();
        row.quantity = 1;
        bad.push(row);
        let mut row = saved.clone();
        row.location_id = Uuid::nil();
        bad.push(row);
        for row in bad {
            let error =
                validate_replay(&row, tenant_id, &value, item_id, None).expect_err("mismatch");
            assert_eq!(error.kind, PortErrorKind::InvariantViolation);
            assert!(!error.retryable);
        }
        assert!(validate_replay(&saved, tenant_id, &value, item_id, Some(Uuid::new_v4())).is_err());
        assert_eq!(saved, before);
    }

    #[test]
    fn consumed_allocation_can_retain_its_saved_location_without_metadata_repair() {
        let value = line();
        let item_id = Uuid::new_v4();
        let mut saved = reservation(&value, item_id);
        saved.metadata = json!({
            "variant_id": value.variant_id.expect("SKU"),
            "order_line_item_id": value.order_line_item_id,
            "location_id": saved.location_id,
            "inventory_item_id": item_id,
            "retained": {"inspection": "unchanged"},
        });
        let before = saved.clone();
        validate_allocation(&saved, &value, item_id, value.variant_id.expect("SKU"))
            .expect("saved allocation");
        assert_eq!(saved, before);
    }

    #[test]
    fn release_disposal_and_supersession_facts_cannot_be_repaired_into_a_return_allocation() {
        let value = line();
        let item_id = Uuid::new_v4();
        for key in [
            "inventory_disposition",
            "superseded_by",
            "released_at",
            "disposed_at",
            "superseded_at",
        ] {
            let mut saved = reservation(&value, item_id);
            saved.metadata[key] = json!("retained-terminal-fact");
            let before = saved.clone();
            assert!(
                validate_allocation(&saved, &value, item_id, value.variant_id.expect("SKU"))
                    .is_err()
            );
            assert_eq!(saved, before);
            saved.metadata[key] = Value::Null;
            assert!(
                validate_allocation(&saved, &value, item_id, value.variant_id.expect("SKU"))
                    .is_ok()
            );
        }
    }

    #[test]
    fn contradictory_saved_allocation_labels_refuse_without_clearing_history() {
        let value = line();
        let item_id = Uuid::new_v4();
        for key in [
            "variant_id",
            "order_line_item_id",
            "inventory_item_id",
            "location_id",
        ] {
            let mut saved = reservation(&value, item_id);
            saved.metadata[key] = json!(Uuid::new_v4());
            assert!(
                validate_allocation(&saved, &value, item_id, value.variant_id.expect("SKU"))
                    .is_err()
            );
        }
        let mut saved = reservation(&value, item_id);
        saved.quantity = 1;
        assert!(
            validate_allocation(&saved, &value, item_id, value.variant_id.expect("SKU")).is_err()
        );
        saved.quantity = -1;
        saved.deleted_at = None;
        assert!(
            validate_allocation(&saved, &value, item_id, value.variant_id.expect("SKU")).is_err()
        );
    }
}
