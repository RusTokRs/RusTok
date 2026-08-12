use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::{Value, json};
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use super::super::CheckoutAdjustment;
use super::super::{CheckoutCompletion, CheckoutCompletionTransportError, CompleteCheckoutRequest};
use super::native_client_error_safety::NativeClientDiagnosticContext;

#[cfg(feature = "ssr")]
const ORDER_STOREFRONT_NATIVE_OWNER: &str = "rustok_order.storefront";
#[cfg(feature = "ssr")]
const ORDER_STOREFRONT_NATIVE_BOUNDARY: &str = "order_storefront_native_transport";

pub async fn complete_checkout_server(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    let context = NativeClientDiagnosticContext::new(&request);
    storefront_order_complete_checkout_native(request)
        .await
        .map_err(|error| {
            context.record_error(&error);
            CheckoutCompletionTransportError::ServerFn(
                "Checkout transport is temporarily unavailable".to_string(),
            )
        })
}

#[server(prefix = "/api/fn", endpoint = "order/complete-checkout")]
async fn storefront_order_complete_checkout_native(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_commerce::services::storefront_staged_checkout_runtime;
        use rustok_commerce::storefront_checkout_runtime::{
            StorefrontCheckoutCompletionCommand, StorefrontCheckoutRuntime,
        };
        use rustok_outbox::TransactionalEventBus;
        use rustok_payment::providers::PaymentProviderRegistry;
        use rustok_product::ProductCatalogReadRuntime;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                tracing::error!(
                    operation = "complete_storefront_checkout",
                    dependency = "TransactionalEventBus",
                    "native checkout runtime dependency is missing"
                );
                ServerFnError::new("Checkout service is temporarily unavailable")
            })?;
        let payment_provider_registry = runtime_ctx
            .shared_get::<PaymentProviderRegistry>()
            .unwrap_or_else(PaymentProviderRegistry::with_manual_provider);
        let product_catalog_read_port = runtime_ctx
            .shared_get::<ProductCatalogReadRuntime>()
            .ok_or_else(|| {
                tracing::error!(
                    operation = "complete_storefront_checkout",
                    dependency = "ProductCatalogReadRuntime",
                    "native checkout runtime dependency is missing"
                );
                ServerFnError::new("Checkout service is temporarily unavailable")
            })?
            .read_port();
        let runtime = StorefrontCheckoutRuntime::new(runtime_ctx.db_clone(), event_bus);
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|error| native_context_error("extract_request_context", error))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| native_context_error("extract_tenant_context", error))?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(|error| native_context_error("extract_auth_context", error))?;
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("Checkout request is invalid"))?;
        let idempotency_key = request.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() || idempotency_key.len() > 191 {
            return Err(ServerFnError::new("Checkout request is invalid"));
        }
        let metadata = request.metadata;
        let correlation_id = Uuid::new_v4();

        let completion =
            storefront_staged_checkout_runtime::complete_storefront_checkout_with_product_port(
                &runtime,
                payment_provider_registry,
                product_catalog_read_port,
                &tenant,
                &request_context,
                auth,
                idempotency_key,
                StorefrontCheckoutCompletionCommand {
                    cart_id,
                    create_fulfillment: metadata.create_fulfillment,
                    metadata: json!({
                        "source_module": metadata.source_module,
                        "source_surface": metadata.source_surface,
                        "command": metadata.command,
                        "owner_module": metadata.owner_module,
                        "create_fulfillment": metadata.create_fulfillment,
                    }),
                },
            )
            .await
            .map_err(|error| {
                native_checkout_runtime_error(&request_context, tenant.id, correlation_id, error)
            })?;

        Ok(map_checkout_completion(completion))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new("Checkout service is unavailable"))
    }
}

#[cfg(feature = "ssr")]
fn native_context_error<E>(operation: &'static str, _error: E) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = ORDER_STOREFRONT_NATIVE_OWNER,
        owner_operation = operation,
        code = "order.storefront_context_unavailable",
        boundary = ORDER_STOREFRONT_NATIVE_BOUNDARY,
        "native checkout request context extraction failed"
    );
    ServerFnError::new("Checkout request context is unavailable")
}

#[cfg(feature = "ssr")]
fn native_checkout_runtime_error(
    request_context: &rustok_api::RequestContext,
    tenant_id: Uuid,
    correlation_id: Uuid,
    error: rustok_commerce::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> ServerFnError {
    let error_type = std::any::type_name_of_val(&error);
    let public_code = error.public_code();
    let public_message = error.public_message();
    let public_retryable = error.retryable();
    tracing::error!(
        error_type,
        owner = ORDER_STOREFRONT_NATIVE_OWNER,
        owner_operation = "complete_storefront_checkout",
        correlation_id = %correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        channel_id_present = request_context.channel_id.is_some(),
        channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil()),
        channel_slug_present = request_context.channel_slug.is_some(),
        channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count()),
        locale_present = !request_context.locale.trim().is_empty(),
        locale_length = request_context.locale.chars().count(),
        public_code = %public_code,
        public_retryable,
        code = "order.storefront_checkout_runtime_failed",
        boundary = ORDER_STOREFRONT_NATIVE_BOUNDARY,
        "order storefront checkout runtime failed"
    );
    ServerFnError::new(format!("{public_code}: {public_message}"))
}

#[cfg(feature = "ssr")]
fn map_checkout_completion(
    value: rustok_commerce::dto::CompleteCheckoutResponse,
) -> CheckoutCompletion {
    let adjustments = value
        .order
        .adjustments
        .into_iter()
        .map(|adjustment| CheckoutAdjustment {
            id: adjustment.id.to_string(),
            line_item_id: adjustment.line_item_id.map(|value| value.to_string()),
            source_type: adjustment.source_type,
            source_id: adjustment.source_id,
            scope: adjustment
                .metadata
                .get("scope")
                .and_then(Value::as_str)
                .map(str::to_string),
            amount: adjustment.amount.normalize().to_string(),
            currency_code: adjustment.currency_code,
            metadata: adjustment.metadata.to_string(),
        })
        .collect::<Vec<_>>();

    CheckoutCompletion {
        order_id: value.order.id.to_string(),
        order_status: value.order.status,
        currency_code: value.order.currency_code,
        shipping_total: value.order.shipping_total.normalize().to_string(),
        adjustment_total: value.order.adjustment_total.normalize().to_string(),
        total_amount: value.order.total_amount.normalize().to_string(),
        adjustments,
        payment_collection_id: value.payment_collection.id.to_string(),
        payment_collection_status: value.payment_collection.status,
        fulfillment_count: value.fulfillments.len() as u64,
        context_locale: value.context.locale,
        context_currency_code: value.context.currency_code,
    }
}
