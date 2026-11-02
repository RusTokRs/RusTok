//! Inventory-owned, quantity-free reservation invalidation on the existing owner transaction.
//!
//! Exact identity replay, zero quantity and adoption never prepare this plan. It is not a
//! second receipt, a Product refresh ledger entry, or permission to retry a quantity command.

use rustok_api::{PortCallPolicy, PortContext};
use rustok_commerce_foundation::entities::{product, product_variant};
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_events::InventoryReservationEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::ports_impl::parse_port_tenant_id;
use crate::stock_events::event_actor_id;

/// Native quantity writers require the same explicit current context as their port callers.
/// Admission metadata remains diagnostic: these commands still have no durable result receipt.
pub(crate) fn quantity_tenant(
    context: &PortContext,
    operation: &'static str,
) -> CommerceResult<Uuid> {
    context.require_policy(PortCallPolicy::write()).map_err(|_| {
        CommerceError::Validation(
            "Inventory quantity writes require current tenant context, positive deadline and a nonempty diagnostic key; the key does not make this command replay-safe".to_owned(),
        )
    })?;
    parse_port_tenant_id(context, operation).map_err(|_| {
        CommerceError::Validation(
            "Inventory quantity writes require a valid current tenant".to_owned(),
        )
    })
}

pub(crate) struct ReservationEventPlan {
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    event: InventoryReservationEvent,
}

impl ReservationEventPlan {
    /// Called only on a new movement after replay/no-op decisions, before counter effects.
    pub(crate) async fn prepare<C>(
        conn: &C,
        context: &PortContext,
        operation: &'static str,
        variant: &product_variant::Model,
    ) -> CommerceResult<Self>
    where
        C: ConnectionTrait,
    {
        let tenant_id = quantity_tenant(context, operation)?;
        let actor_id = event_actor_id(context).map_err(|_| {
            CommerceError::Validation(
                "Inventory reservation notifications require the current human actor's non-nil UUID; do not reconstruct an actor from saved metadata".to_owned(),
            )
        })?;
        if variant.tenant_id != tenant_id || variant.id.is_nil() || variant.product_id.is_nil() {
            return Err(scope_unverified());
        }
        if product::Entity::find_by_id(variant.product_id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .is_none()
        {
            return Err(scope_unverified());
        }
        Ok(Self {
            tenant_id,
            actor_id,
            event: InventoryReservationEvent::Changed {
                product_id: variant.product_id,
                variant_id: variant.id,
            },
        })
    }

    /// Invoke after exact counter/receipt effects and checked result, before the same commit.
    pub(crate) async fn publish(self, txn: &DatabaseTransaction) -> CommerceResult<()> {
        let InventoryReservationEvent::Changed { variant_id, .. } = &self.event;
        crate::source_history::record_changed(txn, self.tenant_id, *variant_id, self.actor_id)
            .await?;
        TransactionalEventBus::publish_contract_direct_in_tx(
            txn,
            self.tenant_id,
            self.actor_id,
            self.event,
        )
        .await?;
        Ok(())
    }
}

fn scope_unverified() -> CommerceError {
    CommerceError::Validation(
        "Inventory reservation notification Product/Variant ownership is unverified; reconcile the original identity without substitution".to_owned(),
    )
}
