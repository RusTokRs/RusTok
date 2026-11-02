//! Quantity compatibility release is an anonymous pool operation, not an identity fallback.
//! Select ledger rows first, then bind every reduction to its proved original level before writes.
//! Fresh reserve uses the same inspection so it cannot grow an oversized/unreleasable pool.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, sea_query::Expr};
use uuid::Uuid;

use rustok_commerce_foundation::entities::{inventory_level, reservation_item, stock_location};
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

use super::insufficient_reservation_items_release_error;

pub(super) const QUANTITY_RESERVATION_ROW_LIMIT: usize = 4096;

pub(super) struct SelectedQuantityRelease {
    inventory_item_id: Uuid,
    reservations: Vec<(reservation_item::Model, i32)>,
    released_by_location: BTreeMap<Uuid, i32>,
    active_claims_by_location: BTreeMap<Uuid, i64>,
}

pub(super) struct QuantityReleasePlan {
    reservations: Vec<(reservation_item::Model, i32)>,
    levels: Vec<(inventory_level::Model, i32)>,
}

struct AnonymousPool {
    eligible: Vec<reservation_item::Model>,
    tracked: i64,
    active_claims_by_location: BTreeMap<Uuid, i64>,
}

pub(super) fn admit_new_reservation(
    inventory_item_id: Uuid,
    variant_id: Uuid,
    rows: Vec<reservation_item::Model>,
) -> CommerceResult<()> {
    if rows.len() >= QUANTITY_RESERVATION_ROW_LIMIT {
        return Err(CommerceError::Validation(format!(
            "Anonymous reservation pool is at its {QUANTITY_RESERVATION_ROW_LIMIT} row bound; release existing anonymous quantities or use the identity port for new allocations"
        )));
    }
    inspect_anonymous_pool(inventory_item_id, variant_id, rows)?;
    Ok(())
}

fn inspect_anonymous_pool(
    inventory_item_id: Uuid,
    variant_id: Uuid,
    mut rows: Vec<reservation_item::Model>,
) -> CommerceResult<AnonymousPool> {
    if inventory_item_id.is_nil() || variant_id.is_nil() {
        return Err(inconsistent_release());
    }
    if rows.len() > QUANTITY_RESERVATION_ROW_LIMIT {
        return Err(CommerceError::Validation(format!(
            "Quantity release exceeds the {QUANTITY_RESERVATION_ROW_LIMIT} anonymous pool row bound; reconcile saved anonymous allocations without guessing identities"
        )));
    }
    // FIFO is explicit only within the unkeyed compatibility pool. It is never a substitute
    // for a missing Cart/Order/reservation identity. UUID breaks equal-timestamp ties.
    rows.sort_by_key(|row| (row.created_at, row.id));
    let mut seen = BTreeSet::new();
    let mut eligible = Vec::new();
    let mut tracked = 0_i64;
    let mut active_claims_by_location = BTreeMap::<Uuid, i64>::new();
    for row in rows {
        if row.inventory_item_id != inventory_item_id
            || row.id.is_nil()
            || row.location_id.is_nil()
            || row.deleted_at.is_some()
            || row.quantity <= 0
            || !seen.insert(row.id)
        {
            return Err(inconsistent_release());
        }
        // Retain the minimum observed pool claims. bind_levels also requires the database
        // aggregate of every live claim, including keyed rows not loaded into this pool.
        let claims = active_claims_by_location
            .entry(row.location_id)
            .or_default();
        *claims = claims
            .checked_add(i64::from(row.quantity))
            .ok_or_else(inconsistent_release)?;
        if row.external_id.is_some() || row.line_item_id.is_some() {
            continue;
        }
        if row
            .metadata
            .get("source")
            .and_then(serde_json::Value::as_str)
            != Some("legacy_inventory_service")
        {
            continue;
        }
        let saved_variant = row
            .metadata
            .get("variant_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        if saved_variant != Some(variant_id)
            || crate::reservation_adoption::has_terminal_disposition(&row.metadata)
        {
            return Err(inconsistent_release());
        }
        // A leftover checkout/adoption label is positive evidence against anonymous ownership,
        // even if someone has removed the actual external/line bindings. Never repair it here.
        if [
            "reservation_id",
            "external_id",
            "line_item_id",
            "checkout_operation_id",
            "cart_id",
            "cart_line_item_id",
            "order_id",
            "order_line_item_id",
        ]
        .into_iter()
        .any(|key| row.metadata.get(key).is_some_and(|value| !value.is_null()))
        {
            continue;
        }
        tracked = tracked
            .checked_add(i64::from(row.quantity))
            .ok_or_else(inconsistent_release)?;
        eligible.push(row);
    }
    Ok(AnonymousPool {
        eligible,
        tracked,
        active_claims_by_location,
    })
}

impl SelectedQuantityRelease {
    pub(super) fn select(
        inventory_item_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
        rows: Vec<reservation_item::Model>,
    ) -> CommerceResult<Self> {
        if quantity <= 0 {
            return Err(inconsistent_release());
        }
        let AnonymousPool {
            eligible,
            tracked,
            active_claims_by_location,
        } = inspect_anonymous_pool(inventory_item_id, variant_id, rows)?;
        if i64::from(quantity) > tracked {
            return Err(insufficient_reservation_items_release_error(
                quantity, tracked,
            ));
        }
        let mut remaining = quantity;
        let mut reservations = Vec::new();
        let mut released_by_location = BTreeMap::<Uuid, i32>::new();
        for row in eligible {
            if remaining == 0 {
                break;
            }
            let released = remaining.min(row.quantity);
            remaining = remaining
                .checked_sub(released)
                .ok_or_else(inconsistent_release)?;
            let new_quantity = row
                .quantity
                .checked_sub(released)
                .ok_or_else(inconsistent_release)?;
            let location_quantity = released_by_location.entry(row.location_id).or_default();
            *location_quantity = location_quantity
                .checked_add(released)
                .ok_or_else(inconsistent_release)?;
            reservations.push((row, new_quantity));
        }
        if remaining != 0 {
            return Err(inconsistent_release());
        }
        Ok(Self {
            inventory_item_id,
            reservations,
            released_by_location,
            active_claims_by_location,
        })
    }

    pub(super) fn location_ids(&self) -> Vec<Uuid> {
        self.released_by_location.keys().copied().collect()
    }

    pub(super) fn bind_levels(
        self,
        tenant_id: Uuid,
        locations: Vec<stock_location::Model>,
        levels: Vec<inventory_level::Model>,
        active_claims: BTreeMap<Uuid, i64>,
    ) -> CommerceResult<QuantityReleasePlan> {
        let expected = self.released_by_location.len();
        if tenant_id.is_nil()
            || locations.len() != expected
            || levels.len() != expected
            || active_claims.len() != expected
        {
            return Err(inconsistent_release());
        }
        let mut owned_locations = BTreeMap::new();
        for location in locations {
            if location.tenant_id != tenant_id
                || !self.released_by_location.contains_key(&location.id)
                || owned_locations.insert(location.id, location).is_some()
            {
                return Err(inconsistent_release());
            }
        }
        let mut seen = BTreeSet::new();
        let mut changes = Vec::with_capacity(expected);
        for level in levels {
            let location = owned_locations
                .get(&level.location_id)
                .ok_or_else(inconsistent_release)?;
            let released = *self
                .released_by_location
                .get(&level.location_id)
                .ok_or_else(inconsistent_release)?;
            let selected_pool_claims = *self
                .active_claims_by_location
                .get(&level.location_id)
                .ok_or_else(inconsistent_release)?;
            let current_claims = *active_claims
                .get(&level.location_id)
                .ok_or_else(inconsistent_release)?;
            if level.id.is_nil()
                || level.inventory_item_id != self.inventory_item_id
                || !seen.insert(level.location_id)
                || level.reserved_quantity < 0
                || level.incoming_quantity < 0
                || current_claims < selected_pool_claims
                || i64::from(level.reserved_quantity) < current_claims
            {
                return Err(inconsistent_release());
            }
            let new_reserved = level
                .reserved_quantity
                .checked_sub(released)
                .filter(|quantity| *quantity >= 0)
                .ok_or_else(inconsistent_release)?;
            // Preserve I021's archived-level invariant, including old invalid rows. A correction
            // to an all-zero level is allowed; no implicit reactivation or warehouse relocation.
            if location.deleted_at.is_some()
                && (level.stocked_quantity != 0
                    || new_reserved != 0
                    || level.incoming_quantity != 0)
            {
                return Err(CommerceError::Validation(
                    "Quantity release would retain nonzero inventory at an archived original location; reconcile that allocation without relocation".to_string(),
                ));
            }
            changes.push((level, new_reserved));
        }
        changes.sort_by_key(|(level, _)| (level.location_id, level.id));
        Ok(QuantityReleasePlan {
            reservations: self.reservations,
            levels: changes,
        })
    }
}

impl QuantityReleasePlan {
    pub(super) async fn apply(self, txn: &DatabaseTransaction) -> CommerceResult<()> {
        let now = Utc::now();
        for (row, new_quantity) in self.reservations {
            let mut update = reservation_item::Entity::update_many()
                .col_expr(
                    reservation_item::Column::Quantity,
                    Expr::value(new_quantity),
                )
                .col_expr(reservation_item::Column::UpdatedAt, Expr::value(now))
                .filter(reservation_item::Column::Id.eq(row.id))
                .filter(reservation_item::Column::InventoryItemId.eq(row.inventory_item_id))
                .filter(reservation_item::Column::LocationId.eq(row.location_id))
                .filter(reservation_item::Column::Quantity.eq(row.quantity))
                .filter(reservation_item::Column::DeletedAt.is_null())
                .filter(reservation_item::Column::ExternalId.is_null())
                .filter(reservation_item::Column::LineItemId.is_null());
            if new_quantity == 0 {
                update = update.col_expr(reservation_item::Column::DeletedAt, Expr::value(now));
            }
            // No metadata rewrite or invented compatibility receipt; suppressed/drifted writes
            // cannot acknowledge a release. The caller owns rollback of the complete transaction.
            if update.exec(txn).await?.rows_affected != 1 {
                return Err(inconsistent_release());
            }
        }
        for (level, new_reserved) in self.levels {
            let updated = inventory_level::Entity::update_many()
                .col_expr(
                    inventory_level::Column::ReservedQuantity,
                    Expr::value(new_reserved),
                )
                .col_expr(inventory_level::Column::UpdatedAt, Expr::value(now))
                .filter(inventory_level::Column::Id.eq(level.id))
                .filter(inventory_level::Column::InventoryItemId.eq(level.inventory_item_id))
                .filter(inventory_level::Column::LocationId.eq(level.location_id))
                .filter(inventory_level::Column::ReservedQuantity.eq(level.reserved_quantity))
                .filter(inventory_level::Column::StockedQuantity.eq(level.stocked_quantity))
                .filter(inventory_level::Column::IncomingQuantity.eq(level.incoming_quantity))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(inconsistent_release());
            }
        }
        Ok(())
    }
}

fn inconsistent_release() -> CommerceError {
    CommerceError::Validation(
        "Anonymous reservation provenance or original level is inconsistent; use the identity port for keyed allocations and reconcile invalid history without fallback".to_string(),
    )
}

#[cfg(test)]
#[path = "reservation_release/tests.rs"]
mod tests;
