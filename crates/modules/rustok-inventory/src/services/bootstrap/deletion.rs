//! Guard the existing Product lifecycle hook; never turn stock/history into empty state.

use std::collections::BTreeSet;

use rustok_commerce_foundation::entities::product_variant;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Select, Statement,
};
use uuid::Uuid;

use crate::entities::{inventory_item, inventory_level, reservation_item, stock_location};
use crate::return_restock::ledger;
use crate::shipment_stock::proof;

/// Bound for the existing atomic Product/Inventory deletion collaboration, not a SKU limit.
pub const MAX_CATALOG_DELETE_VARIANTS: usize = 256;
const MAX_DELETE_LEVELS: usize = 4096;

/// Finite owner refusals. No saved labels, database text or submitted values are public copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogDeletionBlocker {
    InvalidRequest,
    LimitExceeded,
    IdentityUnavailable,
    HistoryPresent,
    StockPresent,
    StateUnverified,
    IsolationUnsupported,
    ConcurrentChange,
}

impl CatalogDeletionBlocker {
    /// Closed wire-policy inventory, shared by Product's safe error adapters.
    pub const ALL: [Self; 8] = [
        Self::InvalidRequest,
        Self::LimitExceeded,
        Self::IdentityUnavailable,
        Self::HistoryPresent,
        Self::StockPresent,
        Self::StateUnverified,
        Self::IsolationUnsupported,
        Self::ConcurrentChange,
    ];

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => {
                "Inventory deletion requires a nonnil tenant and distinct nonnil SKU identities"
            }
            Self::LimitExceeded => {
                "Inventory deletion exceeds its atomic batch bound; use a bounded owner-governed catalog reconciliation"
            }
            Self::IdentityUnavailable => {
                "Inventory deletion cannot prove every tenant-owned SKU and inventory identity; reconcile the catalog first"
            }
            Self::HistoryPresent => {
                "Saved reservation or stock history must be retained; archive the product instead of deleting its SKUs"
            }
            Self::StockPresent => {
                "SKU inventory is not empty; reconcile stocked, reserved and incoming quantities through Inventory or archive the product"
            }
            Self::StateUnverified => {
                "Inventory deletion cannot prove the original location and complete level bindings; reconcile without relocating stock"
            }
            Self::IsolationUnsupported => {
                "Inventory deletion requires a current-state transaction; PostgreSQL must use READ COMMITTED isolation"
            }
            Self::ConcurrentChange => {
                "Inventory deletion state changed; reload and reconcile the same catalog command"
            }
        }
    }
}

impl std::fmt::Display for CatalogDeletionBlocker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogDeletionError {
    #[error("{0}")]
    Rejected(CatalogDeletionBlocker),
    #[error("Inventory catalog deletion storage failed: {0}")]
    Database(#[from] DbErr),
}

impl From<CatalogDeletionBlocker> for CatalogDeletionError {
    fn from(blocker: CatalogDeletionBlocker) -> Self {
        Self::Rejected(blocker)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
struct Level {
    id: Uuid,
    inventory_item_id: Uuid,
    location_id: Uuid,
    stocked_quantity: i32,
    reserved_quantity: i32,
    incoming_quantity: i32,
}

impl Level {
    fn binding(&self) -> (Uuid, Uuid, Uuid) {
        (self.inventory_item_id, self.location_id, self.id)
    }

    fn is_empty(&self) -> bool {
        self.stocked_quantity == 0 && self.reserved_quantity == 0 && self.incoming_quantity == 0
    }
}

struct DeletionPlan {
    /// (item, Variant), locked in Variant UUID order shared with shipment/return writers.
    items: Vec<(Uuid, Uuid)>,
    levels: Vec<Level>,
}

pub(super) async fn delete_records(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
    variant_ids: &[Uuid],
) -> Result<(), CatalogDeletionError> {
    if actor_id.is_nil() {
        return Err(CatalogDeletionBlocker::InvalidRequest.into());
    }
    let variant_ids = validate_request(tenant_id, variant_ids)?;
    if variant_ids.is_empty() {
        return Ok(());
    }
    require_current_snapshot(txn).await?;
    let plan = DeletionPlan::load(txn, tenant_id, &variant_ids).await?;
    plan.apply(txn, tenant_id, actor_id).await
}

fn validate_request(
    tenant_id: Uuid,
    variant_ids: &[Uuid],
) -> Result<Vec<Uuid>, CatalogDeletionError> {
    if tenant_id.is_nil() || variant_ids.iter().any(Uuid::is_nil) {
        return Err(CatalogDeletionBlocker::InvalidRequest.into());
    }
    if variant_ids.len() > MAX_CATALOG_DELETE_VARIANTS {
        return Err(CatalogDeletionBlocker::LimitExceeded.into());
    }
    let ids = variant_ids.iter().copied().collect::<BTreeSet<_>>();
    if ids.len() != variant_ids.len() {
        return Err(CatalogDeletionBlocker::InvalidRequest.into());
    }
    Ok(ids.into_iter().collect())
}

async fn require_current_snapshot(txn: &DatabaseTransaction) -> Result<(), CatalogDeletionError> {
    // A repeatable PostgreSQL snapshot could miss a level inserted before the item lock was
    // obtained. FOR UPDATE of an unchanged parent does not advance that older snapshot.
    if txn.get_database_backend() == DbBackend::Postgres {
        let row = txn
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SHOW transaction_isolation",
            ))
            .await?
            .ok_or(CatalogDeletionBlocker::IsolationUnsupported)?;
        let isolation: String = row.try_get("", "transaction_isolation")?;
        if isolation != "read committed" {
            return Err(CatalogDeletionBlocker::IsolationUnsupported.into());
        }
    }
    Ok(())
}

// These are current locking reads on MySQL as well as PostgreSQL. SQLite's transaction
// must obtain its writer lock before effects; a conflicting snapshot upgrade must fail.
fn current_read<E: EntityTrait>(txn: &DatabaseTransaction, query: Select<E>) -> Select<E> {
    if txn.get_database_backend() == DbBackend::Sqlite {
        query
    } else {
        query.lock_exclusive()
    }
}

impl DeletionPlan {
    async fn load(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<Self, CatalogDeletionError> {
        // Structural tenant identity proof in the existing transaction-sharing native
        // collaboration; no Product eligibility/publication policy is inferred here.
        let variants = current_read(
            txn,
            product_variant::Entity::find()
                .select_only()
                .column(product_variant::Column::Id)
                .filter(product_variant::Column::TenantId.eq(tenant_id))
                .filter(product_variant::Column::Id.is_in(variant_ids.iter().copied()))
                .order_by_asc(product_variant::Column::Id)
                .limit((MAX_CATALOG_DELETE_VARIANTS + 1) as u64),
        )
        .into_tuple::<Uuid>()
        .all(txn)
        .await?;
        if variants.as_slice() != variant_ids {
            return Err(CatalogDeletionBlocker::IdentityUnavailable.into());
        }

        // INVARIANT: all Variant UUIDs above are proved inside the current tenant before any
        // tenant-less Inventory item, level or opaque history-reference lookup.
        let items = current_read(
            txn,
            inventory_item::Entity::find()
                .select_only()
                .column(inventory_item::Column::Id)
                .column(inventory_item::Column::VariantId)
                .filter(inventory_item::Column::VariantId.is_in(variant_ids.iter().copied()))
                .order_by_asc(inventory_item::Column::VariantId)
                .order_by_asc(inventory_item::Column::Id)
                .limit((MAX_CATALOG_DELETE_VARIANTS + 1) as u64),
        )
        .into_tuple::<(Uuid, Uuid)>()
        .all(txn)
        .await?;
        let item_ids = items.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
        if items.len() != variant_ids.len()
            || item_ids.len() != items.len()
            || item_ids.iter().any(Uuid::is_nil)
            || !items
                .iter()
                .zip(variant_ids)
                .all(|((_, actual), expected)| actual == expected)
        {
            return Err(CatalogDeletionBlocker::IdentityUnavailable.into());
        }
        if has_history(txn, variant_ids, &item_ids).await? {
            return Err(CatalogDeletionBlocker::HistoryPresent.into());
        }

        // Discovery does not lock levels before their original locations. Under MySQL's
        // repeatable snapshot it may be older; the complete current locking reread below
        // must match its identities/bindings before any effect can be admitted.
        let discovered = level_query(&item_ids)
            .into_model::<Level>()
            .all(txn)
            .await?;
        if discovered.len() > MAX_DELETE_LEVELS {
            return Err(CatalogDeletionBlocker::LimitExceeded.into());
        }
        let location_ids = discovered
            .iter()
            .map(|row| row.location_id)
            .collect::<BTreeSet<_>>();
        if location_ids.iter().any(Uuid::is_nil) {
            return Err(CatalogDeletionBlocker::StateUnverified.into());
        }
        let locations = current_read(
            txn,
            stock_location::Entity::find()
                .select_only()
                .column(stock_location::Column::Id)
                .filter(stock_location::Column::TenantId.eq(tenant_id))
                .filter(stock_location::Column::Id.is_in(location_ids.iter().copied()))
                .order_by_asc(stock_location::Column::Id)
                .limit((MAX_DELETE_LEVELS + 1) as u64),
        )
        .into_tuple::<Uuid>()
        .all(txn)
        .await?;
        if locations != location_ids.iter().copied().collect::<Vec<_>>() {
            return Err(CatalogDeletionBlocker::StateUnverified.into());
        }
        // Do not filter deleted_at: removing zero bookkeeping never reopens a warehouse.
        let levels = current_read(txn, level_query(&item_ids))
            .into_model::<Level>()
            .all(txn)
            .await?;
        if levels.len() > MAX_DELETE_LEVELS {
            return Err(CatalogDeletionBlocker::LimitExceeded.into());
        }
        if !levels
            .iter()
            .map(Level::binding)
            .eq(discovered.iter().map(Level::binding))
        {
            return Err(CatalogDeletionBlocker::ConcurrentChange.into());
        }
        validate_levels(&levels, &item_ids, &location_ids)?;
        Ok(Self { items, levels })
    }

    async fn apply(
        self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), CatalogDeletionError> {
        // The full batch is proved before the first write. The caller must propagate an
        // error and roll back its enclosing transaction, including prior level deletions.
        // The owner source proof includes the original parent/item/level/location bindings.
        // Capture the full batch while those rows still exist; no source write occurs here.
        let mut source_deletions = Vec::with_capacity(self.items.len());
        for &(item_id, variant_id) in &self.items {
            source_deletions.push(
                crate::source_history::prepare_deletion(
                    txn,
                    tenant_id,
                    variant_id,
                    item_id,
                    Some(actor_id),
                )
                .await?,
            );
        }
        for level in self.levels {
            let deleted = inventory_level::Entity::delete_many()
                .filter(inventory_level::Column::Id.eq(level.id))
                .filter(inventory_level::Column::InventoryItemId.eq(level.inventory_item_id))
                .filter(inventory_level::Column::LocationId.eq(level.location_id))
                .filter(inventory_level::Column::StockedQuantity.eq(0))
                .filter(inventory_level::Column::ReservedQuantity.eq(0))
                .filter(inventory_level::Column::IncomingQuantity.eq(0))
                .exec(txn)
                .await?;
            if deleted.rows_affected != 1 {
                return Err(CatalogDeletionBlocker::ConcurrentChange.into());
            }
        }
        for (item_id, variant_id) in self.items {
            let deleted = inventory_item::Entity::delete_many()
                .filter(inventory_item::Column::Id.eq(item_id))
                .filter(inventory_item::Column::VariantId.eq(variant_id))
                .exec(txn)
                .await?;
            if deleted.rows_affected != 1 {
                return Err(CatalogDeletionBlocker::ConcurrentChange.into());
            }
        }
        for deletion in source_deletions {
            deletion.record_deleted().await?;
        }
        Ok(())
    }
}

fn level_query(item_ids: &BTreeSet<Uuid>) -> Select<inventory_level::Entity> {
    inventory_level::Entity::find()
        .select_only()
        .column(inventory_level::Column::Id)
        .column(inventory_level::Column::InventoryItemId)
        .column(inventory_level::Column::LocationId)
        .column(inventory_level::Column::StockedQuantity)
        .column(inventory_level::Column::ReservedQuantity)
        .column(inventory_level::Column::IncomingQuantity)
        .filter(inventory_level::Column::InventoryItemId.is_in(item_ids.iter().copied()))
        .order_by_asc(inventory_level::Column::InventoryItemId)
        .order_by_asc(inventory_level::Column::LocationId)
        .order_by_asc(inventory_level::Column::Id)
        .limit((MAX_DELETE_LEVELS + 1) as u64)
}

fn validate_levels(
    levels: &[Level],
    item_ids: &BTreeSet<Uuid>,
    location_ids: &BTreeSet<Uuid>,
) -> Result<(), CatalogDeletionError> {
    if levels.len() > MAX_DELETE_LEVELS {
        return Err(CatalogDeletionBlocker::LimitExceeded.into());
    }
    let mut ids = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    for level in levels {
        if level.id.is_nil()
            || level.inventory_item_id.is_nil()
            || level.location_id.is_nil()
            || !item_ids.contains(&level.inventory_item_id)
            || !location_ids.contains(&level.location_id)
            || !ids.insert(level.id)
            || !bindings.insert((level.inventory_item_id, level.location_id))
        {
            return Err(CatalogDeletionBlocker::StateUnverified.into());
        }
        if !level.is_empty() {
            return Err(CatalogDeletionBlocker::StockPresent.into());
        }
    }
    Ok(())
}

async fn has_history(
    txn: &DatabaseTransaction,
    variant_ids: &[Uuid],
    item_ids: &BTreeSet<Uuid>,
) -> Result<bool, CatalogDeletionError> {
    // INVARIANT: these parent UUIDs were tenant-proved and locked above. Any reference
    // protects them, even if a corrupt ledger tenant/Variant label disagrees. Project only
    // opaque primary keys, never foreign payloads; no row state/quantity/metadata filters.
    if current_read(
        txn,
        reservation_item::Entity::find()
            .select_only()
            .column(reservation_item::Column::Id)
            .filter(reservation_item::Column::InventoryItemId.is_in(item_ids.iter().copied()))
            .limit(1),
    )
    .into_tuple::<Uuid>()
    .one(txn)
    .await?
    .is_some()
    {
        return Ok(true);
    }
    if current_read(
        txn,
        ledger::Entity::find()
            .select_only()
            .column(ledger::Column::ReturnItemId)
            .filter(ledger::Column::InventoryItemId.is_in(item_ids.iter().copied()))
            .limit(1),
    )
    .into_tuple::<Uuid>()
    .one(txn)
    .await?
    .is_some()
    {
        return Ok(true);
    }
    if current_read(
        txn,
        proof::Entity::find()
            .select_only()
            .column(proof::Column::FulfillmentItemId)
            .filter(proof::Column::VariantId.is_in(variant_ids.iter().copied()))
            .limit(1),
    )
    .into_tuple::<Uuid>()
    .one(txn)
    .await?
    .is_some()
    {
        return Ok(true);
    }
    Ok(current_read(
        txn,
        proof::Entity::find()
            .select_only()
            .column(proof::Column::FulfillmentItemId)
            .filter(proof::Column::InventoryItemId.is_in(item_ids.iter().copied()))
            .limit(1),
    )
    .into_tuple::<Uuid>()
    .one(txn)
    .await?
    .is_some())
}

#[cfg(test)]
mod tests;
