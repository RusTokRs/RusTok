use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotRequest, CartStorefrontAddLineItemRequest,
    CartStorefrontContextUpdateRequest, CartStorefrontCreateRequest,
    CartStorefrontLineItemPricingRequest, CartStorefrontLineItemQuantityRequest,
    CartStorefrontPort, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    CartStorefrontRepriceRequest,
};
use crate::{CartError, CartResponse, CartService};

const READ_CART_CHECKOUT_SNAPSHOT_OPERATION: &str = "read_cart_checkout_snapshot";
const UPDATE_CART_CHECKOUT_CONTEXT_OPERATION: &str = "update_cart_checkout_context";
const BEGIN_CART_CHECKOUT_OPERATION: &str = "begin_cart_checkout";
const RELEASE_CART_CHECKOUT_OPERATION: &str = "release_cart_checkout";
const COMPLETE_CART_CHECKOUT_OPERATION: &str = "complete_cart_checkout";
const READ_STOREFRONT_CART_OPERATION: &str = "read_storefront_cart";
const CREATE_STOREFRONT_CART_OPERATION: &str = "create_storefront_cart";
const ADD_STOREFRONT_LINE_ITEM_OPERATION: &str = "add_storefront_line_item";
const UPDATE_STOREFRONT_CONTEXT_OPERATION: &str = "update_storefront_context";
const UPDATE_STOREFRONT_LINE_ITEM_QUANTITY_OPERATION: &str = "update_storefront_line_item_quantity";
const UPDATE_STOREFRONT_LINE_ITEM_PRICING_OPERATION: &str = "update_storefront_line_item_pricing";
const REMOVE_STOREFRONT_LINE_ITEM_OPERATION: &str = "remove_storefront_line_item";
const REPRICE_STOREFRONT_LINE_ITEMS_OPERATION: &str = "reprice_storefront_line_items";

pub fn owner_cart_storefront_port(db: DatabaseConnection) -> Arc<dyn CartStorefrontPort> {
    Arc::new(OwnerCartPort::new(db))
}

pub fn owner_cart_checkout_port(db: DatabaseConnection) -> Arc<dyn CartCheckoutPort> {
    Arc::new(OwnerCartPort::new(db))
}

struct OwnerCartPort {
    service: CartService,
}

impl OwnerCartPort {
    fn new(db: DatabaseConnection) -> Self {
        Self {
            service: CartService::new(db),
        }
    }
}

#[async_trait]
impl CartCheckoutPort for OwnerCartPort {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = READ_CART_CHECKOUT_SNAPSHOT_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::read())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .get_cart(tenant_id, request.cart_id)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartCheckoutContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = UPDATE_CART_CHECKOUT_CONTEXT_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .update_context(tenant_id, request.cart_id, request.input)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = BEGIN_CART_CHECKOUT_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .begin_checkout(tenant_id, request.cart_id)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = RELEASE_CART_CHECKOUT_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .release_checkout(tenant_id, request.cart_id)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = COMPLETE_CART_CHECKOUT_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .complete_cart(tenant_id, request.cart_id)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }
}

#[async_trait]
impl CartStorefrontPort for OwnerCartPort {
    async fn read_storefront_cart(
        &self,
        context: PortContext,
        request: CartStorefrontReadRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = READ_STOREFRONT_CART_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::read())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .get_cart(tenant_id, request.cart_id)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn create_storefront_cart(
        &self,
        context: PortContext,
        request: CartStorefrontCreateRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = CREATE_STOREFRONT_CART_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .create_cart_with_channel(
                tenant_id,
                request.input,
                request.channel_id,
                request.channel_slug,
            )
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn add_storefront_line_item(
        &self,
        context: PortContext,
        request: CartStorefrontAddLineItemRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = ADD_STOREFRONT_LINE_ITEM_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .add_line_item_with_pricing_adjustment(
                tenant_id,
                request.cart_id,
                request.input,
                request.pricing_adjustment,
            )
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn update_storefront_context(
        &self,
        context: PortContext,
        request: CartStorefrontContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = UPDATE_STOREFRONT_CONTEXT_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .update_context(tenant_id, request.cart_id, request.input)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn update_storefront_line_item_quantity(
        &self,
        context: PortContext,
        request: CartStorefrontLineItemQuantityRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = UPDATE_STOREFRONT_LINE_ITEM_QUANTITY_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .update_line_item_quantity(
                tenant_id,
                request.cart_id,
                request.line_item_id,
                request.quantity,
            )
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn update_storefront_line_item_pricing(
        &self,
        context: PortContext,
        request: CartStorefrontLineItemPricingRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = UPDATE_STOREFRONT_LINE_ITEM_PRICING_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .update_line_item_pricing(
                tenant_id,
                request.cart_id,
                request.line_item_id,
                request.quantity,
                request.unit_price,
                request.pricing_adjustment,
            )
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn remove_storefront_line_item(
        &self,
        context: PortContext,
        request: CartStorefrontRemoveLineItemRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = REMOVE_STOREFRONT_LINE_ITEM_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .remove_line_item(tenant_id, request.cart_id, request.line_item_id)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }

    async fn reprice_storefront_line_items(
        &self,
        context: PortContext,
        request: CartStorefrontRepriceRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = REPRICE_STOREFRONT_LINE_ITEMS_OPERATION;
        require_cart_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_cart_tenant_id(&context, owner_operation)?;
        self.service
            .reprice_line_items(tenant_id, request.cart_id, request.updates)
            .await
            .map_err(|error| cart_error_to_port_error(&context, owner_operation, error))
    }
}

fn require_cart_policy(
    context: &PortContext,
    owner_operation: &'static str,
    policy: PortCallPolicy,
) -> Result<(), PortError> {
    context
        .require_policy(policy)
        .map_err(|error| cart_context_error(context, owner_operation, error))
}

fn parse_cart_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "cart.tenant_id_invalid",
            "cart owner tenant context is invalid"
        );
        PortError::validation("cart.tenant_id_invalid", "cart request context is invalid")
    })
}

fn cart_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    tracing::warn!(
        internal_code = %error.code,
        internal_message = %error.message,
        kind = ?error.kind,
        retryable = error.retryable,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "cart.context_invalid",
        "cart owner call context was rejected"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;
    match kind {
        PortErrorKind::Timeout => PortError::timeout(code, "cart request context is invalid"),
        PortErrorKind::Validation => PortError::validation(code, "cart request context is invalid"),
        kind => PortError::new(
            kind,
            "cart.context_invalid",
            "cart request context is invalid",
            retryable,
        ),
    }
}

fn cart_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CartError,
) -> PortError {
    let code = cart_error_code(&error);
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "cart owner operation failed"
    );

    match error {
        CartError::Validation(_) => {
            PortError::validation("cart.validation", "cart request is invalid")
        }
        CartError::CartNotFound(_) => {
            PortError::not_found("cart.cart_not_found", "cart was not found")
        }
        CartError::CartLineItemNotFound(_) => {
            PortError::not_found("cart.line_item_not_found", "cart line item was not found")
        }
        CartError::InvalidTransition { .. } => PortError::conflict(
            "cart.invalid_transition",
            "cart lifecycle transition conflicts with the current state",
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
        } => PortError::new(kind, code, "cart tax recalculation failed", retryable),
    }
}

fn cart_error_code(error: &CartError) -> &str {
    match error {
        CartError::Validation(_) => "cart.validation",
        CartError::CartNotFound(_) => "cart.cart_not_found",
        CartError::CartLineItemNotFound(_) => "cart.line_item_not_found",
        CartError::InvalidTransition { .. } => "cart.invalid_transition",
        CartError::Database(_) => "cart.database_unavailable",
        CartError::TaxBoundary { code, .. } => code.as_str(),
    }
}
