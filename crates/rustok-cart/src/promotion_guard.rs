use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;

use crate::ports::{CartPromotionPort, CartPromotionRequest};
use crate::{CartPromotionPreview, CartResponse};

const READ_CART_PROMOTION_PREVIEW_OPERATION: &str = "read_cart_promotion_preview";
const APPLY_CART_PROMOTION_OPERATION: &str = "apply_cart_promotion";

pub fn guarded_cart_promotion_port(db: DatabaseConnection) -> Arc<dyn CartPromotionPort> {
    Arc::new(GuardedCartPromotionPort {
        inner: crate::ports::in_process_cart_promotion_port(db),
    })
}

struct GuardedCartPromotionPort {
    inner: Arc<dyn CartPromotionPort>,
}

#[async_trait]
impl CartPromotionPort for GuardedCartPromotionPort {
    async fn read_cart_promotion_preview(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartPromotionPreview, PortError> {
        let owner_operation = READ_CART_PROMOTION_PREVIEW_OPERATION;
        self.inner
            .read_cart_promotion_preview(context.clone(), request)
            .await
            .map_err(|error| cart_promotion_port_error(&context, owner_operation, error))
    }

    async fn apply_cart_promotion(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = APPLY_CART_PROMOTION_OPERATION;
        self.inner
            .apply_cart_promotion(context.clone(), request)
            .await
            .map_err(|error| cart_promotion_port_error(&context, owner_operation, error))
    }
}

fn cart_promotion_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    tracing::error!(
        internal_code = %error.code,
        internal_message = %error.message,
        kind = ?error.kind,
        retryable = error.retryable,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        "cart promotion owner operation failed"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;

    if code.starts_with("tax.") {
        return PortError::new(
            kind,
            code,
            "cart promotion tax recalculation failed",
            retryable,
        );
    }

    match kind {
        PortErrorKind::Validation if is_context_error_code(&code) => {
            PortError::validation(code, "cart promotion request context is invalid")
        }
        PortErrorKind::Validation => PortError::validation(
            "cart.promotion_validation",
            "cart promotion request is invalid",
        ),
        PortErrorKind::NotFound => match code.as_str() {
            "cart.cart_not_found" => {
                PortError::not_found(code, "cart was not found")
            }
            "cart.line_item_not_found" => {
                PortError::not_found(code, "cart line item was not found")
            }
            _ => PortError::not_found(
                "cart.promotion_resource_not_found",
                "cart promotion resource was not found",
            ),
        },
        PortErrorKind::Conflict => PortError::conflict(
            "cart.promotion_state_conflict",
            "cart promotion conflicts with the current cart state",
        ),
        PortErrorKind::Forbidden => PortError::forbidden(
            "cart.promotion_forbidden",
            "cart promotion is not allowed",
        ),
        PortErrorKind::Unavailable => {
            let public_code = if code == "cart.database_unavailable" {
                code
            } else {
                "cart.promotion_unavailable".to_string()
            };
            PortError::new(
                PortErrorKind::Unavailable,
                public_code,
                "cart promotion is temporarily unavailable",
                retryable,
            )
        }
        PortErrorKind::Timeout if is_context_error_code(&code) => {
            PortError::timeout(code, "cart promotion request context is invalid")
        }
        PortErrorKind::Timeout => {
            PortError::timeout("cart.promotion_timeout", "cart promotion request timed out")
        }
        PortErrorKind::InvariantViolation => PortError::new(
            PortErrorKind::InvariantViolation,
            "cart.promotion_invariant_violation",
            "cart promotion could not be completed safely",
            retryable,
        ),
    }
}

fn is_context_error_code(code: &str) -> bool {
    matches!(
        code,
        "cart.tenant_id_invalid" | "port.deadline_required" | "port.idempotency_key_required"
    )
}
