use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use sea_orm::DatabaseConnection;

use crate::CartResponse;
use crate::guest_access::{
    attach_transient_guest_token, guest_cart_token_from_context, prepare_guest_cart_metadata,
    record_issued_guest_cart_token, sanitize_guest_cart_metadata, verify_guest_cart_token,
};
use crate::ports::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotRequest, CartStorefrontAddLineItemRequest,
    CartStorefrontContextUpdateRequest, CartStorefrontCreateRequest,
    CartStorefrontLineItemPricingRequest, CartStorefrontLineItemQuantityRequest,
    CartStorefrontPort, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    CartStorefrontRepriceRequest,
};

pub fn guarded_cart_storefront_port(db: DatabaseConnection) -> Arc<dyn CartStorefrontPort> {
    Arc::new(GuardedCartPort::new(
        crate::owner_ports::owner_cart_storefront_port(db.clone()),
        crate::owner_ports::owner_cart_checkout_port(db),
    ))
}

pub fn guarded_cart_checkout_port(db: DatabaseConnection) -> Arc<dyn CartCheckoutPort> {
    Arc::new(GuardedCartPort::new(
        crate::owner_ports::owner_cart_storefront_port(db.clone()),
        crate::owner_ports::owner_cart_checkout_port(db),
    ))
}

struct GuardedCartPort {
    storefront: Arc<dyn CartStorefrontPort>,
    checkout: Arc<dyn CartCheckoutPort>,
}

impl GuardedCartPort {
    fn new(storefront: Arc<dyn CartStorefrontPort>, checkout: Arc<dyn CartCheckoutPort>) -> Self {
        Self {
            storefront,
            checkout,
        }
    }

    async fn authorize_cart(
        &self,
        context: &PortContext,
        cart_id: uuid::Uuid,
    ) -> Result<(), PortError> {
        let cart = self
            .storefront
            .read_storefront_cart(context.clone(), CartStorefrontReadRequest { cart_id })
            .await?;
        authorize_guest_cart(context, &cart)
    }
}

fn authorize_guest_cart(context: &PortContext, cart: &CartResponse) -> Result<(), PortError> {
    if cart.customer_id.is_some() {
        return Ok(());
    }

    let presented_token = guest_cart_token_from_context(context);
    if verify_guest_cart_token(&cart.metadata, presented_token.as_deref()) {
        Ok(())
    } else {
        Err(PortError::forbidden(
            "cart.guest_access_denied",
            "A valid guest cart access token is required",
        ))
    }
}

fn sanitize_cart(mut cart: CartResponse) -> CartResponse {
    cart.metadata = sanitize_guest_cart_metadata(cart.metadata);
    cart
}

#[async_trait]
impl CartStorefrontPort for GuardedCartPort {
    async fn read_storefront_cart(
        &self,
        context: PortContext,
        request: CartStorefrontReadRequest,
    ) -> Result<CartResponse, PortError> {
        let cart = self
            .storefront
            .read_storefront_cart(context.clone(), request)
            .await?;
        authorize_guest_cart(&context, &cart)?;
        Ok(sanitize_cart(cart))
    }

    async fn create_storefront_cart(
        &self,
        context: PortContext,
        mut request: CartStorefrontCreateRequest,
    ) -> Result<CartResponse, PortError> {
        let (metadata, token) = prepare_guest_cart_metadata(
            request.input.customer_id,
            std::mem::take(&mut request.input.metadata),
        );
        request.input.metadata = metadata;
        let mut cart = sanitize_cart(
            self.storefront
                .create_storefront_cart(context, request)
                .await?,
        );
        if let Some(token) = token {
            record_issued_guest_cart_token(&token);
            // Preserve compatibility for non-HTTP callers that receive the
            // newly created cart directly. The token is transient and was
            // never persisted; later reads sanitize all reserved fields.
            attach_transient_guest_token(&mut cart, token);
        }
        Ok(cart)
    }

    async fn add_storefront_line_item(
        &self,
        context: PortContext,
        request: CartStorefrontAddLineItemRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.storefront
            .add_storefront_line_item(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn update_storefront_context(
        &self,
        context: PortContext,
        request: CartStorefrontContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.storefront
            .update_storefront_context(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn update_storefront_line_item_quantity(
        &self,
        context: PortContext,
        request: CartStorefrontLineItemQuantityRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.storefront
            .update_storefront_line_item_quantity(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn update_storefront_line_item_pricing(
        &self,
        context: PortContext,
        request: CartStorefrontLineItemPricingRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.storefront
            .update_storefront_line_item_pricing(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn remove_storefront_line_item(
        &self,
        context: PortContext,
        request: CartStorefrontRemoveLineItemRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.storefront
            .remove_storefront_line_item(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn reprice_storefront_line_items(
        &self,
        context: PortContext,
        request: CartStorefrontRepriceRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.storefront
            .reprice_storefront_line_items(context, request)
            .await
            .map(sanitize_cart)
    }
}

#[async_trait]
impl CartCheckoutPort for GuardedCartPort {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.checkout
            .read_cart_checkout_snapshot(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartCheckoutContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.checkout
            .update_cart_checkout_context(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.checkout
            .begin_cart_checkout(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.checkout
            .release_cart_checkout(context, request)
            .await
            .map(sanitize_cart)
    }

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        self.authorize_cart(&context, request.cart_id).await?;
        self.checkout
            .complete_cart_checkout(context, request)
            .await
            .map(sanitize_cart)
    }
}

#[cfg(test)]
mod tests {
    use super::authorize_guest_cart;
    use crate::CartResponse;
    use crate::guest_access::{guest_cart_claim, prepare_guest_cart_metadata};
    use rust_decimal::Decimal;
    use rustok_api::{PortActor, PortContext};
    use serde_json::json;
    use uuid::Uuid;

    fn cart(metadata: serde_json::Value) -> CartResponse {
        let now = chrono::Utc::now();
        CartResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            channel_id: None,
            channel_slug: None,
            customer_id: None,
            email: None,
            region_id: None,
            country_code: None,
            locale_code: None,
            selected_shipping_option_id: None,
            status: "active".to_string(),
            currency_code: "USD".to_string(),
            subtotal_amount: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            shipping_total: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            metadata,
            created_at: now,
            updated_at: now,
            completed_at: None,
            line_items: Vec::new(),
            adjustments: Vec::new(),
            tax_lines: Vec::new(),
            delivery_groups: Vec::new(),
        }
    }

    #[test]
    fn guest_cart_requires_matching_claim() {
        let (metadata, token) = prepare_guest_cart_metadata(None, json!({}));
        let token = token.expect("guest token");
        let base = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::service("storefront"),
            "en",
            "request",
        );

        assert!(authorize_guest_cart(&base, &cart(metadata.clone())).is_err());
        assert!(
            authorize_guest_cart(
                &base.with_claim(guest_cart_claim(&token).expect("claim")),
                &cart(metadata),
            )
            .is_ok()
        );
    }
}
