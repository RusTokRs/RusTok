use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::json;
#[cfg(feature = "ssr")]
use uuid::Uuid;

use super::super::{
    PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionFetchRequest,
    PaymentTransportError, RefundSummary, RefundSummaryFetchRequest,
};

pub async fn fetch_refund_summary_server(
    request: RefundSummaryFetchRequest,
) -> Result<RefundSummary, PaymentTransportError> {
    storefront_refund_summary_native(request)
        .await
        .map_err(|error| PaymentTransportError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "payment/refund-summary")]
async fn storefront_refund_summary_native(
    request: RefundSummaryFetchRequest,
) -> Result<RefundSummary, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_commerce::storefront_checkout_runtime;

        let runtime = checkout_runtime()?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let order_id = Uuid::parse_str(request.order_id.trim())
            .map_err(|_| ServerFnError::new("order_id must be a valid UUID"))?;

        let (items, total) = storefront_checkout_runtime::read_storefront_order_refunds(
            &runtime,
            &tenant,
            &request_context,
            auth,
            order_id,
        )
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(summarize_native_refunds(items, total))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "payment/refund-summary requires the `ssr` feature",
        ))
    }
}

pub async fn fetch_payment_collection_server(
    request: PaymentCollectionFetchRequest,
) -> Result<Option<PaymentCollection>, PaymentTransportError> {
    storefront_payment_collection_native(request)
        .await
        .map_err(|error| PaymentTransportError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "payment/payment-collection")]
async fn storefront_payment_collection_native(
    request: PaymentCollectionFetchRequest,
) -> Result<Option<PaymentCollection>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_commerce::storefront_checkout_runtime;

        let runtime = checkout_runtime()?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("cart_id must be a valid UUID"))?;

        storefront_checkout_runtime::read_storefront_payment_collection(
            &runtime, &tenant, auth, cart_id,
        )
        .await
        .map(|collection| collection.map(map_payment_collection))
        .map_err(|error| ServerFnError::new(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "payment/payment-collection requires the `ssr` feature",
        ))
    }
}

pub async fn create_payment_collection_server(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentTransportError> {
    storefront_payment_create_collection_native(request)
        .await
        .map_err(|error| PaymentTransportError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "payment/create-payment-collection")]
async fn storefront_payment_create_collection_native(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_commerce::storefront_checkout_runtime::{
            self, StorefrontPaymentCollectionCommand,
        };

        let runtime = checkout_runtime()?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("cart_id must be a valid UUID"))?;
        let metadata = request.metadata;

        let collection = storefront_checkout_runtime::create_storefront_payment_collection(
            &runtime,
            &tenant,
            &request_context,
            auth,
            StorefrontPaymentCollectionCommand {
                cart_id,
                metadata: json!({
                    "source_module": metadata.source_module,
                    "source_surface": metadata.source_surface,
                    "command": metadata.command,
                    "owner_module": metadata.owner_module,
                }),
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(map_payment_collection(collection))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "payment/create-payment-collection requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn checkout_runtime()
-> Result<rustok_commerce::storefront_checkout_runtime::StorefrontCheckoutRuntime, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;
    use rustok_outbox::TransactionalEventBus;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let event_bus = runtime_ctx
        .shared_get::<TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new(
                "payment storefront native transport requires TransactionalEventBus in host runtime context",
            )
        })?;

    Ok(
        rustok_commerce::storefront_checkout_runtime::StorefrontCheckoutRuntime::new(
            runtime_ctx.db_clone(),
            event_bus,
        ),
    )
}

#[cfg(feature = "ssr")]
fn map_payment_collection(
    value: rustok_payment::dto::PaymentCollectionResponse,
) -> PaymentCollection {
    PaymentCollection {
        id: value.id.to_string(),
        status: value.status,
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        authorized_amount: value.authorized_amount.normalize().to_string(),
        captured_amount: value.captured_amount.normalize().to_string(),
        order_id: value.order_id.map(|value| value.to_string()),
        provider_id: value.provider_id,
        payment_count: value.payments.len() as u64,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn summarize_native_refunds(
    items: Vec<rustok_payment::dto::RefundResponse>,
    total: u64,
) -> RefundSummary {
    let refunded_amount = items
        .iter()
        .map(|item| item.amount)
        .fold(rust_decimal::Decimal::ZERO, |acc, value| acc + value);
    RefundSummary {
        total,
        refunded_amount: (total > 0).then(|| refunded_amount.normalize().to_string()),
        latest_status: items.first().map(|item| item.status.clone()),
    }
}
