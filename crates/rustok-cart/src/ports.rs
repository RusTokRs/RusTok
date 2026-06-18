use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CartError, CartResponse};

/// Transport-neutral owner boundary for cart checkout snapshots.
#[async_trait]
pub trait CartSnapshotReadPort: Send + Sync {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CartCheckoutSnapshotRequest {
    pub cart_id: Uuid,
    pub locale: Option<String>,
}

#[async_trait]
impl CartSnapshotReadPort for crate::CartService {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.get_cart(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "cart.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for cart ports",
        )
    })
}

fn cart_error_to_port_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => PortError::validation("cart.validation", message),
        CartError::CartNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "cart.cart_not_found",
            format!("cart {id} not found"),
            false,
        ),
        CartError::CartLineItemNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "cart.line_item_not_found",
            format!("cart line item {id} not found"),
            false,
        ),
        CartError::InvalidTransition { from, to } => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "cart.invalid_transition",
            format!("invalid cart status transition: {from} -> {to}"),
            false,
        ),
        CartError::Database(error) => PortError::unavailable(
            "cart.database_unavailable",
            format!("cart storage unavailable: {error}"),
        ),
        CartError::Tax(error) => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "cart.tax_invariant_violation",
            format!("cart tax calculation failed: {error}"),
            false,
        ),
    }
}
