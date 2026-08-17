use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::json;
#[cfg(feature = "ssr")]
use uuid::Uuid;

use super::super::{
    PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionFetchRequest,
    PaymentTransportError, RefundSummary, RefundSummaryFetchRequest,
};

#[cfg(feature = "ssr")]
const PAYMENT_STOREFRONT_NATIVE_OWNER: &str = "rustok_payment.storefront";
#[cfg(feature = "ssr")]
const PAYMENT_STOREFRONT_NATIVE_BOUNDARY: &str = "payment_storefront_native_transport";

#[cfg(feature = "ssr")]
fn map_request_context_error<E>(owner_operation: &'static str, _error: E) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = PAYMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        code = "payment.storefront_request_context_unavailable",
        boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY,
        "payment storefront request context extraction failed"
    );
    ServerFnError::new("Payment storefront request context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error<E>(
    request_context: &rustok_api::RequestContext,
    owner_operation: &'static str,
    _error: E,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = PAYMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        channel_id_present = request_context.channel_id.is_some(),
        channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil()),
        channel_slug_present = request_context.channel_slug.is_some(),
        channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count()),
        locale_present = !request_context.locale.trim().is_empty(),
        locale_length = request_context.locale.chars().count(),
        code = "payment.storefront_tenant_context_unavailable",
        boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY,
        "payment storefront tenant context extraction failed"
    );
    ServerFnError::new("Payment storefront tenant context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_auth_context_error<E>(
    request_context: &rustok_api::RequestContext,
    tenant_id: Uuid,
    owner_operation: &'static str,
    _error: E,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = PAYMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        tenant_id_non_nil = !tenant_id.is_nil(),
        channel_id_present = request_context.channel_id.is_some(),
        channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil()),
        channel_slug_present = request_context.channel_slug.is_some(),
        channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count()),
        locale_present = !request_context.locale.trim().is_empty(),
        locale_length = request_context.locale.chars().count(),
        code = "payment.storefront_auth_context_unavailable",
        boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY,
        "payment storefront authentication context extraction failed"
    );
    ServerFnError::new("Payment storefront authentication context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_owner_runtime_error<E>(
    request_context: &rustok_api::RequestContext,
    tenant_id: Uuid,
    owner_operation: &'static str,
    code: &'static str,
    public_message: &'static str,
    _error: E,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = PAYMENT_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        tenant_id_non_nil = !tenant_id.is_nil(),
        channel_id_present = request_context.channel_id.is_some(),
        channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil()),
        channel_slug_present = request_context.channel_slug.is_some(),
        channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count()),
        locale_present = !request_context.locale.trim().is_empty(),
        locale_length = request_context.locale.chars().count(),
        code,
        boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY,
        "payment storefront owner runtime call failed"
    );
    ServerFnError::new(public_message)
}

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

        let owner_operation = "read_storefront_order_refunds";
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|error| map_request_context_error(owner_operation, error))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| map_tenant_context_error(&request_context, owner_operation, error))?;
        let tenant_id = tenant.id;
        let runtime = checkout_runtime(&request_context, tenant_id, owner_operation)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(|error| {
                map_auth_context_error(&request_context, tenant_id, owner_operation, error)
            })?;
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
        .map_err(|error| {
            map_owner_runtime_error(
                &request_context,
                tenant_id,
                owner_operation,
                "payment.storefront_refund_summary_unavailable",
                "Storefront refund summary is temporarily unavailable",
                error,
            )
        })?;

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

        let owner_operation = "read_storefront_payment_collection";
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|error| map_request_context_error(owner_operation, error))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| map_tenant_context_error(&request_context, owner_operation, error))?;
        let tenant_id = tenant.id;
        let runtime = checkout_runtime(&request_context, tenant_id, owner_operation)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(|error| {
                map_auth_context_error(&request_context, tenant_id, owner_operation, error)
            })?;
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("cart_id must be a valid UUID"))?;

        storefront_checkout_runtime::read_storefront_payment_collection(
            &runtime, &tenant, auth, cart_id,
        )
        .await
        .map(|collection| collection.map(map_payment_collection))
        .map_err(|error| {
            map_owner_runtime_error(
                &request_context,
                tenant_id,
                owner_operation,
                "payment.storefront_collection_unavailable",
                "Storefront payment collection is temporarily unavailable",
                error,
            )
        })
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

        let owner_operation = "create_storefront_payment_collection";
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|error| map_request_context_error(owner_operation, error))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| map_tenant_context_error(&request_context, owner_operation, error))?;
        let tenant_id = tenant.id;
        let runtime = checkout_runtime(&request_context, tenant_id, owner_operation)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(|error| {
                map_auth_context_error(&request_context, tenant_id, owner_operation, error)
            })?;
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
        .map_err(|error| {
            map_owner_runtime_error(
                &request_context,
                tenant_id,
                owner_operation,
                "payment.storefront_collection_create_failed",
                "Storefront payment collection is temporarily unavailable",
                error,
            )
        })?;

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
fn checkout_runtime(
    request_context: &rustok_api::RequestContext,
    tenant_id: Uuid,
    owner_operation: &'static str,
) -> Result<rustok_commerce::storefront_checkout_runtime::StorefrontCheckoutRuntime, ServerFnError>
{
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;
    use rustok_outbox::TransactionalEventBus;

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let event_bus = runtime_ctx
        .shared_get::<TransactionalEventBus>()
        .ok_or_else(|| {
            tracing::error!(
                owner = PAYMENT_STOREFRONT_NATIVE_OWNER,
                owner_operation,
                tenant_id_non_nil = !tenant_id.is_nil(),
                channel_id_present = request_context.channel_id.is_some(),
                channel_id_non_nil = ?request_context.channel_id.map(|value| !value.is_nil()),
                channel_slug_present = request_context.channel_slug.is_some(),
                channel_slug_length = ?request_context.channel_slug.as_ref().map(|value| value.chars().count()),
                locale_present = !request_context.locale.trim().is_empty(),
                locale_length = request_context.locale.chars().count(),
                code = "payment.storefront_runtime_unavailable",
                boundary = PAYMENT_STOREFRONT_NATIVE_BOUNDARY,
                "payment storefront TransactionalEventBus is unavailable"
            );
            ServerFnError::new("Payment storefront runtime is temporarily unavailable")
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
