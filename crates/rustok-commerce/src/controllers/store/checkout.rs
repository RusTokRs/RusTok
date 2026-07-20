use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::{OptionalAuthContext, RequestContext, TenantContext};
use rustok_cart::{
    CartStorefrontReadRequest, PrepareCartCheckoutSnapshotRequest,
    bind_in_process_atomic_cart_checkout_with_pricing, in_process_cart_checkout_port,
    in_process_cart_storefront_port,
};
use rustok_payment::PaymentService;
use rustok_web::{HttpError, HttpResult};
use std::sync::Arc;
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime, StoreCompleteCartInput, StoreCreatePaymentCollectionInput,
};
use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse, PaymentCollectionResponse};

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 191;
const CHECKOUT_PRICING_CHANGED_CODE: &str = "cart.checkout_pricing_changed";

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
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?
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
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

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

    let cart_storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let cart = cart_storefront_port
        .read_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                cart_id,
                "read",
                false,
            ),
            CartStorefrontReadRequest { cart_id },
        )
        .await
        .map_err(rustok_web::port_error_to_http_error)?;
    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    super::ensure_store_cart_access(&cart, customer_id)?;
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
    let event_bus = runtime.event_bus();
    let payment_provider_registry = runtime.payment_provider_registry();
    let pricing_resolver = Arc::new(crate::StorefrontCheckoutPricingResolver::new(
        runtime.db_clone(),
        event_bus.clone(),
        request_context.channel_id,
        request_context.channel_slug.clone(),
    ));
    let atomic_cart = bind_in_process_atomic_cart_checkout_with_pricing(
        runtime.db_clone(),
        PrepareCartCheckoutSnapshotRequest {
            cart_id,
            region_id: checkout_input.region_id,
            country_code: checkout_input.country_code.clone(),
            locale_code: checkout_input.locale.clone(),
            selected_shipping_option_id: checkout_input.shipping_option_id,
            shipping_selections: checkout_input.shipping_selections.clone(),
        },
        pricing_resolver,
    );

    let inventory_service = Arc::new(rustok_inventory::InventoryService::new(
        runtime.db_clone(),
        event_bus.clone(),
    ));
    let reservation_port =
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone());
    let plan_builder = crate::CheckoutPlanBuilder::new(
        runtime.db_clone(),
        Arc::new(rustok_region::RegionService::new(runtime.db_clone())),
        inventory_service,
        Arc::new(rustok_product::CatalogService::new(
            runtime.db_clone(),
            event_bus.clone(),
        )),
    );
    let pipeline = crate::CheckoutStagePipeline::new(
        runtime.db_clone(),
        event_bus.clone(),
        reservation_port.clone(),
        atomic_cart.port.clone(),
    )
    .with_payment_provider_registry(payment_provider_registry.clone());
    let staged = crate::StagedCheckoutService::new(
        plan_builder,
        pipeline,
        atomic_cart.handle,
        runtime.db_clone(),
    );
    let compensation = crate::CheckoutCompensationService::new(
        runtime.db_clone(),
        event_bus,
        reservation_port,
        in_process_cart_checkout_port(runtime.db_clone()),
    )
    .with_payment_provider_registry(payment_provider_registry);
    let service = crate::services::RecoveringStagedCheckoutService::new(staged, compensation);
    let response = service
        .complete_checkout(tenant.id, actor_id, idempotency_key, checkout_input)
        .await
        .map_err(recovering_checkout_http_error)?;

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

fn recovering_checkout_http_error(
    error: crate::services::RecoveringStagedCheckoutError,
) -> HttpError {
    match error {
        crate::services::RecoveringStagedCheckoutError::Staged(staged) => {
            staged_checkout_http_error(staged)
        }
        crate::services::RecoveringStagedCheckoutError::StagedAndJournal { .. } => {
            HttpError::internal("Checkout failed and recovery journal lookup is unavailable")
        }
        crate::services::RecoveringStagedCheckoutError::Journal(_) => {
            HttpError::internal("Checkout recovery journal lookup is unavailable")
        }
        crate::services::RecoveringStagedCheckoutError::StagedAndCompensation {
            compensation: crate::CheckoutCompensationError::ManualReconciliation(_),
            ..
        } => HttpError::new(
            StatusCode::CONFLICT,
            "checkout_reconciliation_required",
            "Checkout reached an external side effect that requires reconciliation",
        ),
        crate::services::RecoveringStagedCheckoutError::StagedAndCompensation { .. } => {
            HttpError::new(
                StatusCode::CONFLICT,
                "checkout_compensation_pending",
                "Checkout failed and compensation will be retried",
            )
        }
    }
}

fn staged_checkout_http_error(error: crate::StagedCheckoutError) -> HttpError {
    match error {
        crate::StagedCheckoutError::Operation(crate::CheckoutOperationError::Conflict(message)) => {
            HttpError::new(StatusCode::CONFLICT, "checkout_operation_conflict", message)
        }
        crate::StagedCheckoutError::Operation(crate::CheckoutOperationError::NotFound(id)) => {
            HttpError::not_found(
                "checkout_operation_not_found",
                format!("Checkout operation {id} was not found"),
            )
        }
        crate::StagedCheckoutError::Operation(crate::CheckoutOperationError::Validation(
            message,
        )) => HttpError::bad_request("checkout_operation_invalid", message),
        crate::StagedCheckoutError::Operation(crate::CheckoutOperationError::Database(_)) => {
            HttpError::internal("Checkout operation storage is unavailable")
        }
        crate::StagedCheckoutError::Checkout(crate::CheckoutError::BoundaryFailure {
            kind: rustok_api::PortErrorKind::Conflict,
            code,
            ..
        }) if code == CHECKOUT_PRICING_CHANGED_CODE => HttpError::new(
            StatusCode::CONFLICT,
            code,
            "Checkout pricing or cart state changed; retry with a new Idempotency-Key",
        ),
        crate::StagedCheckoutError::Checkout(crate::CheckoutError::BoundaryFailure {
            kind: rustok_api::PortErrorKind::Conflict,
            code,
            ..
        }) => HttpError::new(
            StatusCode::CONFLICT,
            code,
            "Checkout could not proceed because a domain constraint changed",
        ),
        crate::StagedCheckoutError::Checkout(checkout) => {
            HttpError::bad_request("commerce_operation_failed", checkout.to_string())
        }
        crate::StagedCheckoutError::Pipeline(_) => HttpError::new(
            StatusCode::CONFLICT,
            "checkout_pipeline_failed",
            "Checkout entered recovery; retry after reconciliation",
        ),
        crate::StagedCheckoutError::CheckoutAndJournal { .. }
        | crate::StagedCheckoutError::PipelineAndJournal { .. } => {
            HttpError::internal("Checkout requires reconciliation after a journal update failure")
        }
    }
}
