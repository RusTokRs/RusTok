use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::core::{
    CartCoreError, normalize_public_channel_slug, parse_cart_id, parse_line_item_id,
};
use crate::model::StorefrontCartData;

use super::native_server_mapping::{map_native_cart, storefront_cart_pricing_update};

const CART_STOREFRONT_NATIVE_BOUNDARY: &str = "cart_storefront_native_transport";
const CART_STOREFRONT_NATIVE_OWNER: &str = "rustok_cart.storefront";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<CartCoreError> for ApiError {
    fn from(value: CartCoreError) -> Self {
        match value {
            CartCoreError::Validation(error) => Self::Validation(error),
        }
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

fn context_extraction_error<E>(
    owner_operation: &'static str,
    code: &'static str,
    public_message: &'static str,
    _error: E,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = CART_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        code,
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront request context extraction failed"
    );
    ServerFnError::new(public_message)
}

fn tenant_context_error<E>(error: E) -> ServerFnError {
    context_extraction_error(
        "extract_tenant_context",
        "cart.storefront_tenant_context_unavailable",
        "Storefront tenant context is temporarily unavailable",
        error,
    )
}

fn auth_context_error<E>(error: E) -> ServerFnError {
    context_extraction_error(
        "extract_optional_auth_context",
        "cart.storefront_auth_context_unavailable",
        "Storefront authentication context is temporarily unavailable",
        error,
    )
}

fn transactional_event_bus_from_runtime(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    endpoint: &'static str,
) -> Result<rustok_outbox::TransactionalEventBus, ServerFnError> {
    runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            tracing::error!(
                owner = CART_STOREFRONT_NATIVE_OWNER,
                owner_operation = "resolve_transactional_event_bus",
                endpoint,
                code = "cart.storefront_event_bus_unavailable",
                boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
                "cart storefront transactional event bus is missing"
            );
            ServerFnError::new("Cart runtime is temporarily unavailable")
        })
}

fn cart_input_error(
    error: CartCoreError,
    owner_operation: &'static str,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    let error_type = std::any::type_name_of_val(&error);
    tracing::warn!(
        error_type,
        owner = CART_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        code,
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront input was rejected"
    );
    ServerFnError::new(public_message)
}

fn customer_error(
    error: rustok_customer::CustomerError,
    tenant_id: Uuid,
    user_id: Uuid,
    owner_operation: &'static str,
    request_context: Option<&rustok_api::RequestContext>,
) -> ServerFnError {
    let error_type = std::any::type_name_of_val(&error);
    tracing::error!(
        error_type,
        owner = "rustok_customer",
        owner_operation,
        consumer = CART_STOREFRONT_NATIVE_OWNER,
        request_context_present = request_context.is_some(),
        request_tenant_id_non_nil = ?request_context.map(|context| !context.tenant_id.is_nil()),
        tenant_id_non_nil = !tenant_id.is_nil(),
        user_id_non_nil = !user_id.is_nil(),
        channel_id_present = request_context.and_then(|context| context.channel_id).is_some(),
        channel_id_non_nil = ?request_context
            .and_then(|context| context.channel_id)
            .map(|value| !value.is_nil()),
        channel_slug_present = request_context
            .and_then(|context| context.channel_slug.as_ref())
            .is_some(),
        channel_slug_length = ?request_context
            .and_then(|context| context.channel_slug.as_ref())
            .map(|value| value.chars().count()),
        locale_present = request_context
            .map(|context| !context.locale.trim().is_empty())
            .unwrap_or(false),
        locale_length = ?request_context.map(|context| context.locale.chars().count()),
        code = "cart.storefront_customer_unavailable",
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront customer lookup failed"
    );
    ServerFnError::new("Customer information is temporarily unavailable")
}

fn cart_error(
    error: rustok_cart::CartError,
    tenant_id: Uuid,
    owner_operation: &'static str,
    cart_id: Option<Uuid>,
    line_item_id: Option<Uuid>,
    request_context: Option<&rustok_api::RequestContext>,
) -> ServerFnError {
    use rustok_api::PortErrorKind;
    use rustok_cart::CartError;

    let (public_message, public_code, retryable, technical) = match &error {
        CartError::Validation(_) => (
            "Cart request is invalid",
            "cart.storefront_request_invalid",
            false,
            false,
        ),
        CartError::CartNotFound(_) => (
            "Cart was not found",
            "cart.storefront_cart_not_found",
            false,
            false,
        ),
        CartError::CartLineItemNotFound(_) => (
            "Cart line item was not found",
            "cart.storefront_line_item_not_found",
            false,
            false,
        ),
        CartError::InvalidTransition { .. } => (
            "Cart operation conflicts with the current state",
            "cart.storefront_state_conflict",
            false,
            false,
        ),
        CartError::Database(_) => (
            "Cart is temporarily unavailable",
            "cart.storefront_storage_unavailable",
            true,
            true,
        ),
        CartError::TaxBoundary {
            kind,
            retryable: owner_retryable,
            ..
        } => match kind {
            PortErrorKind::Validation => (
                "Cart tax request is invalid",
                "cart.storefront_tax_invalid",
                false,
                false,
            ),
            PortErrorKind::NotFound => (
                "Cart tax policy was not found",
                "cart.storefront_tax_not_found",
                false,
                false,
            ),
            PortErrorKind::Conflict => (
                "Cart tax calculation conflicts with the current state",
                "cart.storefront_tax_conflict",
                false,
                false,
            ),
            PortErrorKind::Forbidden => (
                "Cart tax calculation is not permitted",
                "cart.storefront_tax_forbidden",
                false,
                false,
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Cart tax calculation is temporarily unavailable",
                "cart.storefront_tax_unavailable",
                *owner_retryable,
                true,
            ),
            PortErrorKind::InvariantViolation => (
                "Cart tax calculation could not be completed safely",
                "cart.storefront_tax_failed",
                false,
                true,
            ),
        },
    };

    let error_type = std::any::type_name_of_val(&error);
    let correlation_id = format!(
        "cart-storefront-native-{owner_operation}-{}",
        Uuid::new_v4()
    );
    let request_context_present = request_context.is_some();
    let request_tenant_id_non_nil = request_context.map(|context| !context.tenant_id.is_nil());
    let tenant_id_non_nil = !tenant_id.is_nil();
    let cart_id_present = cart_id.is_some();
    let cart_id_non_nil = cart_id.map(|value| !value.is_nil());
    let line_item_id_present = line_item_id.is_some();
    let line_item_id_non_nil = line_item_id.map(|value| !value.is_nil());
    let channel_id_present = request_context
        .and_then(|context| context.channel_id)
        .is_some();
    let channel_id_non_nil = request_context
        .and_then(|context| context.channel_id)
        .map(|value| !value.is_nil());
    let channel_slug_present = request_context
        .and_then(|context| context.channel_slug.as_ref())
        .is_some();
    let channel_slug_length = request_context
        .and_then(|context| context.channel_slug.as_ref())
        .map(|value| value.chars().count());
    let locale_present = request_context
        .map(|context| !context.locale.trim().is_empty())
        .unwrap_or(false);
    let locale_length = request_context.map(|context| context.locale.chars().count());

    if technical {
        tracing::error!(
            error_type,
            owner = "rustok_cart",
            owner_operation,
            consumer = CART_STOREFRONT_NATIVE_OWNER,
            correlation_id = ?correlation_id,
            request_context_present,
            request_tenant_id_non_nil = ?request_tenant_id_non_nil,
            tenant_id_non_nil,
            cart_id_present,
            cart_id_non_nil = ?cart_id_non_nil,
            line_item_id_present,
            line_item_id_non_nil = ?line_item_id_non_nil,
            channel_id_present,
            channel_id_non_nil = ?channel_id_non_nil,
            channel_slug_present,
            channel_slug_length = ?channel_slug_length,
            locale_present,
            locale_length = ?locale_length,
            public_code,
            public_retryable = retryable,
            boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
            "cart storefront owner operation failed"
        );
    } else {
        tracing::warn!(
            error_type,
            owner = "rustok_cart",
            owner_operation,
            consumer = CART_STOREFRONT_NATIVE_OWNER,
            correlation_id = ?correlation_id,
            request_context_present,
            request_tenant_id_non_nil = ?request_tenant_id_non_nil,
            tenant_id_non_nil,
            cart_id_present,
            cart_id_non_nil = ?cart_id_non_nil,
            line_item_id_present,
            line_item_id_non_nil = ?line_item_id_non_nil,
            channel_id_present,
            channel_id_non_nil = ?channel_id_non_nil,
            channel_slug_present,
            channel_slug_length = ?channel_slug_length,
            locale_present,
            locale_length = ?locale_length,
            public_code,
            public_retryable = retryable,
            boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
            "cart storefront owner operation was rejected"
        );
    }

    ServerFnError::new(public_message)
}

fn pricing_error(
    error: rustok_api::PortError,
    tenant_id: Uuid,
    owner_operation: &'static str,
    cart_id: Uuid,
    line_item_id: Uuid,
    request_context: Option<&rustok_api::RequestContext>,
) -> ServerFnError {
    let technical = matches!(
        &error.kind,
        rustok_api::PortErrorKind::Unavailable
            | rustok_api::PortErrorKind::Timeout
            | rustok_api::PortErrorKind::InvariantViolation
    );
    let error_type = std::any::type_name_of_val(&error);
    let correlation_id = format!(
        "cart-storefront-pricing-{owner_operation}-{}",
        Uuid::new_v4()
    );
    let request_context_present = request_context.is_some();
    let request_tenant_id_non_nil = request_context.map(|context| !context.tenant_id.is_nil());
    let tenant_id_non_nil = !tenant_id.is_nil();
    let cart_id_non_nil = !cart_id.is_nil();
    let line_item_id_non_nil = !line_item_id.is_nil();
    let channel_id_present = request_context
        .and_then(|context| context.channel_id)
        .is_some();
    let channel_id_non_nil = request_context
        .and_then(|context| context.channel_id)
        .map(|value| !value.is_nil());
    let channel_slug_present = request_context
        .and_then(|context| context.channel_slug.as_ref())
        .is_some();
    let channel_slug_length = request_context
        .and_then(|context| context.channel_slug.as_ref())
        .map(|value| value.chars().count());
    let locale_present = request_context
        .map(|context| !context.locale.trim().is_empty())
        .unwrap_or(false);
    let locale_length = request_context.map(|context| context.locale.chars().count());

    if technical {
        tracing::error!(
            error_type,
            owner = "rustok_pricing",
            owner_operation,
            consumer = CART_STOREFRONT_NATIVE_OWNER,
            correlation_id = ?correlation_id,
            request_context_present,
            request_tenant_id_non_nil = ?request_tenant_id_non_nil,
            tenant_id_non_nil,
            cart_id_non_nil,
            line_item_id_non_nil,
            channel_id_present,
            channel_id_non_nil = ?channel_id_non_nil,
            channel_slug_present,
            channel_slug_length = ?channel_slug_length,
            locale_present,
            locale_length = ?locale_length,
            owner_code = %error.code,
            owner_kind = ?error.kind,
            owner_retryable = error.retryable,
            boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
            "cart storefront pricing operation failed"
        );
    } else {
        tracing::warn!(
            error_type,
            owner = "rustok_pricing",
            owner_operation,
            consumer = CART_STOREFRONT_NATIVE_OWNER,
            correlation_id = ?correlation_id,
            request_context_present,
            request_tenant_id_non_nil = ?request_tenant_id_non_nil,
            tenant_id_non_nil,
            cart_id_non_nil,
            line_item_id_non_nil,
            channel_id_present,
            channel_id_non_nil = ?channel_id_non_nil,
            channel_slug_present,
            channel_slug_length = ?channel_slug_length,
            locale_present,
            locale_length = ?locale_length,
            owner_code = %error.code,
            owner_kind = ?error.kind,
            owner_retryable = error.retryable,
            boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
            "cart storefront pricing operation was rejected"
        );
    }
    ServerFnError::new(error.message)
}

fn missing_variant_error(tenant_id: Uuid, cart_id: Uuid, line_item_id: Uuid) -> ServerFnError {
    let tenant_id_non_nil = !tenant_id.is_nil();
    let cart_id_non_nil = !cart_id.is_nil();
    let line_item_id_non_nil = !line_item_id.is_nil();
    tracing::error!(
        owner = "rustok_cart",
        owner_operation = "decrement_line_item",
        consumer = CART_STOREFRONT_NATIVE_OWNER,
        tenant_id_non_nil,
        cart_id_non_nil,
        line_item_id_non_nil,
        code = "cart.storefront_line_item_variant_missing",
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront line item is missing variant identity"
    );
    ServerFnError::new("Cart line item could not be updated safely")
}

pub async fn fetch_storefront_cart_server(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ApiError> {
    storefront_cart_native(selected_cart_id, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_cart(
    request: crate::core::CartFetchRequest,
) -> Result<StorefrontCartData, ApiError> {
    fetch_storefront_cart_server(request.selected_cart_id, request.locale).await
}

pub async fn decrement_storefront_cart_line_item_server(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    storefront_cart_decrement_line_item(cart_id, line_item_id)
        .await
        .map_err(ApiError::from)
}

pub async fn decrement_line_item(
    request: crate::core::CartLineItemDecrementRequest,
) -> Result<(), ApiError> {
    decrement_storefront_cart_line_item_server(request.cart_id, request.line_item_id).await
}

pub async fn remove_storefront_cart_line_item_server(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ApiError> {
    storefront_cart_remove_line_item(cart_id, line_item_id)
        .await
        .map_err(ApiError::from)
}

pub async fn remove_line_item(
    request: crate::core::CartLineItemMutationRequest,
) -> Result<(), ApiError> {
    remove_storefront_cart_line_item_server(request.cart_id, request.line_item_id).await
}

async fn resolve_storefront_customer_id(
    db: sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<rustok_api::AuthContext>,
    request_context: Option<&rustok_api::RequestContext>,
    owner_operation: &'static str,
) -> Result<Option<Uuid>, ServerFnError> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match rustok_customer::CustomerService::new(db)
        .get_customer_by_user(tenant_id, auth.user_id)
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(error) => Err(customer_error(
            error,
            tenant_id,
            auth.user_id,
            owner_operation,
            request_context,
        )),
    }
}

fn ensure_storefront_cart_access(
    cart: &rustok_cart::CartResponse,
    storefront_customer_id: Option<Uuid>,
) -> Result<(), ServerFnError> {
    if let Some(owner_customer_id) = cart.customer_id {
        match storefront_customer_id {
            Some(customer_id) if customer_id == owner_customer_id => Ok(()),
            Some(_) => Err(ServerFnError::new(
                "Cart does not belong to the current storefront customer",
            )),
            None => Err(ServerFnError::new(
                "Authentication required to access this cart",
            )),
        }
    } else {
        Ok(())
    }
}

#[server(prefix = "/api/fn", endpoint = "cart/storefront-data")]
async fn storefront_cart_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let db = runtime_ctx.db_clone();
    let event_bus = transactional_event_bus_from_runtime(&runtime_ctx, "cart/storefront-data")?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(tenant_context_error)?;
    let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
        .await
        .map_err(auth_context_error)?;
    let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .ok();
    let Some((normalized_cart_id, cart_id)) = parse_cart_id(selected_cart_id).map_err(|error| {
        cart_input_error(
            error,
            "parse_storefront_cart_id",
            "cart.storefront_cart_id_invalid",
            "Invalid cart selection",
        )
    })?
    else {
        let _ = locale;
        return Ok(StorefrontCartData {
            selected_cart_id: None,
            cart: None,
        });
    };

    let cart_service = rustok_cart::CartService::new(db.clone());
    let cart = match cart_service.get_cart(tenant.id, cart_id).await {
        Ok(cart) => cart,
        Err(rustok_cart::CartError::CartNotFound(_)) => {
            return Ok(StorefrontCartData {
                selected_cart_id: Some(normalized_cart_id),
                cart: None,
            });
        }
        Err(error) => {
            return Err(cart_error(
                error,
                tenant.id,
                "get_cart",
                Some(cart_id),
                None,
                request_context.as_ref(),
            ));
        }
    };
    let storefront_customer_id = resolve_storefront_customer_id(
        db.clone(),
        tenant.id,
        auth.0,
        request_context.as_ref(),
        "resolve_storefront_customer",
    )
    .await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;
    let cart = reprice_storefront_cart_line_items(
        db,
        event_bus,
        tenant.id,
        &cart_service,
        cart,
        request_context.as_ref(),
    )
    .await?;

    let _ = locale;
    Ok(StorefrontCartData {
        selected_cart_id: Some(normalized_cart_id),
        cart: Some(map_native_cart(cart)),
    })
}

async fn reprice_storefront_cart_line_items(
    db: sea_orm::DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    cart_service: &rustok_cart::CartService,
    cart: rustok_cart::CartResponse,
    request_context: Option<&rustok_api::RequestContext>,
) -> Result<rustok_cart::CartResponse, ServerFnError> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    use rustok_pricing::{PricingReadPort, ResolveProductPriceRequest};

    let pricing_service = rustok_pricing::PricingService::new(db, event_bus);
    let channel_id = cart
        .channel_id
        .or_else(|| request_context.and_then(|context| context.channel_id));
    let channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref()).or_else(|| {
        request_context
            .and_then(|context| normalize_public_channel_slug(context.channel_slug.as_deref()))
    });
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let resolved_price = pricing_service
            .resolve_product_price(
                rustok_api::PortContext::new(
                    tenant_id.to_string(),
                    rustok_api::PortActor::service("rustok-cart.storefront"),
                    "en",
                    format!("cart:{}:reprice", cart.id),
                )
                .with_deadline(std::time::Duration::from_secs(2)),
                ResolveProductPriceRequest {
                    product_id: line_item.product_id,
                    variant_id,
                    region_id: cart.region_id,
                    channel_id,
                    channel_slug: channel_slug.clone(),
                    price_list_id: None,
                    quantity: Some(line_item.quantity),
                    currency_code: cart.currency_code.to_ascii_uppercase(),
                },
            )
            .await
            .map_err(|error| {
                pricing_error(
                    error,
                    tenant_id,
                    "resolve_product_price",
                    cart.id,
                    line_item.id,
                    request_context,
                )
            })?;
        updates.push(storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        Ok(cart)
    } else {
        let cart_id = cart.id;
        cart_service
            .reprice_line_items(tenant_id, cart_id, updates)
            .await
            .map_err(|error| {
                cart_error(
                    error,
                    tenant_id,
                    "reprice_line_items",
                    Some(cart_id),
                    None,
                    request_context,
                )
            })
    }
}

#[server(prefix = "/api/fn", endpoint = "cart/decrement-line-item")]
async fn storefront_cart_decrement_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;
    use rustok_pricing::{PricingReadPort, PricingService, ResolveProductPriceRequest};

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let db = runtime_ctx.db_clone();
    let event_bus = transactional_event_bus_from_runtime(&runtime_ctx, "cart/decrement-line-item")?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(tenant_context_error)?;
    let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
        .await
        .map_err(auth_context_error)?;
    let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .ok();
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id)).map_err(|error| {
        cart_input_error(
            error,
            "parse_decrement_cart_id",
            "cart.storefront_cart_id_invalid",
            "Invalid cart selection",
        )
    })?
    else {
        return Err(ServerFnError::new("Invalid cart selection"));
    };
    let (_, parsed_line_item_id) = parse_line_item_id(line_item_id).map_err(|error| {
        cart_input_error(
            error,
            "parse_decrement_line_item_id",
            "cart.storefront_line_item_id_invalid",
            "Invalid cart line item selection",
        )
    })?;

    let cart_service = rustok_cart::CartService::new(db.clone());
    let cart = cart_service
        .get_cart(tenant.id, parsed_cart_id)
        .await
        .map_err(|error| {
            cart_error(
                error,
                tenant.id,
                "get_cart_for_decrement",
                Some(parsed_cart_id),
                Some(parsed_line_item_id),
                request_context.as_ref(),
            )
        })?;
    let storefront_customer_id = resolve_storefront_customer_id(
        db.clone(),
        tenant.id,
        auth.0,
        request_context.as_ref(),
        "resolve_storefront_customer_for_decrement",
    )
    .await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;

    let line_item = cart
        .line_items
        .iter()
        .find(|item| item.id == parsed_line_item_id)
        .ok_or_else(|| ServerFnError::new("Cart line item was not found"))?;
    if line_item.quantity <= 1 {
        cart_service
            .remove_line_item(tenant.id, parsed_cart_id, parsed_line_item_id)
            .await
            .map_err(|error| {
                cart_error(
                    error,
                    tenant.id,
                    "remove_line_item_for_decrement",
                    Some(parsed_cart_id),
                    Some(parsed_line_item_id),
                    request_context.as_ref(),
                )
            })?;
    } else {
        let next_quantity = line_item.quantity - 1;
        let pricing_service = PricingService::new(db, event_bus);
        let variant_id = line_item
            .variant_id
            .ok_or_else(|| missing_variant_error(tenant.id, parsed_cart_id, parsed_line_item_id))?;
        let resolved_price = pricing_service
            .resolve_product_price(
                rustok_api::PortContext::new(
                    tenant.id.to_string(),
                    rustok_api::PortActor::service("rustok-cart.storefront"),
                    "en",
                    format!("cart:{}:decrement", parsed_cart_id),
                )
                .with_deadline(std::time::Duration::from_secs(2)),
                ResolveProductPriceRequest {
                    product_id: line_item.product_id,
                    variant_id,
                    region_id: cart.region_id,
                    channel_id: cart.channel_id.or_else(|| {
                        request_context
                            .as_ref()
                            .and_then(|context| context.channel_id)
                    }),
                    channel_slug: normalize_public_channel_slug(cart.channel_slug.as_deref())
                        .or_else(|| {
                            request_context.as_ref().and_then(|context| {
                                normalize_public_channel_slug(context.channel_slug.as_deref())
                            })
                        }),
                    price_list_id: None,
                    quantity: Some(next_quantity),
                    currency_code: cart.currency_code.to_ascii_uppercase(),
                },
            )
            .await
            .map_err(|error| {
                pricing_error(
                    error,
                    tenant.id,
                    "resolve_decrement_product_price",
                    parsed_cart_id,
                    parsed_line_item_id,
                    request_context.as_ref(),
                )
            })?;

        let pricing_update =
            storefront_cart_pricing_update(parsed_line_item_id, next_quantity, &resolved_price);
        cart_service
            .update_line_item_pricing(
                tenant.id,
                parsed_cart_id,
                parsed_line_item_id,
                next_quantity,
                pricing_update.unit_price,
                pricing_update.pricing_adjustment,
            )
            .await
            .map_err(|error| {
                cart_error(
                    error,
                    tenant.id,
                    "update_line_item_pricing",
                    Some(parsed_cart_id),
                    Some(parsed_line_item_id),
                    request_context.as_ref(),
                )
            })?;
    }

    Ok(())
}

#[server(prefix = "/api/fn", endpoint = "cart/remove-line-item")]
async fn storefront_cart_remove_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let db = runtime_ctx.db_clone();
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(tenant_context_error)?;
    let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
        .await
        .map_err(auth_context_error)?;
    let Some((_, parsed_cart_id)) = parse_cart_id(Some(cart_id)).map_err(|error| {
        cart_input_error(
            error,
            "parse_remove_cart_id",
            "cart.storefront_cart_id_invalid",
            "Invalid cart selection",
        )
    })?
    else {
        return Err(ServerFnError::new("Invalid cart selection"));
    };
    let (_, parsed_line_item_id) = parse_line_item_id(line_item_id).map_err(|error| {
        cart_input_error(
            error,
            "parse_remove_line_item_id",
            "cart.storefront_line_item_id_invalid",
            "Invalid cart line item selection",
        )
    })?;

    let cart_service = rustok_cart::CartService::new(db.clone());
    let cart = cart_service
        .get_cart(tenant.id, parsed_cart_id)
        .await
        .map_err(|error| {
            cart_error(
                error,
                tenant.id,
                "get_cart_for_remove",
                Some(parsed_cart_id),
                Some(parsed_line_item_id),
                None,
            )
        })?;
    let storefront_customer_id = resolve_storefront_customer_id(
        db,
        tenant.id,
        auth.0,
        None,
        "resolve_storefront_customer_for_remove",
    )
    .await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;

    cart_service
        .remove_line_item(tenant.id, parsed_cart_id, parsed_line_item_id)
        .await
        .map_err(|error| {
            cart_error(
                error,
                tenant.id,
                "remove_line_item",
                Some(parsed_cart_id),
                Some(parsed_line_item_id),
                None,
            )
        })?;
    Ok(())
}
