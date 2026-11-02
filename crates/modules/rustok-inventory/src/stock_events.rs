//! Canonical outbox invalidations for native physical stock batches.
//!
//! The registered event carries descriptive global Variant availability, not location
//! deltas or a movement ledger. Consumers refetch owner state; equal counters still invalidate
//! after shipment changes stocked/reserved together. No broker or peer IO occurs here.

use std::collections::{BTreeMap, BTreeSet};

use rustok_api::{PortActorKind, PortContext, PortError};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement, Value};
use uuid::Uuid;

use crate::ports_impl::{parse_port_tenant_id, storage_unavailable_with_context};

const MAX_STOCK_EVENT_LEVELS: usize = 1024;

type LevelKey = (Uuid, Uuid);
type ItemBindings = BTreeMap<Uuid, (Uuid, Uuid)>;

/// Facts already proved/locked by the native caller, never fresh Catalog eligibility.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StockLevelChange {
    pub inventory_item_id: Uuid,
    pub variant_id: Uuid,
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub old_stocked: i32,
    pub old_reserved: i32,
    pub new_stocked: i32,
    pub new_reserved: i32,
}

impl StockLevelChange {
    fn key(self) -> LevelKey {
        (self.inventory_item_id, self.location_id)
    }

    fn available_delta(self) -> i64 {
        (i64::from(self.new_stocked) - i64::from(self.old_stocked))
            - (i64::from(self.new_reserved) - i64::from(self.old_reserved))
    }
}

/// One immutable invalidation per changed item/location; preparation never publishes it.
#[derive(Default)]
pub(crate) struct StockEventPlan {
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    changes: BTreeMap<LevelKey, StockLevelChange>,
    bindings: ItemBindings,
    before: BTreeMap<Uuid, i32>,
}

impl StockEventPlan {
    pub(crate) async fn prepare(
        txn: &DatabaseTransaction,
        context: &PortContext,
        operation: &'static str,
        changes: Vec<StockLevelChange>,
    ) -> Result<Self, PortError> {
        let changes = validate_changes(changes)?;
        if changes.is_empty() {
            // Explicit SKU-less/no physical movement must not acquire an outbox dependency.
            return Ok(Self::default());
        }
        let tenant_id = parse_port_tenant_id(context, operation)?;
        let actor_id = event_actor_id(context)?;
        let bindings = item_bindings(&changes)?;
        let before =
            load_available_quantities(txn, context, operation, tenant_id, &bindings).await?;
        validate_projected_quantities(&changes, &before)?;
        Ok(Self {
            tenant_id,
            actor_id,
            changes,
            bindings,
            before,
        })
    }

    pub(crate) async fn publish(
        self,
        txn: &DatabaseTransaction,
        context: &PortContext,
        operation: &'static str,
    ) -> Result<(), PortError> {
        if self.changes.is_empty() {
            return Ok(());
        }
        if parse_port_tenant_id(context, operation)? != self.tenant_id
            || event_actor_id(context)? != self.actor_id
        {
            return Err(identity_unverified());
        }
        // Read actual owner state after the writes, not a fabricated available-quantity delta.
        // These descriptive observations are not a lease over other locations or consumers.
        let after =
            load_available_quantities(txn, context, operation, self.tenant_id, &self.bindings)
                .await?;
        // Multiple changed locations still produce one source revision per SKU in this plan.
        for &(variant_id, _) in self.bindings.values() {
            crate::source_history::record_changed(txn, self.tenant_id, variant_id, self.actor_id)
                .await
                .map_err(|error| {
                    crate::ports_impl::storage_unavailable_with_context(context, operation, error)
                })?;
        }
        for change in self.changes.into_values() {
            let event = DomainEvent::InventoryUpdated {
                variant_id: change.variant_id,
                product_id: change.product_id,
                location_id: change.location_id,
                old_quantity: *self
                    .before
                    .get(&change.inventory_item_id)
                    .ok_or_else(identity_unverified)?,
                new_quantity: *after
                    .get(&change.inventory_item_id)
                    .ok_or_else(identity_unverified)?,
            };
            // The canonical bus validates the registered schema and writes sys_events on this
            // exact transaction. Missing outbox storage is a refusal, never an event-less success.
            TransactionalEventBus::publish_root_in_tx(txn, self.tenant_id, self.actor_id, event)
                .await
                .map_err(|_| {
                    tracing::error!(
                        operation,
                        code = "inventory.stock_event_persistence_unavailable",
                        "native stock and canonical outbox require same-identity recovery"
                    );
                    PortError::unavailable(
                        "inventory.stock_event_persistence_unavailable",
                        "stock notification persistence failed; recover the original mutation identity",
                    )
                })?;
        }
        Ok(())
    }
}

fn validate_changes(
    changes: Vec<StockLevelChange>,
) -> Result<BTreeMap<LevelKey, StockLevelChange>, PortError> {
    if changes.len() > MAX_STOCK_EVENT_LEVELS {
        return Err(identity_unverified());
    }
    let mut validated = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for change in changes {
        if [
            change.inventory_item_id,
            change.variant_id,
            change.product_id,
            change.location_id,
        ]
        .iter()
        .any(Uuid::is_nil)
            || !seen.insert(change.key())
            || [
                change.old_stocked,
                change.old_reserved,
                change.new_stocked,
                change.new_reserved,
            ]
            .iter()
            .any(|quantity| *quantity < 0)
        {
            return Err(identity_unverified());
        }
        if change.old_stocked == change.new_stocked && change.old_reserved == change.new_reserved {
            continue;
        }
        if validated.insert(change.key(), change).is_some() {
            return Err(identity_unverified());
        }
    }
    Ok(validated)
}

fn item_bindings(
    changes: &BTreeMap<LevelKey, StockLevelChange>,
) -> Result<ItemBindings, PortError> {
    let mut bindings = BTreeMap::new();
    let mut variant_items = BTreeMap::new();
    for change in changes.values() {
        let binding = (change.variant_id, change.product_id);
        if bindings
            .insert(change.inventory_item_id, binding)
            .is_some_and(|previous| previous != binding)
            || variant_items
                .insert(change.variant_id, change.inventory_item_id)
                .is_some_and(|previous| previous != change.inventory_item_id)
        {
            return Err(identity_unverified());
        }
    }
    Ok(bindings)
}

fn validate_projected_quantities(
    changes: &BTreeMap<LevelKey, StockLevelChange>,
    before: &BTreeMap<Uuid, i32>,
) -> Result<(), PortError> {
    let mut quantities: BTreeMap<Uuid, i64> = before
        .iter()
        .map(|(item_id, quantity)| (*item_id, i64::from(*quantity)))
        .collect();
    for change in changes.values() {
        let quantity = quantities
            .get_mut(&change.inventory_item_id)
            .ok_or_else(identity_unverified)?;
        *quantity = quantity
            .checked_add(change.available_delta())
            .ok_or_else(quantity_unrepresentable)?;
    }
    for quantity in quantities.into_values() {
        representable_quantity(quantity)?;
    }
    Ok(())
}

async fn load_available_quantities(
    txn: &DatabaseTransaction,
    context: &PortContext,
    operation: &'static str,
    tenant_id: Uuid,
    bindings: &ItemBindings,
) -> Result<BTreeMap<Uuid, i32>, PortError> {
    if bindings.is_empty() || bindings.len() > MAX_STOCK_EVENT_LEVELS {
        return Err(identity_unverified());
    }
    let backend = txn.get_database_backend();
    let placeholder = |index: usize| match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => "?".to_string(),
    };
    if !matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
        return Err(PortError::unavailable(
            "inventory.stock_event_backend_unsupported",
            "native stock notifications require PostgreSQL or SQLite",
        ));
    }
    // Exactly one aggregate row per already tenant-proved/locked item. No unbounded row
    // materialization, location eligibility refresh or cross-tenant location contribution.
    let placeholders = (0..bindings.len())
        .map(|index| placeholder(index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values: Vec<Value> = vec![tenant_id.into()];
    values.extend(bindings.keys().map(|id| Value::from(*id)));
    let rows = txn
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT il.inventory_item_id, SUM(il.stocked_quantity) AS stocked, \
                 SUM(il.reserved_quantity) AS reserved, \
                 SUM(CASE WHEN l.tenant_id = {} THEN 0 ELSE 1 END) AS foreign_locations \
                 FROM inventory_levels il LEFT JOIN stock_locations l ON l.id = il.location_id \
                 WHERE il.inventory_item_id IN ({placeholders}) GROUP BY il.inventory_item_id",
                placeholder(1),
            ),
            values,
        ))
        .await
        .map_err(|error| storage_unavailable_with_context(context, operation, error))?;
    let mut quantities = BTreeMap::new();
    for row in rows {
        let item_id: Uuid = row
            .try_get("", "inventory_item_id")
            .map_err(|error| storage_unavailable_with_context(context, operation, error))?;
        let stocked: i64 = row
            .try_get("", "stocked")
            .map_err(|error| storage_unavailable_with_context(context, operation, error))?;
        let reserved: i64 = row
            .try_get("", "reserved")
            .map_err(|error| storage_unavailable_with_context(context, operation, error))?;
        let foreign_locations: i64 = row
            .try_get("", "foreign_locations")
            .map_err(|error| storage_unavailable_with_context(context, operation, error))?;
        if !bindings.contains_key(&item_id) || foreign_locations != 0 || stocked < 0 || reserved < 0
        {
            return Err(identity_unverified());
        }
        let available = stocked
            .checked_sub(reserved)
            .ok_or_else(quantity_unrepresentable)?;
        if quantities
            .insert(item_id, representable_quantity(available)?)
            .is_some()
        {
            return Err(identity_unverified());
        }
    }
    if quantities.len() != bindings.len() {
        return Err(identity_unverified());
    }
    Ok(quantities)
}

fn representable_quantity(quantity: i64) -> Result<i32, PortError> {
    i32::try_from(quantity)
        .ok()
        .filter(|quantity| *quantity >= 0)
        .ok_or_else(quantity_unrepresentable)
}

pub(crate) fn event_actor_id(context: &PortContext) -> Result<Option<Uuid>, PortError> {
    match &context.actor.kind {
        PortActorKind::User => Uuid::parse_str(&context.actor.id)
            .ok()
            .filter(|id| !id.is_nil())
            .map(Some)
            .ok_or_else(|| {
                PortError::validation(
                    "inventory.stock_event_actor_invalid",
                    "stock notification requires the current human actor's non-nil UUID",
                )
            }),
        PortActorKind::Service | PortActorKind::System => Ok(None),
    }
}

fn identity_unverified() -> PortError {
    PortError::invariant_violation(
        "inventory.stock_event_scope_unverified",
        "native stock notification identity or location ownership requires reconciliation",
    )
}

fn quantity_unrepresentable() -> PortError {
    PortError::invariant_violation(
        "inventory.stock_event_quantity_unrepresentable",
        "global availability cannot be represented by the canonical stock event; reconcile without changing mutation identity",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    fn change() -> StockLevelChange {
        StockLevelChange {
            inventory_item_id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            location_id: Uuid::new_v4(),
            old_stocked: 5,
            old_reserved: 3,
            new_stocked: 3,
            new_reserved: 1,
        }
    }

    #[test]
    fn physical_shipment_with_equal_available_counters_still_invalidates() {
        let change = change();
        let changes = validate_changes(vec![change]).expect("physical changes");
        assert_eq!(changes.len(), 1);
        assert_eq!(change.available_delta(), 0);
        let mut unchanged = change;
        unchanged.new_stocked = unchanged.old_stocked;
        unchanged.new_reserved = unchanged.old_reserved;
        assert!(
            validate_changes(vec![unchanged])
                .expect("no physical movement")
                .is_empty()
        );
        assert!(validate_changes(vec![unchanged, change]).is_err());
        validate_projected_quantities(&changes, &BTreeMap::from([(change.inventory_item_id, 2)]))
            .expect("equal available snapshots remain valid");
    }

    #[test]
    fn changed_locations_share_one_variant_binding_and_grouped_global_quantity() {
        let mut first = change();
        first.new_stocked = 7;
        first.new_reserved = first.old_reserved;
        let mut second = first;
        second.location_id = Uuid::new_v4();
        second.new_stocked = 6;
        let changes = validate_changes(vec![first, second]).expect("two changed locations");
        assert_eq!(changes.len(), 2);
        assert_eq!(item_bindings(&changes).expect("one saved binding").len(), 1);
        validate_projected_quantities(&changes, &BTreeMap::from([(first.inventory_item_id, 4)]))
            .expect("global quantity seven is representable");
        // Either location delta alone fits; the entire batch's combined +3 does not.
        assert!(
            validate_projected_quantities(
                &changes,
                &BTreeMap::from([(first.inventory_item_id, i32::MAX - 2)]),
            )
            .is_err()
        );
    }

    #[test]
    fn duplicate_level_and_contradictory_variant_binding_are_refused() {
        let first = change();
        assert!(validate_changes(vec![first, first]).is_err());
        let mut second = first;
        second.location_id = Uuid::new_v4();
        second.product_id = Uuid::new_v4();
        let changes = validate_changes(vec![first, second]).expect("distinct locations");
        assert!(item_bindings(&changes).is_err());
    }

    #[test]
    fn unrepresentable_global_quantity_refuses_instead_of_saturation() {
        assert_eq!(representable_quantity(0).expect("zero"), 0);
        assert_eq!(
            representable_quantity(i64::from(i32::MAX)).expect("maximum"),
            i32::MAX
        );
        for quantity in [-1, i64::from(i32::MAX) + 1] {
            let error = representable_quantity(quantity).expect_err("unrepresentable counter");
            assert_eq!(error.code, "inventory.stock_event_quantity_unrepresentable");
            assert!(!error.retryable);
        }
        let mut change = change();
        change.new_stocked = 6;
        change.new_reserved = change.old_reserved;
        let changes = validate_changes(vec![change]).expect("one stocked addition");
        assert!(
            validate_projected_quantities(
                &changes,
                &BTreeMap::from([(change.inventory_item_id, i32::MAX)]),
            )
            .is_err()
        );
    }

    #[test]
    fn current_human_actor_is_preserved_without_fabricating_service_users() {
        let mut context = PortContext::new("tenant", PortActor::system(), "en", "correlation");
        assert_eq!(event_actor_id(&context).expect("system"), None);
        context.actor = PortActor::service("stock-worker");
        assert_eq!(event_actor_id(&context).expect("service"), None);
        let actor_id = Uuid::new_v4();
        context.actor = PortActor::user(actor_id.to_string());
        assert_eq!(event_actor_id(&context).expect("human"), Some(actor_id));
        context.actor = PortActor::user(Uuid::nil().to_string());
        assert_eq!(
            event_actor_id(&context).expect_err("nil actor").code,
            "inventory.stock_event_actor_invalid"
        );
    }
}
