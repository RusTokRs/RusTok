use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::{Value, json};
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use super::super::CheckoutAdjustment;
use super::super::{CheckoutCompletion, CheckoutCompletionTransportError, CompleteCheckoutRequest};

pub async fn complete_checkout_server(
    request: CompleteCheckoutRequest,
) -> Result<CheckoutCompletion, CheckoutCompletionTransportError> {
    storefront_order_complete_checkout_native(request)
        .await
        .map_err(|error| {
            tracing::error!(
                error = ?error,
                operation = "complete_checkout_server",
                "native checkout transport failed"
            );
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

        let completion = storefront_staged_checkout_runtime::complete_storefront_checkout(
            &runtime,
            payment_provider_registry,
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
        .map_err(native_checkout_runtime_error)?;

        Ok(map_checkout_completion(completion))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new("Checkout service is unavailable"))
    }
}

#[cfg(feature = "ssr")]
fn native_context_error(operation: &'static str, error: impl std::fmt::Display) -> ServerFnError {
    tracing::error!(
        error = %error,
        operation,
        "native checkout request context extraction failed"
    );
    ServerFnError::new("Checkout request context is unavailable")
}

#[cfg(feature = "ssr")]
fn native_checkout_runtime_error(
    error: rustok_commerce::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> ServerFnError {
    ServerFnError::new(format!(
        "{}: {}",
        error.public_code(),
        error.public_message()
    ))
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
