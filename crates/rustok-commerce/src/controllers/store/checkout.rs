use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::{OptionalAuthContext, RequestContext, TenantContext};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_payment::{PaymentError, PaymentService};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime, StoreCompleteCartInput, StoreCreatePaymentCollectionInput,
};
use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse, PaymentCollectionResponse};

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;

/// Create payment collection from storefront cart
#[utoipa::path(
    post,
    path = "/store/payment-collections",
    tag = "store",
    request_body = StoreCreatePaymentCollectionInput,
    responses(
        (status = 201, description = "Payment collection created", body = PaymentCollectionResponse),
        (status = 400, description = "Cart is completed and cannot create payment collection"),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn create_payment_collection(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Json(input): Json<StoreCreatePaymentCollectionInput>,
) -> HttpResult<(StatusCode, Json<PaymentCollectionResponse>)> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let cart_storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let cart = cart_storefront_port
        .read_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                input.cart_id,
                "read",
                false,
            ),
            CartStorefrontReadRequest {
                cart_id: input.cart_id,
            },
        )
        .await
        .map_err(rustok_web::port_error_to_http_error)?;
    super::ensure_store_cart_access(&cart, customer_id)?;
    super::ensure_cart_allows_payment_collection(&cart)?;
    let cart = super::reprice_storefront_cart_line_items_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        &request_context,
        cart_storefront_port.as_ref(),
        cart,
    )
    .await?;
    let context =
        super::resolve_context_from_cart_for_db(runtime.db(), tenant.id, &request_context, &cart)
            .await?;

    let service = PaymentService::new(runtime.db_clone());
    if let Some(existing) = service
        .find_reusable_collection_by_cart(tenant.id, cart.id)
        .await
        .map_err(|error| {
            payment_collection_http_error(
                tenant.id,
                cart.id,
                "find_reusable_collection_by_cart",
                error,
            )
        })?
    {
        return Ok((StatusCode::OK, Json(existing)));
    }
    let collection = service
        .create_collection(
            tenant.id,
            rustok_payment::dto::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: super::merge_metadata(
                    input.metadata,
                    super::cart_context_metadata(&cart, &context),
                ),
            },
        )
        .await
        .map_err(|error| {
            payment_collection_http_error(tenant.id, cart.id, "create_collection", error)
        })?;

    Ok((StatusCode::CREATED, Json(collection)))
}

/// Complete storefront cart checkout
#[utoipa::path(
    post,
    path = "/store/carts/{id}/complete",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("Idempotency-Key" = String, Header, description = "Stable key for replay-safe checkout")
    ),
    request_body = StoreCompleteCartInput,
    responses(
        (status = 200, description = "Checkout completed", body = CompleteCheckoutResponse),
        (status = 400, description = "Checkout request is invalid"),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found"),
        (status = 409, description = "Checkout key, pricing or domain conflict")
    )
)]
pub async fn complete_cart_checkout(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(cart_id): Path<Uuid>,
    Json(input): Json<StoreCompleteCartInput>,
) -> HttpResult<Json<CompleteCheckoutResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;
    let idempotency_key = required_idempotency_key(&headers)?;
    let actor_id = super::checkout_actor_id(auth.0.as_ref());
    let checkout_input = CompleteCheckoutInput {
        cart_id,
        shipping_option_id: input.shipping_option_id,
        shipping_selections: input.shipping_selections.map(|items| {
            items
                .into_iter()
                .map(|item| crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: item.shipping_profile_slug,
                    seller_id: item.seller_id,
                    seller_scope: None,
                    selected_shipping_option_id: item.selected_shipping_option_id,
                })
                .collect()
        }),
        region_id: input.region_id,
        country_code: input.country_code,
        locale: input.locale,
        create_fulfillment: input.create_fulfillment,
        metadata: input.metadata,
    };
    let storefront_runtime = crate::storefront_checkout_runtime::StorefrontCheckoutRuntime::new(
        runtime.db_clone(),
        runtime.event_bus(),
    );
    let response = crate::services::storefront_staged_checkout_runtime::complete_storefront_checkout_input(
        &storefront_runtime,
        runtime.payment_provider_registry(),
        tenant.id,
        &request_context,
        auth.0,
        idempotency_key,
        checkout_input,
    )
    .await
    .map_err(|error| {
        tracing::error!(
            tenant_id = %tenant.id,
            cart_id = %cart_id,
            actor_id = %actor_id,
            code = error.public_code(),
            retryable = error.retryable(),
            "storefront checkout request failed"
        );
        storefront_checkout_http_error(error)
    })?;

    Ok(Json(response))
}

fn required_idempotency_key(headers: &HeaderMap) -> HttpResult<String> {
    let value = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .ok_or_else(|| {
            HttpError::bad_request(
                "idempotency_key_required",
                "Idempotency-Key header is required for checkout",
            )
        })?
        .to_str()
        .map_err(|_| {
            HttpError::bad_request(
                "idempotency_key_invalid",
                "Idempotency-Key header must be valid ASCII",
            )
        })?
        .trim();

    if value.is_empty() || value.chars().count() > MAX_IDEMPOTENCY_KEY_LENGTH {
        return Err(HttpError::bad_request(
            "idempotency_key_invalid",
            format!("Idempotency-Key must contain 1 to {MAX_IDEMPOTENCY_KEY_LENGTH} characters"),
        ));
    }

    Ok(value.to_string())
}

fn storefront_checkout_http_error(
    error: crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> HttpError {
    use crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError;

    let status = match &error {
        StorefrontStagedCheckoutRuntimeError::Validation(_) => StatusCode::BAD_REQUEST,
        StorefrontStagedCheckoutRuntimeError::CartAccess => StatusCode::NOT_FOUND,
        StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        StorefrontStagedCheckoutRuntimeError::CheckoutFailed => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        StorefrontStagedCheckoutRuntimeError::CompensationPending
        | StorefrontStagedCheckoutRuntimeError::ReconciliationRequired => StatusCode::CONFLICT,
    };
    HttpError::new(status, error.public_code(), error.public_message())
}

fn payment_collection_http_error(
    tenant_id: Uuid,
    cart_id: Uuid,
    operation: &'static str,
    error: PaymentError,
) -> HttpError {
    tracing::error!(
        error = ?error,
        tenant_id = %tenant_id,
        cart_id = %cart_id,
        operation,
        "storefront payment collection operation failed"
    );
    match error {
        PaymentError::Validation(_) => HttpError::bad_request(
            "payment_request_invalid",
            "Payment collection request is invalid",
        ),
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => HttpError::not_found(
            "payment_resource_not_found",
            "Payment resource was not found",
        ),
        PaymentError::InvalidTransition { .. } => HttpError::new(
            StatusCode::CONFLICT,
            "payment_state_conflict",
            "Payment lifecycle conflicts with the requested operation",
        ),
        PaymentError::ProviderUnavailable { .. } | PaymentError::ProviderConfiguration { .. } => {
            HttpError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "payment_temporarily_unavailable",
                "Payment service is temporarily unavailable",
            )
        }
        PaymentError::ProviderRejected { .. } => HttpError::new(
            StatusCode::CONFLICT,
            "payment_provider_rejected",
            "Payment provider rejected the requested operation",
        ),
        PaymentError::ProviderInvalidResponse { .. }
        | PaymentError::ProviderOutcomeUnknown { .. } => HttpError::new(
            StatusCode::CONFLICT,
            "payment_reconciliation_required",
            "Payment operation requires reconciliation",
        ),
        PaymentError::Database(_) => HttpError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_storage_unavailable",
            "Payment service is temporarily unavailable",
        ),
    }
}
