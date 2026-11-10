use std::collections::BTreeSet;

use rustok_api::sha256_digest;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use super::{InventorySourceError as E, MAX_SOURCE_LEVELS};
use crate::entities::{inventory_item, inventory_level, stock_location};

pub(super) struct Snapshot {
    pub variant_id: Uuid,
    pub product_id: Uuid,
    pub item_id: Uuid,
    pub stocked: i64,
    pub reserved: i64,
    pub incoming: i64,
    pub binding_digest: String,
    pub level_count: usize,
}

#[derive(FromQueryResult)]
struct Item {
    id: Uuid,
    variant_id: Uuid,
}

#[derive(Clone, FromQueryResult)]
pub(super) struct Level {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub stocked_quantity: i32,
    pub reserved_quantity: i32,
    pub incoming_quantity: i32,
}

pub(super) async fn load<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
    variant: Uuid,
) -> Result<Snapshot, E> {
    load_with_budget(conn, tenant, variant, MAX_SOURCE_LEVELS).await
}

pub(super) async fn load_with_budget<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
    variant: Uuid,
    allowance: usize,
) -> Result<Snapshot, E> {
    if tenant.is_nil() || variant.is_nil() {
        return Err(E::StateUnverified);
    }
    if allowance == 0 || allowance > MAX_SOURCE_LEVELS {
        return Err(E::LimitExceeded);
    }
    // Narrow, current structural proof only. Never acquire a Product lock after child locks.
    let info = crate::product_port::get_variant_info_from_conn(conn, tenant, variant)
        .await
        .map_err(|_| E::StateUnverified)?;
    if info.product_id.is_nil() {
        return Err(E::StateUnverified);
    }
    let items = inventory_item::Entity::find()
        .select_only()
        .column(inventory_item::Column::Id)
        .column(inventory_item::Column::VariantId)
        .filter(inventory_item::Column::VariantId.eq(variant))
        .limit(2)
        .into_model::<Item>()
        .all(conn)
        .await?;
    if items.len() != 1 || items[0].id.is_nil() || items[0].variant_id != variant {
        return Err(E::StateUnverified);
    }
    let item = items[0].id;
    let levels = inventory_level::Entity::find()
        .select_only()
        .column(inventory_level::Column::Id)
        .column(inventory_level::Column::InventoryItemId)
        .column(inventory_level::Column::LocationId)
        .column(inventory_level::Column::StockedQuantity)
        .column(inventory_level::Column::ReservedQuantity)
        .column(inventory_level::Column::IncomingQuantity)
        .filter(inventory_level::Column::InventoryItemId.eq(item))
        .order_by_asc(inventory_level::Column::Id)
        .limit((allowance + 1) as u64)
        .into_model::<Level>()
        .all(conn)
        .await?;
    // Refuse the overflow sentinel before any location query. Baseline pages consume a
    // single total level allowance, so rejected work is also bounded by 4096 plus one row.
    if levels.len() > allowance {
        return Err(E::LimitExceeded);
    }
    let locations: BTreeSet<_> = levels.iter().map(|level| level.location_id).collect();
    let proved = stock_location::Entity::find()
        .select_only()
        .column(stock_location::Column::Id)
        .filter(stock_location::Column::TenantId.eq(tenant))
        .filter(stock_location::Column::Id.is_in(locations.iter().copied()))
        .limit((MAX_SOURCE_LEVELS + 1) as u64)
        .into_tuple::<Uuid>()
        .all(conn)
        .await?;
    if proved.into_iter().collect::<BTreeSet<_>>() != locations {
        return Err(E::StateUnverified);
    }
    summarize(variant, parent, item, &levels)
}

pub(super) fn summarize(
    variant: Uuid,
    parent: Uuid,
    item: Uuid,
    levels: &[Level],
) -> Result<Snapshot, E> {
    if levels.len() > MAX_SOURCE_LEVELS {
        return Err(E::LimitExceeded);
    }
    if variant.is_nil() || parent.is_nil() || item.is_nil() || levels.is_empty() {
        return Err(E::StateUnverified);
    }
    let mut ids = BTreeSet::new();
    let mut locations = BTreeSet::new();
    let (mut stocked, mut reserved, mut incoming) = (0_i64, 0_i64, 0_i64);
    let mut bindings = Vec::with_capacity(levels.len());
    for level in levels {
        if level.id.is_nil()
            || level.location_id.is_nil()
            || level.inventory_item_id != item
            || !ids.insert(level.id)
            || !locations.insert(level.location_id)
            || level.stocked_quantity < 0
            || level.reserved_quantity < 0
            || level.incoming_quantity < 0
        {
            return Err(E::StateUnverified);
        }
        stocked = stocked
            .checked_add(i64::from(level.stocked_quantity))
            .ok_or(E::StateUnverified)?;
        reserved = reserved
            .checked_add(i64::from(level.reserved_quantity))
            .ok_or(E::StateUnverified)?;
        incoming = incoming
            .checked_add(i64::from(level.incoming_quantity))
            .ok_or(E::StateUnverified)?;
        bindings.push((level.id, level.location_id));
    }
    bindings.sort_unstable();
    let mut bytes = Vec::with_capacity(bindings.len() * 32);
    for (level, location) in bindings {
        bytes.extend_from_slice(level.as_bytes());
        bytes.extend_from_slice(location.as_bytes());
    }
    let binding_digest = sha256_digest(&[b"inventory-source-level-bindings", &bytes])
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(Snapshot {
        variant_id: variant,
        product_id: parent,
        item_id: item,
        stocked,
        reserved,
        incoming,
        binding_digest,
        level_count: levels.len(),
    })
}
