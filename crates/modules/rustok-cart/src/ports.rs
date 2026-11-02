use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    AddCartLineItemInput, CartError, CartLineItemPricingUpdate, CartPricingAdjustmentUpdate,
    CartResponse, CreateCartInput, UpdateCartContextInput,
};

/// Transport-neutral owner boundary for cart checkout snapshots and lifecycle.
#[async_trait]
pub trait CartCheckoutPort: Send + Sync {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError>;

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartContextUpdateRequest,
    ) -> Result<CartResponse, PortError>;

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError>;

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError>;

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError>;
}

/// Builds the owner-managed in-process checkout/read provider for explicit consumers.
pub fn in_process_cart_checkout_port(db: DatabaseConnection) -> Arc<dyn CartCheckoutPort> {
    Arc::new(crate::CartService::new(db))
}

/// Transport-neutral owner boundary for storefront cart reads and mutations.
#[async_trait]
pub trait CartStorefrontPort: Send + Sync {
    async fn read_storefront_cart(
        &self,
        context: PortContext,
        request: CartStorefrontReadRequest,
    ) -> Result<CartResponse, PortError>;

    async fn create_storefront_cart(
        &self,
        context: PortContext,
        request: CartStorefrontCreateRequest,
    ) -> Result<CartResponse, PortError>;

    async fn add_storefront_line_item(
        &self,
        context: PortContext,
        request: CartStorefrontAddLineItemRequest,
    ) -> Result<CartResponse, PortError>;

    async fn update_storefront_context(
        &self,
        context: PortContext,
        request: CartContextUpdateRequest,
    ) -> Result<CartResponse, PortError>;

    async fn update_storefront_line_item_pricing(
        &self,
        context: PortContext,
        request: CartStorefrontLineItemPricingRequest,
    ) -> Result<CartResponse, PortError>;

    async fn remove_storefront_line_item(
        &self,
        context: PortContext,
        request: CartStorefrontRemoveLineItemRequest,
    ) -> Result<CartResponse, PortError>;

    async fn reprice_storefront_line_items(
        &self,
        context: PortContext,
        request: CartStorefrontRepriceRequest,
    ) -> Result<CartResponse, PortError>;
}

/// Transport-neutral owner boundary for cart promotion preview and application.
#[async_trait]
pub trait CartPromotionPort: Send + Sync {
    async fn read_cart_promotion_preview(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<crate::CartPromotionPreview, PortError>;

    async fn apply_cart_promotion(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartResponse, PortError>;
}

/// The module and root import paths expose the same admitted owner provider.
pub use crate::guarded_ports::guarded_cart_storefront_port as in_process_cart_storefront_port;

/// Builds the owner-managed in-process promotion provider for explicit consumers.
pub fn in_process_cart_promotion_port(db: DatabaseConnection) -> Arc<dyn CartPromotionPort> {
    Arc::new(crate::CartService::new(db))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CartPromotionScopeRequest {
    Cart,
    LineItem,
    Shipping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CartPromotionKindRequest {
    PercentageDiscount,
    FixedDiscount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartPromotionRequest {
    pub cart_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub scope: CartPromotionScopeRequest,
    pub kind: CartPromotionKindRequest,
    pub source_id: String,
    pub amount: rust_decimal::Decimal,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CartStorefrontReadRequest {
    pub cart_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartStorefrontCreateRequest {
    pub input: CreateCartInput,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartStorefrontAddLineItemRequest {
    pub cart_id: Uuid,
    pub input: AddCartLineItemInput,
    pub pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
    /// Context read by Commerce before obtaining the authoritative Pricing result.
    pub pricing_context: crate::CartPricingContextGuard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartContextUpdateRequest {
    pub cart_id: Uuid,
    pub input: UpdateCartContextInput,
    pub pricing_context: crate::CartPricingContextGuard,
    pub pricing: crate::CartContextPricing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartStorefrontLineItemPricingRequest {
    pub cart_id: Uuid,
    pub line_item_id: Uuid,
    pub quantity: i32,
    pub unit_price: rust_decimal::Decimal,
    pub pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
    pub pricing_context: crate::CartPricingContextGuard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CartStorefrontRemoveLineItemRequest {
    pub cart_id: Uuid,
    pub line_item_id: Uuid,
    pub pricing_context: crate::CartPricingContextGuard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartStorefrontRepriceRequest {
    pub cart_id: Uuid,
    pub updates: Vec<CartLineItemPricingUpdate>,
    pub pricing_context: crate::CartPricingContextGuard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CartCheckoutSnapshotRequest {
    pub cart_id: Uuid,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CartCheckoutLifecycleRequest {
    pub cart_id: Uuid,
}

fn validate_non_nil_uuid(id: Uuid, field_name: &'static str) -> Result<(), PortError> {
    if id.is_nil() {
        Err(PortError::validation(
            format!("cart.{field_name}_nil"),
            format!("{field_name} must not be a nil UUID"),
        ))
    } else {
        Ok(())
    }
}

#[async_trait]
impl CartCheckoutPort for crate::CartService {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_non_nil_uuid(request.cart_id, "cart_id")?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_checkout_port",
            operation = "read_cart_checkout_snapshot",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            "reading cart checkout snapshot"
        );
        self.get_cart(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        validate_non_nil_uuid(request.cart_id, "cart_id")?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_checkout_port",
            operation = "update_cart_checkout_context",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            "updating cart checkout context"
        );
        self.update_context(tenant_id, request)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        validate_non_nil_uuid(request.cart_id, "cart_id")?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_checkout_port",
            operation = "begin_cart_checkout",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            "beginning cart checkout"
        );
        self.begin_checkout(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        validate_non_nil_uuid(request.cart_id, "cart_id")?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_checkout_port",
            operation = "release_cart_checkout",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            "releasing cart checkout"
        );
        self.release_checkout(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        validate_non_nil_uuid(request.cart_id, "cart_id")?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_checkout_port",
            operation = "complete_cart_checkout",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            "completing cart checkout"
        );
        self.complete_cart(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }
}

fn validate_cart_promotion_request(request: &CartPromotionRequest) -> Result<(), PortError> {
    validate_non_nil_uuid(request.cart_id, "cart_id")?;
    if let Some(line_item_id) = request.line_item_id {
        validate_non_nil_uuid(line_item_id, "line_item_id")?;
    }
    match request.scope {
        CartPromotionScopeRequest::LineItem if request.line_item_id.is_none() => {
            Err(PortError::validation(
                "cart.promotion_line_item_required",
                "line_item_id is required for line-item cart promotions",
            ))
        }
        CartPromotionScopeRequest::Shipping if request.line_item_id.is_some() => {
            Err(PortError::validation(
                "cart.promotion_shipping_line_item_forbidden",
                "line_item_id is not allowed for shipping cart promotions",
            ))
        }
        _ => Ok(()),
    }
}

#[async_trait]
impl CartPromotionPort for crate::CartService {
    async fn read_cart_promotion_preview(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<crate::CartPromotionPreview, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_cart_promotion_request(&request)?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_promotion_port",
            operation = "read_cart_promotion_preview",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            line_item_id = ?request.line_item_id,
            scope = ?request.scope,
            kind = ?request.kind,
            "reading cart promotion preview"
        );
        match (request.scope, request.kind) {
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::PercentageDiscount) => {
                self.preview_percentage_shipping_promotion(
                    tenant_id,
                    request.cart_id,
                    &request.source_id,
                    request.amount,
                )
                .await
            }
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::FixedDiscount) => {
                self.preview_fixed_shipping_promotion(
                    tenant_id,
                    request.cart_id,
                    &request.source_id,
                    request.amount,
                )
                .await
            }
            (_, CartPromotionKindRequest::PercentageDiscount) => {
                self.preview_percentage_promotion(
                    tenant_id,
                    request.cart_id,
                    request.line_item_id,
                    &request.source_id,
                    request.amount,
                )
                .await
            }
            (_, CartPromotionKindRequest::FixedDiscount) => {
                self.preview_fixed_promotion(
                    tenant_id,
                    request.cart_id,
                    request.line_item_id,
                    &request.source_id,
                    request.amount,
                )
                .await
            }
        }
        .map_err(cart_error_to_port_error)
    }

    async fn apply_cart_promotion(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        validate_cart_promotion_request(&request)?;
        let tenant_id = parse_port_tenant_id(&context)?;
        tracing::debug!(
            boundary = "cart_promotion_port",
            operation = "apply_cart_promotion",
            tenant_id = %tenant_id,
            cart_id = %request.cart_id,
            line_item_id = ?request.line_item_id,
            scope = ?request.scope,
            kind = ?request.kind,
            "applying cart promotion"
        );
        match (request.scope, request.kind) {
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::PercentageDiscount) => {
                self.apply_percentage_shipping_promotion(
                    tenant_id,
                    request.cart_id,
                    &request.source_id,
                    request.amount,
                    request.metadata,
                )
                .await
            }
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::FixedDiscount) => {
                self.apply_fixed_shipping_promotion(
                    tenant_id,
                    request.cart_id,
                    &request.source_id,
                    request.amount,
                    request.metadata,
                )
                .await
            }
            (_, CartPromotionKindRequest::PercentageDiscount) => {
                self.apply_percentage_promotion(
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
                self.apply_fixed_promotion(
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
        .map_err(cart_error_to_port_error)
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "cart.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for cart ports",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "cart.tenant_id_nil",
            "PortContext.tenant_id must not be a nil UUID for cart ports",
        ));
    }
    Ok(id)
}

fn cart_error_to_port_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => PortError::validation("cart.validation", message),
        CartError::PricingContextChanged => PortError::conflict(
            "cart.pricing_context_changed",
            "Cart pricing context changed; refresh the cart and quote before adding an item",
        ),
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
        CartError::TaxBoundary {
            kind,
            code,
            message,
            retryable,
        }
        | CartError::ShippingBoundary {
            kind,
            code,
            message,
            retryable,
        } => PortError::new(kind, code, message, retryable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn validate_non_nil_uuid_rejects_nil() {
        let err = validate_non_nil_uuid(Uuid::nil(), "cart_id").expect_err("should reject nil");
        assert_eq!(err.code, "cart.cart_id_nil");
        assert!(err.message.contains("cart_id must not be a nil UUID"));

        assert!(validate_non_nil_uuid(Uuid::new_v4(), "cart_id").is_ok());
    }

    #[test]
    fn parse_port_tenant_id_validates_nil_and_malformed() {
        let nil_context =
            PortContext::new(Uuid::nil().to_string(), PortActor::system(), "en", "req-1");
        let err = parse_port_tenant_id(&nil_context).expect_err("should reject nil tenant");
        assert_eq!(err.code, "cart.tenant_id_nil");

        let invalid_context =
            PortContext::new("not-a-uuid".to_string(), PortActor::system(), "en", "req-2");
        let err = parse_port_tenant_id(&invalid_context).expect_err("should reject invalid tenant");
        assert_eq!(err.code, "cart.tenant_id_invalid");

        let valid_uuid = Uuid::new_v4();
        let valid_context =
            PortContext::new(valid_uuid.to_string(), PortActor::system(), "en", "req-3");
        let parsed = parse_port_tenant_id(&valid_context).expect("valid tenant");
        assert_eq!(parsed, valid_uuid);
    }
}
