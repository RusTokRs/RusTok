//! Real native buyer commands. GraphQL remains a separately selected adapter, never a fallback.

use super::native_server_adapter::ApiError;
use crate::core::{CartAddVariantRequest, CartCreateRequest};
use crate::model::StorefrontCart;
use leptos::prelude::*;

pub(super) async fn create_cart(request: CartCreateRequest) -> Result<StorefrontCart, ApiError> {
    storefront_cart_create_native(request)
        .await
        .map_err(ApiError::from)
}

pub(super) async fn add_variant(
    request: CartAddVariantRequest,
) -> Result<StorefrontCart, ApiError> {
    storefront_cart_add_variant_native(request)
        .await
        .map_err(ApiError::from)
}

#[cfg(feature = "ssr")]
pub(super) fn port_error(error: rustok_api::PortError) -> ServerFnError {
    let technical = matches!(
        error.kind,
        rustok_api::PortErrorKind::Unavailable
            | rustok_api::PortErrorKind::Timeout
            | rustok_api::PortErrorKind::InvariantViolation
    );
    if technical {
        tracing::error!(boundary = "cart_storefront_buyer_native", kind = ?error.kind,
            retryable = error.retryable, code_length = error.code.len(), "native cart buyer operation failed");
    } else {
        tracing::warn!(boundary = "cart_storefront_buyer_native", kind = ?error.kind,
            retryable = error.retryable, code_length = error.code.len(), "native cart buyer operation was refused");
    }
    ServerFnError::new(
        "Cart operation could not be confirmed. Inspect the cart before trying again.",
    )
}

#[cfg(feature = "ssr")]
pub(super) struct BuyerScope {
    pub(super) host: rustok_api::HostRuntimeContext,
    pub(super) tenant: rustok_api::TenantContext,
    auth: Option<rustok_api::AuthContext>,
    pub(super) request: rustok_api::RequestContext,
    pub(super) customer_id: Option<uuid::Uuid>,
}

#[cfg(feature = "ssr")]
pub(super) async fn buyer_scope(operation: &'static str) -> Result<BuyerScope, ServerFnError> {
    use super::native_server_adapter_ssr::{
        auth_context_error, resolve_storefront_customer_id, tenant_context_error,
    };
    let host = use_context::<rustok_api::HostRuntimeContext>()
        .ok_or_else(|| ServerFnError::new("Cart runtime is unavailable"))?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(tenant_context_error)?;
    let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
        .await
        .map_err(auth_context_error)?;
    let request = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .map_err(|_| ServerFnError::new("Cart request context is unavailable"))?;
    if tenant.id.is_nil()
        || request.tenant_id != tenant.id
        || auth
            .0
            .as_ref()
            .is_some_and(|auth| auth.tenant_id != tenant.id || auth.user_id.is_nil())
        || request
            .user_id
            .is_some_and(|id| auth.0.as_ref().map(|auth| auth.user_id) != Some(id))
    {
        return Err(ServerFnError::new("Cart request context is unavailable"));
    }
    rustok_commerce::admit_storefront_cart_request(host.db(), tenant.id, &request)
        .await
        .map_err(port_error)?;
    let customer_id = resolve_storefront_customer_id(
        host.db_clone(),
        tenant.id,
        auth.0.clone(),
        Some(&request),
        operation,
    )
    .await?;
    Ok(BuyerScope {
        host,
        tenant,
        auth: auth.0,
        request,
        customer_id,
    })
}

#[cfg(feature = "ssr")]
pub(super) fn port_context(scope: &BuyerScope, operation: &'static str) -> rustok_api::PortContext {
    let actor = scope
        .auth
        .as_ref()
        .map(|auth| rustok_api::PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| rustok_api::PortActor::service("rustok-cart.storefront"));
    let mut context = rustok_api::PortContext::new(
        scope.tenant.id.to_string(),
        actor,
        scope.request.locale.clone(),
        scope.request.correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2))
    .with_idempotency_key(format!("cart-buyer:{operation}:{}", uuid::Uuid::new_v4()));
    context.channel = scope.request.channel_slug.clone();
    if let Some(auth) = scope.auth.as_ref() {
        context.claims.extend(auth.scopes.iter().cloned());
        context
            .claims
            .extend(auth.permissions.iter().map(ToString::to_string));
    }
    context
}

#[server(prefix = "/api/fn", endpoint = "cart/storefront-create", input = leptos::server_fn::codec::Json, client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_cart_create_native(
    request: CartCreateRequest,
) -> Result<StorefrontCart, ServerFnError> {
    crate::core::validate_cart_create(&request)
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    #[cfg(feature = "ssr")]
    {
        use rustok_cart::{CartStorefrontCreateRequest, in_process_cart_storefront_port};
        let scope = buyer_scope("create_cart").await?;
        let region_id = request
            .region_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ServerFnError::new("Invalid cart region"))?;
        let context = rustok_commerce::StoreContextService::new(
            scope.host.db_clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(scope.host.db_clone())),
        )
        .resolve_context(
            scope.tenant.id,
            rustok_commerce::dto::ResolveStoreContextInput {
                region_id,
                country_code: None,
                locale: request
                    .locale
                    .or_else(|| Some(scope.request.locale.clone())),
                currency_code: Some(request.currency_code),
            },
        )
        .await
        .map_err(|_| {
            tracing::warn!(
                boundary = "cart_storefront_buyer_native",
                operation = "resolve_create_context",
                "cart context was refused"
            );
            ServerFnError::new("Cart region, currency or locale could not be resolved")
        })?;
        let currency_code = context
            .currency_code
            .ok_or_else(|| ServerFnError::new("Cart currency is required"))?;
        let cart = in_process_cart_storefront_port(scope.host.db_clone())
            .create_storefront_cart(
                port_context(&scope, "create"),
                CartStorefrontCreateRequest {
                    input: rustok_cart::CreateCartInput {
                        customer_id: scope.customer_id,
                        email: None,
                        region_id: context.region.map(|region| region.id),
                        country_code: None,
                        locale_code: Some(context.locale),
                        selected_shipping_option_id: None,
                        currency_code,
                        metadata: serde_json::json!({}),
                    },
                    channel_id: scope.request.channel_id,
                    channel_slug: scope.request.channel_slug.clone(),
                },
            )
            .await
            .map_err(port_error)?;
        // The owner port records the issued guest capability in the canonical HTTP request scope.
        // No browser token store or token-bearing cart metadata is created by this adapter.
        Ok(super::native_server_mapping::map_native_cart(cart))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native cart commands require the SSR server",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "cart/storefront-add-variant", input = leptos::server_fn::codec::Json, client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_cart_add_variant_native(
    request: CartAddVariantRequest,
) -> Result<StorefrontCart, ServerFnError> {
    crate::core::validate_cart_add(&request)
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    #[cfg(feature = "ssr")]
    {
        use rustok_commerce::storefront_cart_runtime::{
            StorefrontCartAddCommand, StorefrontCartRuntime,
        };
        let scope = buyer_scope("add_variant").await?;
        let events = super::native_server_adapter_ssr::transactional_event_bus_from_runtime(
            &scope.host,
            "cart/storefront-add-variant",
        )?;
        let product = scope
            .host
            .shared_get::<rustok_product::ProductCatalogReadRuntime>()
            .unwrap_or_else(|| {
                rustok_product::ProductCatalogReadRuntime::in_process(
                    scope.host.db_clone(),
                    events.clone(),
                )
            });
        let runtime = StorefrontCartRuntime::new(
            product.read_port(),
            rustok_pricing::in_process_pricing_read_port(
                scope.host.db_clone(),
                events.clone(),
                product,
            ),
            rustok_cart::in_process_cart_storefront_port(scope.host.db_clone()),
            rustok_inventory::in_process_inventory_reservation_port(scope.host.db_clone(), events),
        );
        let parse = |value: &str| {
            uuid::Uuid::parse_str(value.trim())
                .map_err(|_| ServerFnError::new("Invalid cart buyer identity"))
        };
        let expected = request.expected_context;
        let context = rustok_pricing::PriceResolutionContext {
            currency_code: expected.currency_code,
            region_id: expected.region_id.as_deref().map(parse).transpose()?,
            price_list_id: expected.price_list_id.as_deref().map(parse).transpose()?,
            channel_id: expected.channel_id.as_deref().map(parse).transpose()?,
            channel_slug: expected.channel_slug,
            quantity: Some(expected.quantity),
        };
        let cart = runtime
            .add_line_item(
                port_context(&scope, "add"),
                scope.customer_id,
                scope.tenant.default_locale.as_str(),
                StorefrontCartAddCommand {
                    cart_id: parse(&request.cart_id)?,
                    variant_id: parse(&request.variant_id)?,
                    quantity: request.quantity,
                    expected_context: Some(context),
                    expected_locale: request.locale,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map_err(port_error)?;
        Ok(super::native_server_mapping::map_native_cart(cart))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native cart commands require the SSR server",
        ))
    }
}
