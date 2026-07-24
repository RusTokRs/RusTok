use async_graphql::{Context, ErrorExtensions, FieldError, Result};
use rustok_api::{AuthContext, graphql::GraphQLError};
use rustok_order::{OrderError, OrderService};
use uuid::Uuid;

pub(crate) use super::cart_safe_helpers::*;
use crate::storefront_shipping::normalize_shipping_profile_slug;
use crate::{CommerceError, ShippingProfileService};

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

fn order_graphql_error(
    tenant_id: Uuid,
    order_id: Uuid,
    operation: &'static str,
    error: OrderError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        order_id = %order_id,
        operation,
        "commerce GraphQL storefront order helper failed"
    );

    let (message, code, retryable) = match &error {
        OrderError::Validation(_) => ("Order request is invalid", "ORDER_REQUEST_INVALID", false),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            "Order resource was not found",
            "ORDER_RESOURCE_NOT_FOUND",
            false,
        ),
        OrderError::InvalidTransition { .. } => (
            "Order operation conflicts with the current state",
            "ORDER_STATE_CONFLICT",
            false,
        ),
        OrderError::Database(_) => (
            "Order service is temporarily unavailable",
            "ORDER_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        OrderError::Core(_) => (
            "Order operation could not be completed safely",
            "ORDER_OPERATION_FAILED",
            false,
        ),
    };

    public_graphql_error(message, code, retryable)
}

fn shipping_profile_graphql_error(
    tenant_id: Uuid,
    operation: &'static str,
    error: CommerceError,
) -> async_graphql::Error {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        operation,
        "commerce GraphQL shipping profile helper failed"
    );

    let (message, code, retryable) = match &error {
        CommerceError::Validation(_)
        | CommerceError::InvalidPrice(_)
        | CommerceError::InvalidOptionCombination
        | CommerceError::NoVariants => (
            "Shipping profile request is invalid",
            "SHIPPING_PROFILE_REQUEST_INVALID",
            false,
        ),
        CommerceError::ShippingProfileNotFound(_) => (
            "Shipping profile was not found",
            "SHIPPING_PROFILE_NOT_FOUND",
            false,
        ),
        CommerceError::DuplicateShippingProfileSlug(_) => (
            "Shipping profile conflicts with the current state",
            "SHIPPING_PROFILE_STATE_CONFLICT",
            false,
        ),
        CommerceError::Database(_) => (
            "Shipping profile service is temporarily unavailable",
            "SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::InsufficientInventory { .. }
        | CommerceError::CannotDeletePublished
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            "Shipping profile operation could not be completed safely",
            "SHIPPING_PROFILE_OPERATION_FAILED",
            false,
        ),
    };

    public_graphql_error(message, code, retryable)
}

pub(crate) async fn ensure_storefront_order_access(
    db: &sea_orm::DatabaseConnection,
    event_bus: &rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    ctx: &Context<'_>,
    order_id: Uuid,
) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let customer_id = super::cart_safe_helpers::resolve_optional_storefront_customer_id(
        db,
        tenant_id,
        Some(auth),
    )
    .await?
    .ok_or_else(|| <FieldError as GraphQLError>::unauthenticated())?;

    let order = OrderService::new(db.clone(), event_bus.clone())
        .get_order(tenant_id, order_id)
        .await
        .map_err(|error| {
            order_graphql_error(tenant_id, order_id, "ensure_storefront_order_access", error)
        })?;

    if order.customer_id != Some(customer_id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Order does not belong to the current customer",
        ));
    }

    Ok(())
}

pub(crate) async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> Result<()> {
    let Some(slug) = shipping_profile_slug.and_then(|s| normalize_shipping_profile_slug(s)) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(tenant_id, &slug)
        .await
        .map_err(|error| {
            shipping_profile_graphql_error(
                tenant_id,
                "validate_product_shipping_profile_input",
                error,
            )
        })?;

    Ok(())
}

pub(crate) async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> Result<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(|error| {
            shipping_profile_graphql_error(
                tenant_id,
                "validate_shipping_option_profile_inputs",
                error,
            )
        })?;

    Ok(())
}
