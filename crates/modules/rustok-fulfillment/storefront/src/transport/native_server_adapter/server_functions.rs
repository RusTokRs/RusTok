use leptos::prelude::*;
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use super::super::build_shipping_selection_updates;
use super::super::{SelectShippingOptionRequest, ShippingSelectionTransportError};

#[cfg(feature = "ssr")]
const FULFILLMENT_STOREFRONT_NATIVE_OWNER: &str = "rustok_fulfillment.storefront";
#[cfg(feature = "ssr")]
const FULFILLMENT_STOREFRONT_NATIVE_OPERATION: &str = "select_storefront_shipping_option";
#[cfg(feature = "ssr")]
const FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY: &str = "fulfillment_storefront_native_transport";

#[cfg(feature = "ssr")]
fn map_runtime_dependency_error(dependency: &'static str) -> ServerFnError {
    tracing::error!(
        owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION,
        dependency,
        code = "fulfillment.storefront_runtime_unavailable",
        boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY,
        "fulfillment storefront runtime dependency is unavailable"
    );
    ServerFnError::new("Shipping selection is temporarily unavailable")
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error<E>(_error: E) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION,
        code = "fulfillment.storefront_tenant_context_unavailable",
        boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY,
        "fulfillment storefront tenant context extraction failed"
    );
    ServerFnError::new("Shipping selection context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_auth_context_error<E>(tenant_id: Uuid, _error: E) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION,
        tenant_id_non_nil = !tenant_id.is_nil(),
        code = "fulfillment.storefront_auth_context_unavailable",
        boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY,
        "fulfillment storefront authentication context extraction failed"
    );
    ServerFnError::new("Shipping selection context is unavailable")
}

#[cfg(feature = "ssr")]
fn record_optional_request_context_error<E>(tenant_id: Uuid, _error: E) {
    let error_type = std::any::type_name::<E>();
    tracing::warn!(
        error_type,
        owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION,
        tenant_id_non_nil = !tenant_id.is_nil(),
        request_context_present = false,
        code = "fulfillment.storefront_request_context_unavailable",
        boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY,
        "optional fulfillment storefront request context extraction failed"
    );
}

#[cfg(feature = "ssr")]
fn map_owner_runtime_error<E>(
    request_context: Option<&rustok_api::RequestContext>,
    tenant_id: Uuid,
    _error: E,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    if let Some(request_context) = request_context {
        tracing::error!(
            error_type,
            owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER,
            owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION,
            tenant_id_non_nil = !tenant_id.is_nil(),
            request_context_present = true,
            correlation_id = %request_context.correlation_id,
            channel_id_present = request_context.channel_id.is_some(),
            channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil()),
            channel_slug_present = request_context.channel_slug.is_some(),
            channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count()),
            locale_present = !request_context.locale.trim().is_empty(),
            locale_length = request_context.locale.chars().count(),
            code = "fulfillment.storefront_shipping_selection_failed",
            boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY,
            "fulfillment storefront owner runtime call failed"
        );
    } else {
        tracing::error!(
            error_type,
            owner = FULFILLMENT_STOREFRONT_NATIVE_OWNER,
            owner_operation = FULFILLMENT_STOREFRONT_NATIVE_OPERATION,
            tenant_id_non_nil = !tenant_id.is_nil(),
            request_context_present = false,
            code = "fulfillment.storefront_shipping_selection_failed",
            boundary = FULFILLMENT_STOREFRONT_NATIVE_BOUNDARY,
            "fulfillment storefront owner runtime call failed without request context"
        );
    }
    ServerFnError::new("Shipping selection is temporarily unavailable")
}

pub async fn select_shipping_option_server(
    request: SelectShippingOptionRequest,
) -> Result<(), ShippingSelectionTransportError> {
    storefront_fulfillment_select_shipping_option_native(request)
        .await
        .map_err(|error| ShippingSelectionTransportError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "fulfillment/select-shipping-option")]
async fn storefront_fulfillment_select_shipping_option_native(
    request: SelectShippingOptionRequest,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_commerce::storefront_checkout_runtime::{
            self, StorefrontShippingSelectionCommand, StorefrontShippingSelectionUpdateInput,
        };
        use rustok_outbox::TransactionalEventBus;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| map_runtime_dependency_error("TransactionalEventBus"))?;
        let runtime = storefront_checkout_runtime::StorefrontCheckoutRuntime::new(
            runtime_ctx.db_clone(),
            event_bus,
        );
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(map_tenant_context_error)?;
        let tenant_id = tenant.id;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(|error| map_auth_context_error(tenant_id, error))?;
        let request_context = match leptos_axum::extract::<rustok_api::RequestContext>().await {
            Ok(request_context) => Some(request_context),
            Err(error) => {
                record_optional_request_context_error(tenant_id, error);
                None
            }
        };
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("cart_id must be a valid UUID"))?;

        let shipping_selections = build_shipping_selection_updates(&request)
            .map_err(|error| ServerFnError::new(error.message().to_string()))?
            .into_iter()
            .map(|selection| {
                Ok(StorefrontShippingSelectionUpdateInput {
                    shipping_profile_slug: selection.shipping_profile_slug,
                    seller_id: selection.seller_id,
                    selected_shipping_option_id: parse_optional_uuid(
                        selection.selected_shipping_option_id,
                        "selected_shipping_option_id",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?;

        storefront_checkout_runtime::select_storefront_shipping_option(
            &runtime,
            &tenant,
            request_context.as_ref(),
            auth,
            StorefrontShippingSelectionCommand {
                cart_id,
                shipping_selections,
            },
        )
        .await
        .map_err(|error| map_owner_runtime_error(request_context.as_ref(), tenant_id, error))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "fulfillment/select-shipping-option requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<Uuid>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            Uuid::parse_str(value.trim())
                .map_err(|_| ServerFnError::new(format!("{field_name} must be a valid UUID")))
        })
        .transpose()
}
