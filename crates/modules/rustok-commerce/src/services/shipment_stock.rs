//! Compose saved Order identities with native Inventory effects; never read peer tables or
//! perform peer I/O under Fulfillment's transaction. An unavailable selected owner is not Noop.
use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{
    FulfillmentAdminCommandRuntime, FulfillmentService, PreparedShipmentStock, ShipmentStockAction,
    ShipmentStockError, ShipmentStockPort, ShipmentStockRequest, ShipmentStockScope,
};
use rustok_inventory::shipment_stock::{self as inventory, ShipmentStockBatch, ShipmentStockLine};
use rustok_order::{OrderReadPort, ReadOrderLineInventoryBindingsRequest};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct InventoryShipmentStockPort {
    db: DatabaseConnection,
    order_reads: Arc<dyn OrderReadPort>,
}

struct InventoryShipmentPlan {
    context: PortContext,
    started: Instant,
    action: inventory::ShipmentStockAction,
    batch: ShipmentStockBatch,
}

impl InventoryShipmentPlan {
    fn remaining_context(&self) -> Result<PortContext, ShipmentStockError> {
        let budget = self
            .context
            .deadline_ms
            .filter(|value| *value > 0)
            .ok_or_else(deadline_expired)?;
        let remaining = Duration::from_millis(budget)
            .checked_sub(self.started.elapsed())
            .filter(|value| value.as_millis() > 0)
            .ok_or_else(deadline_expired)?;
        Ok(self.context.clone().with_deadline(remaining))
    }
}

#[async_trait]
impl ShipmentStockPort for InventoryShipmentStockPort {
    async fn prepare(
        &self,
        context: &PortContext,
        scope: &ShipmentStockScope,
    ) -> Result<Arc<dyn PreparedShipmentStock>, ShipmentStockError> {
        context
            .require_policy(PortCallPolicy::write())
            .map_err(ShipmentStockError::Owner)?;
        let tenant_id = Uuid::parse_str(context.tenant_id.trim())
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(binding_unverified)?;
        if tenant_id != scope.tenant_id
            || scope.order_id.is_nil()
            || scope.fulfillment_id.is_nil()
            || scope.items.is_empty()
            || scope.items.len() > rustok_fulfillment::MAX_SHIPMENT_STOCK_ITEMS
        {
            return Err(binding_unverified());
        }
        let budget = context
            .deadline_ms
            .filter(|value| *value > 0)
            .ok_or_else(deadline_expired)?
            .min(30_000);
        let context = context.clone().with_deadline(Duration::from_millis(budget));
        let started = Instant::now();
        let mut expected = BTreeMap::<Uuid, i32>::new();
        let mut item_ids = BTreeSet::new();
        for item in &scope.items {
            if item.fulfillment_item_id.is_nil()
                || item.order_line_item_id.is_nil()
                || !item_ids.insert(item.fulfillment_item_id)
                || item.quantity <= 0
                || item.shipped_quantity < 0
                || item.delivered_quantity < 0
                || item.delivered_quantity > item.shipped_quantity
                || item.shipped_quantity > item.quantity
                || item.movement_quantity < 0
                || item.movement_quantity > item.quantity
            {
                return Err(binding_unverified());
            }
            let total = expected.entry(item.order_line_item_id).or_default();
            *total = total
                .checked_add(item.quantity)
                .ok_or_else(binding_unverified)?;
        }
        let snapshot = self
            .order_reads
            .read_order_line_inventory_bindings(
                context.clone(),
                ReadOrderLineInventoryBindingsRequest {
                    order_id: scope.order_id,
                    order_line_item_ids: expected.keys().copied().collect(),
                },
            )
            .await
            .map_err(ShipmentStockError::Owner)?;
        if snapshot.tenant_id != tenant_id
            || snapshot.order_id != scope.order_id
            || snapshot.items.len() != expected.len()
        {
            return Err(binding_unverified());
        }
        let mut bindings = BTreeMap::new();
        for binding in snapshot.items {
            let planned = expected
                .get(&binding.order_line_item_id)
                .ok_or_else(binding_unverified)?;
            if binding.quantity <= 0
                || binding.quantity < *planned
                || binding.variant_id.is_some_and(|id| id.is_nil())
            {
                return Err(binding_unverified());
            }
            if bindings
                .insert(binding.order_line_item_id, binding.variant_id)
                .is_some()
            {
                return Err(binding_unverified());
            }
        }
        if bindings.len() != expected.len() {
            return Err(binding_unverified());
        }
        let lines = scope
            .items
            .iter()
            .filter(|item| item.movement_quantity > 0)
            .map(|item| {
                Ok(ShipmentStockLine {
                    fulfillment_item_id: item.fulfillment_item_id,
                    order_line_item_id: item.order_line_item_id,
                    variant_id: *bindings
                        .get(&item.order_line_item_id)
                        .ok_or_else(binding_unverified)?,
                    fulfillment_quantity: item.quantity,
                    shipped_quantity: item.shipped_quantity,
                    delivered_quantity: item.delivered_quantity,
                    quantity: item.movement_quantity,
                })
            })
            .collect::<Result<Vec<_>, ShipmentStockError>>()?;
        if lines.is_empty() {
            return Err(binding_unverified());
        }
        let action = match scope.action {
            ShipmentStockAction::Ship => inventory::ShipmentStockAction::Ship,
            ShipmentStockAction::Restore => inventory::ShipmentStockAction::Restore,
            ShipmentStockAction::Reconsume => inventory::ShipmentStockAction::Reconsume,
        };
        let prepared = InventoryShipmentPlan {
            context,
            started,
            action,
            batch: ShipmentStockBatch {
                order_id: scope.order_id,
                fulfillment_id: scope.fulfillment_id,
                lines,
            },
        };
        // Known legacy-history/overlap refusals precede a fresh external effect. This is not
        // a stock-availability promise; native mutation repeats proof under deterministic locks.
        inventory::validate_shipment_stock_preparation(
            &self.db,
            &prepared.remaining_context()?,
            &prepared.batch,
            action,
        )
        .await
        .map_err(ShipmentStockError::Owner)?;
        Ok(Arc::new(prepared))
    }
}

#[async_trait]
impl PreparedShipmentStock for InventoryShipmentPlan {
    async fn apply(
        &self,
        txn: &DatabaseTransaction,
        requests: &[ShipmentStockRequest],
    ) -> Result<(), ShipmentStockError> {
        let expected = self
            .batch
            .lines
            .iter()
            .map(|line| {
                (
                    line.fulfillment_item_id,
                    (line.order_line_item_id, line.quantity),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut actual = BTreeMap::new();
        for request in requests {
            if request.quantity <= 0
                || actual
                    .insert(
                        request.fulfillment_item_id,
                        (request.order_line_item_id, request.quantity),
                    )
                    .is_some()
            {
                return Err(binding_unverified());
            }
        }
        if actual != expected {
            return Err(binding_unverified());
        }
        let context = self.remaining_context()?;
        match self.action {
            inventory::ShipmentStockAction::Ship => {
                inventory::consume_shipped_reservation(txn, &context, &self.batch).await
            }
            inventory::ShipmentStockAction::Restore => {
                inventory::restore_unshipped_stock(txn, &context, &self.batch).await
            }
            inventory::ShipmentStockAction::Reconsume => {
                inventory::reconsume_restored_stock(txn, &context, &self.batch).await
            }
        }
        .map_err(ShipmentStockError::Owner)
    }
}

fn binding_unverified() -> ShipmentStockError {
    ShipmentStockError::Owner(PortError::invariant_violation(
        "commerce.shipment_stock_binding_unverified",
        "saved shipment Order/SKU/quantity bindings are incomplete or contradictory; reconcile without fallback",
    ))
}
fn deadline_expired() -> ShipmentStockError {
    ShipmentStockError::Owner(PortError::timeout(
        "commerce.shipment_stock_deadline_expired",
        "prepared shipment stock budget has expired",
    ))
}

/// Selected Order capability is mandatory; construction never silently captures a native fallback.
pub fn inventory_shipment_stock_port(
    db: DatabaseConnection,
    order_reads: Arc<dyn OrderReadPort>,
) -> Arc<dyn ShipmentStockPort> {
    Arc::new(InventoryShipmentStockPort { db, order_reads })
}
pub fn fulfillment_service(
    db: DatabaseConnection,
    order_reads: Arc<dyn OrderReadPort>,
) -> FulfillmentService {
    FulfillmentService::new(db.clone())
        .with_shipment_stock_port(inventory_shipment_stock_port(db, order_reads))
}
pub fn fulfillment_admin_command_runtime(
    db: DatabaseConnection,
    provider_registry: FulfillmentProviderRegistry,
    order_reads: Arc<dyn OrderReadPort>,
) -> FulfillmentAdminCommandRuntime {
    FulfillmentAdminCommandRuntime::in_process_with_shipment_stock(
        db.clone(),
        provider_registry,
        inventory_shipment_stock_port(db, order_reads),
    )
}

#[cfg(test)]
/// Explicit test intent/actor. Production entrypoints must use the caller's canonical context.
pub(crate) fn test_shipment_context(tenant_id: uuid::Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::user(uuid::Uuid::new_v4().to_string()),
        "en",
        uuid::Uuid::new_v4().to_string(),
    )
    .with_idempotency_key(uuid::Uuid::new_v4().to_string())
    .with_deadline(std::time::Duration::from_secs(30))
}
