//! One current-state quantity plan for native set/adjust and Product's existing native hook.

use std::collections::BTreeSet;

use chrono::Utc;
use rustok_api::{PortError, PortErrorKind};
use rustok_commerce_foundation::entities::{product, product_variant};
use rustok_core::error::Error as CoreError;
use rustok_events::{DomainEvent, ValidateEvent};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Select, Statement, sea_query::Expr,
};
use uuid::Uuid;

use super::bootstrap::BootstrapService;
use super::inventory::inventory_item_write_query;
use super::policy::inventory_policy_allows_backorder;
use crate::entities::{inventory_level, stock_location};

const MAX_QUANTITY_LEVELS: usize = 4096;

/// Finite owner refusals; no submitted quantity, saved payload or dependency text is public copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityWriteBlocker {
    InvalidRequest,
    LimitExceeded,
    IdentityUnavailable,
    StateUnverified,
    LocationUnavailable,
    QuantityUnrepresentable,
    InsufficientStock,
    ConcurrentChange,
    IsolationUnsupported,
    EventUnrepresentable,
    PublicationUnverified,
}

impl QuantityWriteBlocker {
    pub const ALL: [Self; 11] = [
        Self::InvalidRequest,
        Self::LimitExceeded,
        Self::IdentityUnavailable,
        Self::StateUnverified,
        Self::LocationUnavailable,
        Self::QuantityUnrepresentable,
        Self::InsufficientStock,
        Self::ConcurrentChange,
        Self::IsolationUnsupported,
        Self::EventUnrepresentable,
        Self::PublicationUnverified,
    ];

    pub const fn kind(self) -> PortErrorKind {
        match self {
            Self::InvalidRequest | Self::LimitExceeded => PortErrorKind::Validation,
            Self::LocationUnavailable | Self::InsufficientStock | Self::ConcurrentChange => {
                PortErrorKind::Conflict
            }
            Self::IdentityUnavailable
            | Self::StateUnverified
            | Self::QuantityUnrepresentable
            | Self::IsolationUnsupported
            | Self::EventUnrepresentable
            | Self::PublicationUnverified => PortErrorKind::InvariantViolation,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "inventory.quantity_invalid",
            Self::LimitExceeded => "inventory.quantity_limit",
            Self::IdentityUnavailable => "inventory.quantity_identity_unavailable",
            Self::StateUnverified => "inventory.quantity_state_unverified",
            Self::LocationUnavailable => "inventory.quantity_location_unavailable",
            Self::QuantityUnrepresentable => "inventory.quantity_unrepresentable",
            Self::InsufficientStock => "inventory.quantity_insufficient",
            Self::ConcurrentChange => "inventory.quantity_changed",
            Self::IsolationUnsupported => "inventory.quantity_isolation_unsupported",
            Self::EventUnrepresentable => "inventory.quantity_event_unrepresentable",
            Self::PublicationUnverified => "inventory.quantity_publication_unverified",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => {
                "Inventory quantity writes require nonnil tenant, actor and SKU identities and valid quantity input"
            }
            Self::LimitExceeded => {
                "Inventory quantity exceeds the complete level bound; reconcile through a bounded Inventory operation"
            }
            Self::IdentityUnavailable => {
                "Inventory quantity cannot prove the saved item and levels; reconcile without recreating stock identity"
            }
            Self::StateUnverified => {
                "Inventory quantity cannot prove all original location bindings and counters; reconcile without moving stock"
            }
            Self::LocationUnavailable => {
                "Inventory quantity needs an active saved owner-default level; configure it explicitly without relocating reservations"
            }
            Self::QuantityUnrepresentable => {
                "Inventory quantity cannot fit the exact storage or response range; reconcile without clamping counters"
            }
            Self::InsufficientStock => {
                "Inventory quantity would violate physical stock or deny-policy availability; reconcile stock and reservations first"
            }
            Self::ConcurrentChange => {
                "Inventory quantity state changed; recover the original command outcome before issuing a new stock instruction"
            }
            Self::IsolationUnsupported => {
                "Inventory quantity requires verified READ COMMITTED on PostgreSQL/MySQL and enabled current-transaction instrumentation on MySQL"
            }
            Self::EventUnrepresentable => {
                "Inventory quantity cannot be represented by the existing stock notification; reconcile without changing reservation history"
            }
            Self::PublicationUnverified => {
                "Inventory quantity notification could not be verified; preserve the original operation and reconcile its outcome"
            }
        }
    }

    pub fn port_error(self) -> PortError {
        PortError::new(self.kind(), self.code(), self.message(), false)
    }
}

impl std::fmt::Display for QuantityWriteBlocker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QuantityWriteError {
    #[error("{0}")]
    Rejected(QuantityWriteBlocker),
    #[error("Inventory quantity variant was not found")]
    VariantNotFound(Uuid),
    #[error("Inventory quantity storage is unavailable")]
    Database(#[from] DbErr),
    #[error("Inventory quantity publication could not be verified")]
    Publication(#[source] CoreError),
}

impl QuantityWriteError {
    /// Safe native policy. Advice is not an outcome/receipt or permission to retry an adjustment.
    pub fn port_error(&self) -> PortError {
        match self {
            Self::Rejected(blocker) => blocker.port_error(),
            Self::VariantNotFound(_) => {
                PortError::not_found("inventory.variant_not_found", "Inventory SKU was not found")
            }
            Self::Database(_) | Self::Publication(CoreError::Database(_)) => {
                PortError::unavailable(
                    "inventory.quantity_storage_unavailable",
                    "Inventory quantity storage is temporarily unavailable",
                )
            }
            Self::Publication(_) => QuantityWriteBlocker::PublicationUnverified.port_error(),
        }
    }
}

impl From<QuantityWriteBlocker> for QuantityWriteError {
    fn from(blocker: QuantityWriteBlocker) -> Self {
        Self::Rejected(blocker)
    }
}

type WriteResult<T> = Result<T, QuantityWriteError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    Set(i32),
    Adjust(i32),
}

pub(super) struct WriteOutcome {
    pub quantity: i32,
    pub inventory_policy: String,
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
}

#[derive(FromQueryResult)]
struct Location {
    id: Uuid,
    code: Option<String>,
    deleted_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
}

struct QuantityPlan {
    tenant_id: Uuid,
    actor_id: Uuid,
    variant_id: Uuid,
    product_id: Uuid,
    inventory_policy: String,
    before: i32,
    after: i32,
    target: Option<(Level, i32)>,
}

pub(super) async fn write_quantity(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
    variant_id: Uuid,
    change: Change,
    low_stock_threshold: Option<i32>,
) -> WriteResult<WriteOutcome> {
    validate_request(tenant_id, actor_id, variant_id)?;
    require_current_snapshot(txn).await?;
    let plan = QuantityPlan::load(txn, tenant_id, actor_id, variant_id, change).await?;
    plan.apply(txn, low_stock_threshold).await
}

pub(super) fn validate_request(
    tenant_id: Uuid,
    actor_id: Uuid,
    variant_id: Uuid,
) -> WriteResult<()> {
    if tenant_id.is_nil() || actor_id.is_nil() || variant_id.is_nil() {
        return Err(QuantityWriteBlocker::InvalidRequest.into());
    }
    Ok(())
}

async fn require_current_snapshot(txn: &DatabaseTransaction) -> WriteResult<()> {
    let backend = txn.get_database_backend();
    let statement = match backend {
        DbBackend::Postgres => "SHOW transaction_isolation",
        // A session default can differ from a one-shot override or the already-open transaction.
        // Require this connection's live explicit transaction; absent instrumentation fails closed.
        DbBackend::MySql => {
            "SELECT trx.ISOLATION_LEVEL AS transaction_isolation
             FROM performance_schema.events_transactions_current AS trx
             INNER JOIN performance_schema.threads AS thread
                 ON thread.THREAD_ID = trx.THREAD_ID
             WHERE thread.PROCESSLIST_ID = CONNECTION_ID()
                 AND trx.STATE = 'ACTIVE'
                 AND trx.END_EVENT_ID IS NULL
                 AND trx.AUTOCOMMIT = 'NO'"
        }
        DbBackend::Sqlite => return Ok(()),
        _ => {
            return Err(QuantityWriteError::Rejected(
                QuantityWriteBlocker::IsolationUnsupported,
            ));
        }
    };
    let row = txn
        .query_one_raw(Statement::from_string(backend, statement))
        .await?
        .ok_or(QuantityWriteBlocker::IsolationUnsupported)?;
    let isolation: String = row.try_get("", "transaction_isolation")?;
    if !isolation
        .replace('-', " ")
        .eq_ignore_ascii_case("read committed")
    {
        return Err(QuantityWriteBlocker::IsolationUnsupported.into());
    }
    Ok(())
}

fn current_read<E: EntityTrait>(txn: &DatabaseTransaction, query: Select<E>) -> Select<E> {
    if txn.get_database_backend() == DbBackend::Sqlite {
        query
    } else {
        query.lock_exclusive()
    }
}

impl QuantityPlan {
    async fn load(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        change: Change,
    ) -> WriteResult<Self> {
        let (product_id, inventory_policy) = current_read(
            txn,
            product_variant::Entity::find_by_id(variant_id)
                .filter(product_variant::Column::TenantId.eq(tenant_id))
                .select_only()
                .column(product_variant::Column::ProductId)
                .column(product_variant::Column::InventoryPolicy),
        )
        .into_tuple::<(Uuid, String)>()
        .one(txn)
        .await?
        .ok_or(QuantityWriteError::VariantNotFound(variant_id))?;
        if product_id.is_nil() {
            return Err(QuantityWriteBlocker::IdentityUnavailable.into());
        }
        // A nonlocking parent proof preserves Product's parent-before-variant lock order.
        let parent = product::Entity::find_by_id(product_id)
            .select_only()
            .column(product::Column::Id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .into_tuple::<Uuid>()
            .one(txn)
            .await?;
        if parent != Some(product_id) {
            return Err(QuantityWriteBlocker::IdentityUnavailable.into());
        }
        // INVARIANT: Variant/parent tenant identity precedes every tenant-less item query.
        let items = inventory_item_write_query(txn, variant_id)
            .limit(2)
            .all(txn)
            .await?;
        if items.len() != 1 || items[0].id.is_nil() || items[0].variant_id != variant_id {
            return Err(QuantityWriteBlocker::IdentityUnavailable.into());
        }
        let item_id = items[0].id;
        let discovered = level_query(item_id).into_model::<Level>().all(txn).await?;
        if discovered.len() > MAX_QUANTITY_LEVELS {
            return Err(QuantityWriteBlocker::LimitExceeded.into());
        }
        if discovered.is_empty() {
            return Err(QuantityWriteBlocker::IdentityUnavailable.into());
        }

        // Read-only owner selection. Never create a warehouse or select another UUID after locks.
        let candidate = BootstrapService::find_default_location_in_tx(txn, tenant_id).await?;
        let original_locations = discovered
            .iter()
            .map(|level| level.location_id)
            .collect::<BTreeSet<_>>();
        let mut location_ids = original_locations.clone();
        if let Some(candidate) = &candidate {
            location_ids.insert(candidate.id);
        }
        if location_ids.iter().any(Uuid::is_nil) {
            return Err(QuantityWriteBlocker::StateUnverified.into());
        }
        let locations = current_read(
            txn,
            stock_location::Entity::find()
                .select_only()
                .column(stock_location::Column::Id)
                .column(stock_location::Column::Code)
                .column(stock_location::Column::DeletedAt)
                .filter(stock_location::Column::TenantId.eq(tenant_id))
                .filter(stock_location::Column::Id.is_in(location_ids.iter().copied()))
                .order_by_asc(stock_location::Column::Id)
                .limit((MAX_QUANTITY_LEVELS + 2) as u64),
        )
        .into_model::<Location>()
        .all(txn)
        .await?;
        if locations
            .iter()
            .map(|location| location.id)
            .collect::<BTreeSet<_>>()
            != location_ids
        {
            return Err(QuantityWriteBlocker::StateUnverified.into());
        }
        let levels = current_read(txn, level_query(item_id))
            .into_model::<Level>()
            .all(txn)
            .await?;
        if !levels
            .iter()
            .map(Level::binding)
            .eq(discovered.iter().map(Level::binding))
        {
            return Err(QuantityWriteBlocker::ConcurrentChange.into());
        }
        let before = available_quantity(&levels, item_id, &original_locations)?;
        let after = target_quantity(before, change)?;
        let mut plan = Self {
            tenant_id,
            actor_id,
            variant_id,
            product_id,
            inventory_policy,
            before,
            after,
            target: None,
        };
        if before == after {
            return Ok(plan);
        }

        let candidate = candidate.ok_or(QuantityWriteBlocker::LocationUnavailable)?;
        let current_location = locations
            .iter()
            .find(|location| location.id == candidate.id)
            .ok_or(QuantityWriteBlocker::StateUnverified)?;
        if current_location.deleted_at.is_some() || current_location.code != candidate.code {
            return Err(QuantityWriteBlocker::LocationUnavailable.into());
        }
        let target = levels
            .into_iter()
            .find(|level| level.location_id == candidate.id)
            .ok_or(QuantityWriteBlocker::LocationUnavailable)?;
        let stocked = stocked_target(
            before,
            after,
            target.stocked_quantity,
            target.reserved_quantity,
            inventory_policy_allows_backorder(&plan.inventory_policy),
        )?;
        validate_event_quantities(before, after)?;
        plan.target = Some((target, stocked));
        Ok(plan)
    }

    async fn apply(
        self,
        txn: &DatabaseTransaction,
        low_stock_threshold: Option<i32>,
    ) -> WriteResult<WriteOutcome> {
        if let Some((level, stocked)) = self.target {
            let event = DomainEvent::InventoryUpdated {
                variant_id: self.variant_id,
                product_id: self.product_id,
                location_id: level.location_id,
                old_quantity: self.before,
                new_quantity: self.after,
            };
            event
                .validate()
                .map_err(|_| QuantityWriteBlocker::EventUnrepresentable)?;
            let low_event = low_stock_threshold
                .filter(|threshold| self.after > 0 && self.after < *threshold)
                .map(|threshold| DomainEvent::InventoryLow {
                    variant_id: self.variant_id,
                    product_id: self.product_id,
                    remaining: self.after,
                    threshold,
                });
            if let Some(event) = &low_event {
                event
                    .validate()
                    .map_err(|_| QuantityWriteBlocker::EventUnrepresentable)?;
            }
            let updated = inventory_level::Entity::update_many()
                .col_expr(
                    inventory_level::Column::StockedQuantity,
                    Expr::value(stocked),
                )
                .col_expr(inventory_level::Column::UpdatedAt, Expr::value(Utc::now()))
                .filter(inventory_level::Column::Id.eq(level.id))
                .filter(inventory_level::Column::InventoryItemId.eq(level.inventory_item_id))
                .filter(inventory_level::Column::LocationId.eq(level.location_id))
                .filter(inventory_level::Column::StockedQuantity.eq(level.stocked_quantity))
                .filter(inventory_level::Column::ReservedQuantity.eq(level.reserved_quantity))
                .filter(inventory_level::Column::IncomingQuantity.eq(level.incoming_quantity))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(QuantityWriteBlocker::ConcurrentChange.into());
            }
            crate::source_history::record_changed(
                txn,
                self.tenant_id,
                self.variant_id,
                Some(self.actor_id),
            )
            .await?;
            TransactionalEventBus::publish_root_in_tx(
                txn,
                self.tenant_id,
                Some(self.actor_id),
                event,
            )
            .await
            .map_err(QuantityWriteError::Publication)?;
            if let Some(event) = low_event {
                TransactionalEventBus::publish_root_in_tx(
                    txn,
                    self.tenant_id,
                    Some(self.actor_id),
                    event,
                )
                .await
                .map_err(QuantityWriteError::Publication)?;
            }
        }
        Ok(WriteOutcome {
            quantity: self.after,
            inventory_policy: self.inventory_policy,
        })
    }
}

fn level_query(item_id: Uuid) -> Select<inventory_level::Entity> {
    inventory_level::Entity::find()
        .select_only()
        .column(inventory_level::Column::Id)
        .column(inventory_level::Column::InventoryItemId)
        .column(inventory_level::Column::LocationId)
        .column(inventory_level::Column::StockedQuantity)
        .column(inventory_level::Column::ReservedQuantity)
        .column(inventory_level::Column::IncomingQuantity)
        .filter(inventory_level::Column::InventoryItemId.eq(item_id))
        .order_by_asc(inventory_level::Column::InventoryItemId)
        .order_by_asc(inventory_level::Column::LocationId)
        .order_by_asc(inventory_level::Column::Id)
        .limit((MAX_QUANTITY_LEVELS + 1) as u64)
}

fn available_quantity(
    levels: &[Level],
    item_id: Uuid,
    location_ids: &BTreeSet<Uuid>,
) -> WriteResult<i32> {
    if levels.len() > MAX_QUANTITY_LEVELS {
        return Err(QuantityWriteBlocker::LimitExceeded.into());
    }
    if levels.is_empty() {
        return Err(QuantityWriteBlocker::IdentityUnavailable.into());
    }
    let mut ids = BTreeSet::new();
    let mut locations = BTreeSet::new();
    let mut total = 0_i64;
    for level in levels {
        if level.id.is_nil()
            || level.inventory_item_id != item_id
            || item_id.is_nil()
            || level.location_id.is_nil()
            || !location_ids.contains(&level.location_id)
            || !ids.insert(level.id)
            || !locations.insert(level.location_id)
            || level.stocked_quantity < 0
            || level.reserved_quantity < 0
            || level.incoming_quantity < 0
        {
            return Err(QuantityWriteBlocker::StateUnverified.into());
        }
        total = total
            .checked_add(i64::from(level.stocked_quantity) - i64::from(level.reserved_quantity))
            .ok_or(QuantityWriteBlocker::QuantityUnrepresentable)?;
    }
    i32::try_from(total).map_err(|_| QuantityWriteBlocker::QuantityUnrepresentable.into())
}

/// Shared projection arithmetic, including the Product response after this writer commits.
/// Visibility/tenant selection belongs to the calling owner projection; absence is not inferred.
pub(super) fn available_by_variant<'a>(
    item_to_variant: &std::collections::HashMap<Uuid, Uuid>,
    levels: impl IntoIterator<Item = &'a inventory_level::Model>,
) -> Result<std::collections::HashMap<Uuid, i32>, DbErr> {
    let mut quantities = std::collections::HashMap::<Uuid, i64>::new();
    for level in levels {
        let variant_id = item_to_variant
            .get(&level.inventory_item_id)
            .ok_or_else(|| DbErr::Custom("Inventory availability binding is unverified".into()))?;
        if level.stocked_quantity < 0 || level.reserved_quantity < 0 || level.incoming_quantity < 0
        {
            return Err(DbErr::Custom(
                "Inventory availability counters are unverified".into(),
            ));
        }
        let total = quantities.entry(*variant_id).or_default();
        *total = total
            .checked_add(i64::from(level.stocked_quantity) - i64::from(level.reserved_quantity))
            .ok_or_else(|| {
                DbErr::Custom("Inventory availability exceeds the supported range".into())
            })?;
    }
    quantities
        .into_iter()
        .map(|(variant, quantity)| {
            i32::try_from(quantity)
                .map(|quantity| (variant, quantity))
                .map_err(|_| {
                    DbErr::Custom("Inventory availability exceeds the signed response range".into())
                })
        })
        .collect()
}

fn target_quantity(before: i32, change: Change) -> WriteResult<i32> {
    match change {
        Change::Set(quantity) => Ok(quantity),
        Change::Adjust(adjustment) => before
            .checked_add(adjustment)
            .ok_or_else(|| QuantityWriteBlocker::QuantityUnrepresentable.into()),
    }
}

pub(super) fn stocked_target(
    before: i32,
    after: i32,
    stocked: i32,
    reserved: i32,
    backorder: bool,
) -> WriteResult<i32> {
    if stocked < 0 || reserved < 0 {
        return Err(QuantityWriteBlocker::StateUnverified.into());
    }
    let delta = i64::from(after) - i64::from(before);
    let target = i64::from(stocked)
        .checked_add(delta)
        .ok_or(QuantityWriteBlocker::QuantityUnrepresentable)?;
    if target < 0 || (!backorder && (after < 0 || target - i64::from(reserved) < 0)) {
        return Err(QuantityWriteBlocker::InsufficientStock.into());
    }
    i32::try_from(target).map_err(|_| QuantityWriteBlocker::QuantityUnrepresentable.into())
}

fn validate_event_quantities(before: i32, after: i32) -> WriteResult<()> {
    if before < 0 || after < 0 {
        return Err(QuantityWriteBlocker::EventUnrepresentable.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
