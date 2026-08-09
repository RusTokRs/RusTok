use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::{
    AuthContext, OptionalAuthContext, PortActor, PortContext, PortError, PortErrorKind,
    RequestContext, TenantContext,
};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_payment::{
    PaymentCollectionCreateOrReuseRequest, ReusablePaymentCollectionByCartRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime, StoreCompleteCartInput, StoreCreatePaymentCollectionInput,
};
use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse, PaymentCollectionResponse};

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const STOREFRONT_CHECKOUT_OWNER: &str = "rustok_commerce.storefront_staged_checkout_runtime";
const STOREFRONT_CHECKOUT_BOUNDARY: &str = "commerce_storefront_checkout_http";
const STOREFRONT_PAYMENT_COLLECTION_OWNER: &str = "rustok_payment.payment_collection_ports";
const STOREFRONT_PAYMENT_COLLECTION_BOUNDARY: &str =
    "commerce_storefront_payment_collection_http";

type StorefrontCheckoutHttpPolicy = (StatusCode, &'static str);
type StorefrontPaymentCollectionHttpPolicy =
    (StatusCode, &'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct StorefrontCheckoutErrorContext {
    tenant_id_non_nil: bool,
    actor_id_non_nil: bool,
    cart_id_non_nil: bool,
    channel_id_present: bool,
    channel_id_non_nil: Option<bool>,
    channel_slug_present: bool,
    channel_slug_length: Option<usize>,
    locale_length: usize,
    operation: &'static str,
}

impl StorefrontCheckoutErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        cart_id: Uuid,
        request_context: &RequestContext,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id_non_nil: !tenant_id.is_nil(),
            actor_id_non_nil: !actor_id.is_nil(),
            cart_id_non_nil: !cart_id.is_nil(),
            channel_id_present: request_context.channel_id.is_some(),
            channel_id_non_nil: request_context.channel_id.map(|value| !value.is_nil()),
            channel_slug_present: request_context.channel_slug.is_some(),
            channel_slug_length: request_context
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            locale_length: request_context.locale.chars().count(),
            operation,
        }
    }
}

#[derive(Clone, Copy)]
struct StorefrontPaymentCollectionErrorContext {
    tenant_id_non_nil: bool,
    actor_id_non_nil: bool,
    cart_id_non_nil: bool,
    customer_id_present: bool,
    customer_id_non_nil: Option<bool>,
    channel_id_present: bool,
    channel_id_non_nil: Option<bool>,
    channel_slug_present: bool,
    channel_slug_length: Option<usize>,
    locale_length: usize,
    operation: &'static str,
}

impl StorefrontPaymentCollectionErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        cart_id: Uuid,
        customer_id: Option<Uuid>,
        request_context: &RequestContext,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id_non_nil: !tenant_id.is_nil(),
            actor_id_non_nil: !actor_id.is_nil(),
            cart_id_non_nil: !cart_id.is_nil(),
            customer_id_present: customer_id.is_some(),
            customer_id_non_nil: customer_id.map(|value| !value.is_nil()),
            channel_id_present: request_context.channel_id.is_some(),
            channel_id_non_nil: request_context.channel_id.map(|value| !value.is_nil()),
            channel_slug_present: request_context.channel_slug.is_some(),
            channel_slug_length: request_context
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            locale_length: request_context.locale.chars().count(),
            operation,
        }
    }
}

#[derive(Clone, Copy)]
struct StorefrontCheckoutRuntimeErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
}

fn storefront_payment_collection_actor(auth: Option<&AuthContext>) -> PortActor {
    auth.map(|auth| PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("rustok-commerce.storefront-payment-collection"))
}

fn storefront_payment_collection_port_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    operation: &'static str,
    is_write: bool,
) -> PortContext {
    let locale = if request_context.locale.trim().is_empty() {
        "und"
    } else {
        request_context.locale.as_str()
    };
    let correlation_id = format!("commerce-storefront-payment-collection:{operation}:{cart_id}");
    let context = PortContext::new(
        tenant_id.to_string(),
        storefront_payment_collection_actor(auth),
        locale,
        correlation_id,
    )
    .with_deadline(std::time::Duration::from_secs(2));
    let context = match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    };
    if is_write {
        context.with_idempotency_key(format!("storefront-payment-collection:{cart_id}"))
    } else {
        context
    }
}

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

    let actor_id = super::checkout_actor_id(auth.0.as_ref());
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
    let store_context =
        super::resolve_context_from_cart_for_db(runtime.db(), tenant.id, &request_context, &cart)
            .await?;

    let read_context = storefront_payment_collection_port_context(
        tenant.id,
        cart.id,
        &request_context,
        auth.0.as_ref(),
        "find_reusable_collection_by_cart",
        false,
    );
    if let Some(existing) = runtime
        .payment_cart_read_port()
        .find_reusable_collection_by_cart(
            read_context.clone(),
            ReusablePaymentCollectionByCartRequest { cart_id: cart.id },
        )
        .await
        .map_err(|error| {
            payment_collection_http_error(
                StorefrontPaymentCollectionErrorContext::new(
                    tenant.id,
                    actor_id,
                    cart.id,
                    cart.customer_id,
                    &request_context,
                    "find_reusable_collection_by_cart",
                ),
                &read_context,
                error,
            )
        })?
    {
        return Ok((StatusCode::OK, Json(existing)));
    }

    let command_context = storefront_payment_collection_port_context(
        tenant.id,
        cart.id,
        &request_context,
        auth.0.as_ref(),
        "create_or_reuse_collection",
        true,
    );
    let collection = runtime
        .payment_collection_port()
        .create_or_reuse_collection(
            command_context.clone(),
            PaymentCollectionCreateOrReuseRequest {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: super::merge_metadata(
                    input.metadata,
                    super::cart_context_metadata(&cart, &store_context),
                ),
            },
        )
        .await
        .map_err(|error| {
            payment_collection_http_error(
                StorefrontPaymentCollectionErrorContext::new(
                    tenant.id,
                    actor_id,
                    cart.id,
                    cart.customer_id,
                    &request_context,
                    "create_or_reuse_collection",
                ),
                &command_context,
                error,
            )
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
    let response = crate::services::storefront_staged_checkout_runtime::complete_storefront_checkout_input_with_product_port(
        &storefront_runtime,
        runtime.payment_provider_registry(),
        runtime.product_catalog_read_port(),
        tenant.id,
        &request_context,
        auth.0,
        idempotency_key,
        checkout_input,
    )
    .await
    .map_err(|error| {
        storefront_checkout_http_error(
            StorefrontCheckoutErrorContext::new(
                tenant.id,
                actor_id,
                cart_id,
                &request_context,
                "complete_cart_checkout",
            ),
            error,
        )
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

fn storefront_checkout_error_policy(
    error: &crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> StorefrontCheckoutHttpPolicy {
    use crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError;

    match error {
        StorefrontStagedCheckoutRuntimeError::Validation(_) => {
            (StatusCode::BAD_REQUEST, "validation")
        }
        StorefrontStagedCheckoutRuntimeError::CartAccess => (StatusCode::NOT_FOUND, "cart_access"),
        StorefrontStagedCheckoutRuntimeError::AuthenticationRequired => {
            (StatusCode::UNAUTHORIZED, "authentication_required")
        }
        StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable => {
            (StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable")
        }
        StorefrontStagedCheckoutRuntimeError::CheckoutFailed => {
            (StatusCode::INTERNAL_SERVER_ERROR, "checkout_failed")
        }
        StorefrontStagedCheckoutRuntimeError::CompensationPending => {
            (StatusCode::CONFLICT, "compensation_pending")
        }
        StorefrontStagedCheckoutRuntimeError::ReconciliationRequired => {
            (StatusCode::CONFLICT, "reconciliation_required")
        }
    }
}

fn storefront_checkout_runtime_error_facts(
    error: &crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> StorefrontCheckoutRuntimeErrorFacts {
    use crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError;

    match error {
        StorefrontStagedCheckoutRuntimeError::Validation(message) => {
            StorefrontCheckoutRuntimeErrorFacts {
                error_variant: "validation",
                text_field_count: 1,
                text_total_length: message.chars().count(),
            }
        }
        StorefrontStagedCheckoutRuntimeError::CartAccess => StorefrontCheckoutRuntimeErrorFacts {
            error_variant: "cart_access",
            text_field_count: 0,
            text_total_length: 0,
        },
        StorefrontStagedCheckoutRuntimeError::AuthenticationRequired => {
            StorefrontCheckoutRuntimeErrorFacts {
                error_variant: "authentication_required",
                text_field_count: 0,
                text_total_length: 0,
            }
        }
        StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable => {
            StorefrontCheckoutRuntimeErrorFacts {
                error_variant: "temporarily_unavailable",
                text_field_count: 0,
                text_total_length: 0,
            }
        }
        StorefrontStagedCheckoutRuntimeError::CheckoutFailed => {
            StorefrontCheckoutRuntimeErrorFacts {
                error_variant: "checkout_failed",
                text_field_count: 0,
                text_total_length: 0,
            }
        }
        StorefrontStagedCheckoutRuntimeError::CompensationPending => {
            StorefrontCheckoutRuntimeErrorFacts {
                error_variant: "compensation_pending",
                text_field_count: 0,
                text_total_length: 0,
            }
        }
        StorefrontStagedCheckoutRuntimeError::ReconciliationRequired => {
            StorefrontCheckoutRuntimeErrorFacts {
                error_variant: "reconciliation_required",
                text_field_count: 0,
                text_total_length: 0,
            }
        }
    }
}

fn storefront_checkout_http_error(
    context: StorefrontCheckoutErrorContext,
    error: crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
) -> HttpError {
    let (status, error_kind) = storefront_checkout_error_policy(&error);
    let error_facts = storefront_checkout_runtime_error_facts(&error);
    let code = error.public_code();
    let message = error.public_message();
    tracing::error!(
        owner = STOREFRONT_CHECKOUT_OWNER,
        tenant_id_non_nil = context.tenant_id_non_nil,
        actor_id_non_nil = context.actor_id_non_nil,
        cart_id_non_nil = context.cart_id_non_nil,
        channel_id_present = context.channel_id_present,
        channel_id_non_nil = ?context.channel_id_non_nil,
        channel_slug_present = context.channel_slug_present,
        channel_slug_length = ?context.channel_slug_length,
        locale_length = context.locale_length,
        operation = context.operation,
        error_variant = error_facts.error_variant,
        error_text_field_count = error_facts.text_field_count,
        error_text_total_length = error_facts.text_total_length,
        error_kind,
        public_code = code,
        retryable = error.retryable(),
        status = status.as_u16(),
        boundary = STOREFRONT_CHECKOUT_BOUNDARY,
        "storefront checkout request failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn payment_collection_error_policy(error: &PortError) -> StorefrontPaymentCollectionHttpPolicy {
    match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "payment_request_invalid",
            "Payment collection request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "payment_resource_not_found",
            "Payment resource was not found",
            "not_found",
        ),
        PortErrorKind::Conflict if error.code == "payment.provider_rejected" => (
            StatusCode::CONFLICT,
            "payment_provider_rejected",
            "Payment provider rejected the requested operation",
            "provider_rejected",
        ),
        PortErrorKind::Conflict if error.code == "payment.provider_outcome_unknown" => (
            StatusCode::CONFLICT,
            "payment_reconciliation_required",
            "Payment operation requires reconciliation",
            "provider_outcome_unknown",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "payment_state_conflict",
            "Payment lifecycle conflicts with the requested operation",
            "state_conflict",
        ),
        PortErrorKind::Unavailable
            if error.code == "payment.database_unavailable"
                || error.code == "payment.cart_read_unavailable" =>
        {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "payment_storage_unavailable",
                "Payment service is temporarily unavailable",
                "database",
            )
        }
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_temporarily_unavailable",
            "Payment service is temporarily unavailable",
            "temporarily_unavailable",
        ),
        PortErrorKind::InvariantViolation if error.code == "payment.provider_invalid_response" => (
            StatusCode::CONFLICT,
            "payment_reconciliation_required",
            "Payment operation requires reconciliation",
            "provider_invalid_response",
        ),
        PortErrorKind::InvariantViolation if error.code == "payment.provider_not_configured" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "payment_temporarily_unavailable",
            "Payment service is temporarily unavailable",
            "provider_configuration",
        ),
        PortErrorKind::Forbidden | PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "payment_operation_failed",
            "Payment operation could not be completed safely",
            "owner_operation_failed",
        ),
    }
}

fn payment_collection_http_error(
    context: StorefrontPaymentCollectionErrorContext,
    port_context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = payment_collection_error_policy(&error);
    tracing::error!(
        owner = STOREFRONT_PAYMENT_COLLECTION_OWNER,
        correlation_id = %port_context.correlation_id,
        tenant_id_non_nil = context.tenant_id_non_nil,
        actor_id_non_nil = context.actor_id_non_nil,
        cart_id_non_nil = context.cart_id_non_nil,
        customer_id_present = context.customer_id_present,
        customer_id_non_nil = ?context.customer_id_non_nil,
        channel_id_present = context.channel_id_present,
        channel_id_non_nil = ?context.channel_id_non_nil,
        channel_slug_present = context.channel_slug_present,
        channel_slug_length = ?context.channel_slug_length,
        locale_length = context.locale_length,
        operation = context.operation,
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = status.as_u16(),
        boundary = STOREFRONT_PAYMENT_COLLECTION_BOUNDARY,
        "storefront payment collection owner call failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}
