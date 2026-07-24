use async_graphql::{ErrorExtensions, Result};
use rustok_api::{AuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext};
use rustok_cart::CartStorefrontPort;
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_pricing::{PriceResolutionContext, PricingReadPort};
use uuid::Uuid;

use super::super::types::AddStorefrontCartLineItemInput;
pub(crate) use super::legacy_helpers::*;

fn public_graphql_error(
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn storefront_customer_port_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("storefront-customer:{user_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn customer_port_graphql_error(
    context: &PortContext,
    operation: &'static str,
    error: PortError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation,
        owner_code = %error.code,
        owner_kind = ?error.kind,
        owner_retryable = error.retryable,
        "commerce GraphQL storefront customer owner port failed"
    );

    let (message, code, retryable) = match &error.kind {
        PortErrorKind::Validation => (
            "Customer request is invalid",
            "CUSTOMER_REQUEST_INVALID",
            false,
        ),
        PortErrorKind::NotFound => ("Customer was not found", "CUSTOMER_NOT_FOUND", false),
        PortErrorKind::Conflict => (
            "Customer state conflicts with the requested operation",
            "CUSTOMER_STATE_CONFLICT",
            false,
        ),
        PortErrorKind::Forbidden => (
            "Customer operation is not permitted",
            "CUSTOMER_ACCESS_DENIED",
            false,
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "Customer information is temporarily unavailable",
            "CUSTOMER_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PortErrorKind::InvariantViolation => (
            "Customer operation could not be completed safely",
            "CUSTOMER_OPERATION_FAILED",
            false,
        ),
    };
    public_graphql_error(message, code, retryable)
}

pub(crate) fn cart_port_error(error: PortError) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        operation = "storefront_cart_port",
        owner_code = %error.code,
        owner_kind = ?error.kind,
        owner_retryable = error.retryable,
        "commerce GraphQL storefront cart owner port failed"
    );

    let (message, code, retryable) = match &error.kind {
        PortErrorKind::Validation => ("Cart request is invalid", "CART_REQUEST_INVALID", false),
        PortErrorKind::NotFound => (
            "Cart resource was not found",
            "CART_RESOURCE_NOT_FOUND",
            false,
        ),
        PortErrorKind::Conflict => (
            "Cart operation conflicts with the current state",
            "CART_STATE_CONFLICT",
            false,
        ),
        PortErrorKind::Forbidden => (
            "Cart operation is not permitted",
            "CART_ACCESS_DENIED",
            false,
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "Cart is temporarily unavailable",
            "CART_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PortErrorKind::InvariantViolation => (
            "Cart operation could not be completed safely",
            "CART_OPERATION_FAILED",
            false,
        ),
    };
    public_graphql_error(message, code, retryable)
}

pub(crate) async fn resolve_optional_storefront_customer_id(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    let port_context = storefront_customer_port_context(tenant_id, auth.user_id);
    let error_context = port_context.clone();
    match in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            port_context,
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(customer_port_graphql_error(
            &error_context,
            "resolve_optional_storefront_customer_id",
            error,
        )),
    }
}

fn legacy_graphql_error(
    error: async_graphql::Error,
    tenant_id: Uuid,
    resource_id: Option<Uuid>,
    operation: &'static str,
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        resource_id = ?resource_id,
        operation,
        "commerce GraphQL storefront cart helper failed"
    );
    public_graphql_error(message, code, retryable)
}

pub(crate) async fn enrich_storefront_cart(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    let cart_id = cart.id;
    super::legacy_helpers::enrich_storefront_cart(
        db,
        tenant_id,
        request_context,
        tenant_default_locale,
        cart,
    )
    .await
    .map_err(|error| {
        legacy_graphql_error(
            error,
            tenant_id,
            Some(cart_id),
            "enrich_storefront_cart",
            "Cart shipping details are temporarily unavailable",
            "CART_ENRICHMENT_UNAVAILABLE",
            true,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn validate_selected_shipping_option(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    cart: &crate::dto::CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    super::legacy_helpers::validate_selected_shipping_option(
        db,
        tenant_id,
        cart,
        selected_shipping_option_id,
        shipping_selections,
        currency_code,
        public_channel_slug,
        requested_locale,
        tenant_default_locale,
    )
    .await
    .map_err(|error| {
        legacy_graphql_error(
            error,
            tenant_id,
            Some(cart.id),
            "validate_selected_shipping_option",
            "Selected shipping option is invalid",
            "SHIPPING_OPTION_INVALID",
            false,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn resolve_storefront_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    pricing_read_port: &dyn PricingReadPort,
    pricing_port_context: PortContext,
    pricing_context: &PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: AddStorefrontCartLineItemInput,
) -> Result<ResolvedStorefrontLineItemInput> {
    let variant_id = input.variant_id;
    super::legacy_helpers::resolve_storefront_line_item_input(
        db,
        tenant_id,
        pricing_read_port,
        pricing_port_context,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    )
    .await
    .map_err(|error| {
        let detail = format!("{error:?}");
        let (message, code, retryable) =
            if detail.contains("Variant not found") || detail.contains("Product not found") {
                (
                    "Product is not available",
                    "CART_PRODUCT_UNAVAILABLE",
                    false,
                )
            } else if detail.contains("does not have enough available inventory") {
                (
                    "Requested quantity is not available",
                    "CART_INVENTORY_INSUFFICIENT",
                    false,
                )
            } else if detail.contains("Invalid JSON metadata payload") {
                (
                    "Cart line item input is invalid",
                    "CART_LINE_ITEM_INVALID",
                    false,
                )
            } else {
                (
                    "Cart line item could not be resolved",
                    "CART_LINE_ITEM_RESOLUTION_FAILED",
                    true,
                )
            };
        legacy_graphql_error(
            error,
            tenant_id,
            Some(variant_id),
            "resolve_storefront_line_item_input",
            message,
            code,
            retryable,
        )
    })
}

pub(crate) async fn reprice_storefront_cart_line_items(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    event_bus: &rustok_outbox::TransactionalEventBus,
    cart_storefront_port: &dyn CartStorefrontPort,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    let cart_id = cart.id;
    super::legacy_helpers::reprice_storefront_cart_line_items(
        db,
        tenant_id,
        request_context,
        event_bus,
        cart_storefront_port,
        cart,
    )
    .await
    .map_err(|error| {
        legacy_graphql_error(
            error,
            tenant_id,
            Some(cart_id),
            "reprice_storefront_cart_line_items",
            "Cart pricing could not be refreshed",
            "CART_REPRICE_FAILED",
            true,
        )
    })
}

pub(crate) async fn validate_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    super::legacy_helpers::validate_storefront_line_item_quantity(
        db,
        tenant_id,
        variant_id,
        requested_quantity,
        public_channel_slug,
    )
    .await
    .map_err(|error| {
        let detail = format!("{error:?}");
        let (message, code, retryable) = if detail.contains("Variant not found") {
            (
                "Product is not available",
                "CART_PRODUCT_UNAVAILABLE",
                false,
            )
        } else if detail.contains("does not have enough available inventory") {
            (
                "Requested quantity is not available",
                "CART_INVENTORY_INSUFFICIENT",
                false,
            )
        } else {
            (
                "Inventory availability could not be verified",
                "CART_INVENTORY_UNAVAILABLE",
                true,
            )
        };
        legacy_graphql_error(
            error,
            tenant_id,
            Some(variant_id),
            "validate_storefront_line_item_quantity",
            message,
            code,
            retryable,
        )
    })
}
