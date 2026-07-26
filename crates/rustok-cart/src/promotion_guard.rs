use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports::{
    CartPromotionKindRequest, CartPromotionPort, CartPromotionRequest, CartPromotionScopeRequest,
};
use crate::{CartError, CartPromotionPreview, CartResponse, CartService};

const CART_PROMOTION_OWNER: &str = "rustok_cart.promotion";
const CART_PROMOTION_CONTEXT_BOUNDARY: &str = "cart_promotion_context";
const READ_CART_PROMOTION_PREVIEW_OPERATION: &str = "read_cart_promotion_preview";
const APPLY_CART_PROMOTION_OPERATION: &str = "apply_cart_promotion";

pub fn guarded_cart_promotion_port(db: DatabaseConnection) -> Arc<dyn CartPromotionPort> {
    Arc::new(GuardedCartPromotionPort {
        service: CartService::new(db),
    })
}

struct GuardedCartPromotionPort {
    service: CartService,
}

#[async_trait]
impl CartPromotionPort for GuardedCartPromotionPort {
    async fn read_cart_promotion_preview(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartPromotionPreview, PortError> {
        let owner_operation = READ_CART_PROMOTION_PREVIEW_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| cart_promotion_context_error(&context, owner_operation, error))?;
        validate_cart_promotion_request(&context, owner_operation, &request)?;
        let tenant_id = parse_cart_promotion_tenant_id(&context, owner_operation)?;

        match (request.scope, request.kind) {
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .preview_percentage_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .preview_fixed_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .preview_percentage_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .preview_fixed_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
        }
        .map_err(|error| cart_promotion_error(&context, owner_operation, error))
    }

    async fn apply_cart_promotion(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = APPLY_CART_PROMOTION_OPERATION;
        context
            .require_write_semantics()
            .map_err(|error| cart_promotion_context_error(&context, owner_operation, error))?;
        validate_cart_promotion_request(&context, owner_operation, &request)?;
        let tenant_id = parse_cart_promotion_tenant_id(&context, owner_operation)?;

        match (request.scope, request.kind) {
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .apply_percentage_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .apply_fixed_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .apply_percentage_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .apply_fixed_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
        }
        .map_err(|error| cart_promotion_error(&context, owner_operation, error))
    }
}

fn validate_cart_promotion_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &CartPromotionRequest,
) -> Result<(), PortError> {
    let code = match &request.scope {
        CartPromotionScopeRequest::LineItem if request.line_item_id.is_none() => {
            Some("cart.promotion_line_item_required")
        }
        CartPromotionScopeRequest::Shipping if request.line_item_id.is_some() => {
            Some("cart.promotion_shipping_line_item_forbidden")
        }
        _ => None,
    };

    if let Some(code) = code {
        tracing::warn!(
            scope = ?request.scope,
            line_item_present = request.line_item_id.is_some(),
            owner = CART_PROMOTION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation = owner_operation,
            code,
            "cart promotion target validation failed"
        );
        return Err(PortError::validation(
            code,
            "cart promotion request is invalid",
        ));
    }

    Ok(())
}

fn parse_cart_promotion_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            owner = CART_PROMOTION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            channel = ?context.channel,
            operation = owner_operation,
            code = "cart.tenant_id_invalid",
            "cart promotion tenant context is invalid"
        );
        PortError::validation(
            "cart.tenant_id_invalid",
            "cart promotion request context is invalid",
        )
    })
}

fn cart_promotion_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    log_cart_promotion_context_rejection(context, owner_operation, &error);

    match error.kind {
        PortErrorKind::Timeout => {
            PortError::timeout(error.code, "cart promotion request context is invalid")
        }
        PortErrorKind::Validation => {
            PortError::validation(error.code, "cart promotion request context is invalid")
        }
        kind => PortError::new(
            kind,
            "cart.promotion_context_invalid",
            "cart promotion request context is invalid",
            error.retryable,
        ),
    }
}

fn log_cart_promotion_context_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    error: &PortError,
) {
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                internal_code = %error.code,
                internal_message = %error.message,
                owner = CART_PROMOTION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                operation = owner_operation,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CART_PROMOTION_CONTEXT_BOUNDARY,
                code = "cart.promotion_context_invalid",
                "cart promotion call context failed"
            );
        }
        _ => {
            tracing::warn!(
                error = ?error,
                internal_code = %error.code,
                internal_message = %error.message,
                owner = CART_PROMOTION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                operation = owner_operation,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CART_PROMOTION_CONTEXT_BOUNDARY,
                code = "cart.promotion_context_invalid",
                "cart promotion call context was rejected"
            );
        }
    }
}

fn cart_promotion_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CartError,
) -> PortError {
    let code = cart_promotion_error_code(&error);
    tracing::error!(
        error = ?error,
        owner = CART_PROMOTION_OWNER,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        channel = ?context.channel,
        operation = owner_operation,
        code,
        "cart promotion owner operation failed"
    );

    match error {
        CartError::Validation(_) => PortError::validation(
            "cart.promotion_validation",
            "cart promotion request is invalid",
        ),
        CartError::CartNotFound(_) => {
            PortError::not_found("cart.cart_not_found", "cart was not found")
        }
        CartError::CartLineItemNotFound(_) => {
            PortError::not_found("cart.line_item_not_found", "cart line item was not found")
        }
        CartError::InvalidTransition { .. } => PortError::conflict(
            "cart.promotion_state_conflict",
            "cart promotion conflicts with the current cart state",
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.database_unavailable",
            "cart storage is temporarily unavailable",
        ),
        CartError::TaxBoundary {
            kind,
            code,
            retryable,
            ..
        } => PortError::new(
            kind,
            code,
            "cart promotion tax recalculation failed",
            retryable,
        ),
    }
}

fn cart_promotion_error_code(error: &CartError) -> &str {
    match error {
        CartError::Validation(_) => "cart.promotion_validation",
        CartError::CartNotFound(_) => "cart.cart_not_found",
        CartError::CartLineItemNotFound(_) => "cart.line_item_not_found",
        CartError::InvalidTransition { .. } => "cart.promotion_state_conflict",
        CartError::Database(_) => "cart.database_unavailable",
        CartError::TaxBoundary { code, .. } => code.as_str(),
    }
}
