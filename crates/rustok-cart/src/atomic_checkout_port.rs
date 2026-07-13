use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    in_process_cart_checkout_snapshot_port, CartCheckoutContextUpdateRequest,
    CartCheckoutLifecycleRequest, CartCheckoutPort, CartCheckoutSnapshotPort,
    CartCheckoutSnapshotRequest, CartError, CartResponse, CartService,
    PrepareCartCheckoutSnapshotRequest,
};

/// Request-scoped adapter that preserves the existing `CartCheckoutPort`
/// protocol while deferring persistence until the atomic checkout claim.
///
/// `update_cart_checkout_context` returns an owner-generated preview only.
/// `begin_cart_checkout` applies the configured overlay, recalculates totals and
/// locks the cart in one transaction through `CartService::prepare_checkout`.
pub struct AtomicCartCheckoutPort {
    service: CartService,
    snapshot_port: Arc<dyn CartCheckoutSnapshotPort>,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
}

impl AtomicCartCheckoutPort {
    pub fn new(db: DatabaseConnection, prepare_request: PrepareCartCheckoutSnapshotRequest) -> Self {
        Self {
            service: CartService::new(db.clone()),
            snapshot_port: in_process_cart_checkout_snapshot_port(db),
            prepare_request,
        }
    }

    fn ensure_cart_id(&self, cart_id: Uuid) -> Result<(), PortError> {
        if cart_id == self.prepare_request.cart_id {
            Ok(())
        } else {
            Err(PortError::validation(
                "cart.checkout_adapter_cart_mismatch",
                format!(
                    "checkout adapter is bound to cart {}, not {cart_id}",
                    self.prepare_request.cart_id
                ),
            ))
        }
    }
}

pub fn in_process_atomic_cart_checkout_port(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
) -> Arc<dyn CartCheckoutPort> {
    Arc::new(AtomicCartCheckoutPort::new(db, prepare_request))
}

#[async_trait]
impl CartCheckoutPort for AtomicCartCheckoutPort {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.ensure_cart_id(request.cart_id)?;
        self.service
            .get_cart(parse_port_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartCheckoutContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        self.snapshot_port
            .prepare_checkout_snapshot(context, self.prepare_request.clone())
            .await
            .map(|snapshot| snapshot.cart)
    }

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        self.service
            .prepare_checkout(
                parse_port_tenant_id(&context)?,
                self.prepare_request.clone(),
            )
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        self.service
            .release_checkout(parse_port_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        self.service
            .complete_cart(parse_port_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "cart.invalid_tenant_id",
            "cart checkout requires a UUID tenant_id",
        )
    })
}

fn cart_error_to_port_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => {
            PortError::validation("cart.checkout_validation", message)
        }
        CartError::CartNotFound(cart_id) => {
            PortError::not_found("cart.not_found", format!("cart {cart_id} not found"))
        }
        CartError::CartLineItemNotFound(line_item_id) => PortError::not_found(
            "cart.line_item_not_found",
            format!("cart line item {line_item_id} not found"),
        ),
        CartError::InvalidTransition { from, to } => PortError::conflict(
            "cart.invalid_transition",
            format!("invalid cart status transition: {from} -> {to}"),
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.checkout_storage_unavailable",
            "cart checkout storage is unavailable",
        ),
        CartError::TaxBoundary {
            kind,
            code,
            message,
            retryable,
        } => PortError::new(kind, code, message, retryable),
    }
}
