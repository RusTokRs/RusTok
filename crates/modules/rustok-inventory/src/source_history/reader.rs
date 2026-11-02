use std::collections::{BTreeMap, BTreeSet};

use rustok_api::PortContext;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use super::{
    InventorySourceError as E, InventorySourceHistory, MAX_SOURCE_PAGE,
    model::{change, record},
};

/// Owner projection only: no Product publication/policy/visibility claim or Index mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventorySourceRecord {
    pub tenant_id: Uuid,
    pub baseline_id: Uuid,
    pub variant_id: Uuid,
    pub product_id: Uuid,
    pub inventory_item_id: Uuid,
    pub source_version: u64,
    pub deleted: bool,
    pub stocked: i64,
    pub reserved: i64,
    pub incoming: i64,
    pub available: i64,
    pub change_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventorySourcePage {
    pub baseline_id: Uuid,
    pub records: Vec<InventorySourceRecord>,
    pub next_after: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventorySourceChange {
    pub change_id: Uuid,
    pub tenant_id: Uuid,
    pub baseline_id: Uuid,
    pub variant_id: Uuid,
    pub source_version: u64,
    pub kind: String,
}

impl InventorySourceHistory {
    pub async fn load(
        &self,
        context: &PortContext,
        baseline_id: Uuid,
        variants: &[Uuid],
    ) -> Result<Vec<InventorySourceRecord>, E> {
        let (tenant, _) = super::scope(context, false)?;
        page_limit(variants.len())?;
        let unique: BTreeSet<_> = variants.iter().copied().collect();
        if unique.len() != variants.len() || unique.iter().any(Uuid::is_nil) {
            return Err(E::InvalidRequest);
        }
        super::read_budget(context, async {
            self.active(tenant, baseline_id).await?;
            let rows = record::Entity::find()
                .filter(record::Column::TenantId.eq(tenant))
                .filter(record::Column::VariantId.is_in(unique.iter().copied()))
                .all(&self.db)
                .await?;
            if rows.len() != unique.len() {
                return Err(E::StateUnverified);
            } // Missing is not absence.
            let records = checked_rows(&self.db, rows, tenant, baseline_id).await?;
            let mut by_id: BTreeMap<_, _> = records
                .into_iter()
                .map(|row| (row.variant_id, row))
                .collect();
            variants
                .iter()
                .map(|id| by_id.remove(id).ok_or(E::StateUnverified))
                .collect()
        })
        .await
    }

    /// Current owner records plus retained tombstones. Cursor belongs to this exact baseline.
    pub async fn scan(
        &self,
        context: &PortContext,
        baseline_id: Uuid,
        after: Option<Uuid>,
        limit: usize,
    ) -> Result<InventorySourcePage, E> {
        let (tenant, _) = super::scope(context, false)?;
        page_limit(limit)?;
        if after.is_some_and(|id| id.is_nil()) {
            return Err(E::InvalidRequest);
        }
        super::read_budget(context, async {
            self.active(tenant, baseline_id).await?;
            let mut query = record::Entity::find()
                .filter(record::Column::TenantId.eq(tenant))
                .order_by_asc(record::Column::VariantId)
                .limit((limit + 1) as u64);
            if let Some(after) = after {
                query = query.filter(record::Column::VariantId.gt(after));
            }
            let mut rows = query.all(&self.db).await?;
            let more = rows.len() > limit;
            rows.truncate(limit);
            let records = checked_rows(&self.db, rows, tenant, baseline_id).await?;
            let next_after = if more {
                Some(records.last().ok_or(E::StateUnverified)?.variant_id)
            } else {
                None
            };
            Ok(InventorySourcePage {
                baseline_id,
                records,
                next_after,
            })
        })
        .await
    }

    /// Per-SKU commits are serialized; no global sequence/commit-order watermark is promised.
    pub async fn changes(
        &self,
        context: &PortContext,
        baseline_id: Uuid,
        variant_id: Uuid,
        after_version: u64,
        limit: usize,
    ) -> Result<Vec<InventorySourceChange>, E> {
        let (tenant, _) = super::scope(context, false)?;
        page_limit(limit)?;
        if variant_id.is_nil() {
            return Err(E::InvalidRequest);
        }
        let after = i64::try_from(after_version).map_err(|_| E::InvalidRequest)?;
        super::read_budget(context, async {
            self.active(tenant, baseline_id).await?;
            let rows = change::Entity::find()
                .filter(change::Column::TenantId.eq(tenant))
                .filter(change::Column::VariantId.eq(variant_id))
                .filter(change::Column::SourceVersion.gt(after))
                .order_by_asc(change::Column::SourceVersion)
                .limit(limit as u64)
                .all(&self.db)
                .await?;
            rows.into_iter()
                .map(|row| {
                    validate_change(&row, tenant, baseline_id)?;
                    if row.variant_id != variant_id {
                        return Err(E::StateUnverified);
                    }
                    Ok(InventorySourceChange {
                        change_id: row.change_id,
                        tenant_id: tenant,
                        baseline_id,
                        variant_id,
                        source_version: row.source_version as u64,
                        kind: row.kind,
                    })
                })
                .collect()
        })
        .await
    }

    async fn active(&self, tenant: Uuid, baseline: Uuid) -> Result<(), E> {
        if baseline.is_nil() {
            return Err(E::InvalidRequest);
        }
        let saved = super::saved_baseline(&self.db, tenant)
            .await?
            .ok_or(E::BaselineUnavailable)?;
        if !saved.active {
            return Err(E::BaselineUnavailable);
        }
        if saved.baseline_id != baseline {
            return Err(E::Conflict);
        }
        Ok(())
    }
}

pub(super) fn page_limit(limit: usize) -> Result<(), E> {
    if !(1..=MAX_SOURCE_PAGE).contains(&limit) {
        return Err(E::LimitExceeded);
    }
    Ok(())
}

pub(super) fn validate_record(row: &record::Model, tenant: Uuid, baseline: Uuid) -> Result<(), E> {
    if tenant.is_nil()
        || baseline.is_nil()
        || row.tenant_id != tenant
        || row.baseline_id != baseline
        || row.variant_id.is_nil()
        || row.inventory_item_id.is_nil()
        || row.product_id.is_nil()
        || row.last_change_id.is_nil()
        || row.source_version <= 0
        || row.stocked < 0
        || row.reserved < 0
        || row.incoming < 0
        || row.binding_digest.len() != 64
        || !row.binding_digest.bytes().all(|c| c.is_ascii_hexdigit())
        || (row.deleted && (row.stocked != 0 || row.reserved != 0 || row.incoming != 0))
    {
        return Err(E::StateUnverified);
    }
    Ok(())
}

pub(super) fn validate_change(row: &change::Model, tenant: Uuid, baseline: Uuid) -> Result<(), E> {
    if row.tenant_id != tenant
        || row.baseline_id != baseline
        || row.change_id.is_nil()
        || row.variant_id.is_nil()
        || row.source_version <= 0
        || !matches!(
            row.kind.as_str(),
            "baseline" | "created" | "changed" | "deleted"
        )
    {
        return Err(E::StateUnverified);
    }
    Ok(())
}

async fn checked_rows<C: ConnectionTrait>(
    conn: &C,
    rows: Vec<record::Model>,
    tenant: Uuid,
    baseline: Uuid,
) -> Result<Vec<InventorySourceRecord>, E> {
    if rows.is_empty() {
        return Ok(Vec::new());
    } // Scan end only; load verifies every key first.
    for row in &rows {
        validate_record(row, tenant, baseline)?;
    }
    let heads = change::Entity::find()
        .filter(change::Column::TenantId.eq(tenant))
        .filter(change::Column::ChangeId.is_in(rows.iter().map(|row| row.last_change_id)))
        .all(conn)
        .await?;
    let mut heads: BTreeMap<_, _> = heads
        .into_iter()
        .map(|head| (head.change_id, head))
        .collect();
    rows.into_iter()
        .map(|row| {
            let head = heads
                .remove(&row.last_change_id)
                .ok_or(E::StateUnverified)?;
            validate_change(&head, tenant, baseline)?;
            if head.variant_id != row.variant_id
                || head.source_version != row.source_version
                || row.deleted != (head.kind == "deleted")
            {
                return Err(E::StateUnverified);
            }
            Ok(InventorySourceRecord {
                tenant_id: tenant,
                baseline_id: baseline,
                variant_id: row.variant_id,
                product_id: row.product_id,
                inventory_item_id: row.inventory_item_id,
                source_version: row.source_version as u64,
                deleted: row.deleted,
                stocked: row.stocked,
                reserved: row.reserved,
                incoming: row.incoming,
                available: row
                    .stocked
                    .checked_sub(row.reserved)
                    .ok_or(E::StateUnverified)?,
                change_id: row.last_change_id,
            })
        })
        .collect()
}
