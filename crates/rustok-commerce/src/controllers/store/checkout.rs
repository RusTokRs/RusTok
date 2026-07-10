use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rustok_api::{OptionalAuthContext, RequestContext, TenantContext};
use rustok_cart::CartService;
use rustok_payment::PaymentService;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime, StoreCartContextPatch, StoreCompleteCartInput,
    StoreCreatePaymentCollectionInput,
};
use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse, PaymentCollectionResponse};

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
    let cart_service = CartService::new(runtime.db_clone());
    let cart = cart_service
        .get_cart(tenant.id, input.cart_id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&cart, customer_id)?;
    super::ensure_cart_allows_payment_collection(&cart)?;
    let cart = super::reprice_storefront_cart_line_items_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        &request_context,
        &cart_service,
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
    params(("id" = Uuid, Path, description = "Cart ID")),
    request_body = StoreCompleteCartInput,
    responses(
        (status = 200, description = "Checkout completed", body = CompleteCheckoutResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn complete_cart_checkout(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(cart_id): Path<Uuid>,
    Json(input): Json<StoreCompleteCartInput>,
) -> HttpResult<Json<CompleteCheckoutResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let cart_service = CartService::new(runtime.db_clone());
    let mut cart = cart_service
        .get_cart(tenant.id, cart_id)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    super::ensure_store_cart_access(&cart, customer_id)?;
    let actor_id = super::checkout_actor_id(auth.0.as_ref());

    if input.shipping_option_id.is_some()
        || input.shipping_selections.is_some()
        || input.region_id.is_some()
        || input.country_code.is_some()
        || input.locale.is_some()
    {
        cart = super::apply_cart_context_patch_for_db(
            runtime.db(),
            runtime.event_bus(),
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            &cart,
            StoreCartContextPatch {
                email: None,
                region_id: input.region_id.map(Some),
                country_code: input.country_code.clone().map(Some),
                locale: input.locale.clone().map(Some),
                selected_shipping_option_id: input.shipping_option_id.map(Some),
                shipping_selections: input.shipping_selections.clone().map(|items| {
                    items
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<crate::dto::CartShippingSelectionInput>>()
                }),
            },
        )
        .await?
        .cart;
    }
    let _ = super::reprice_storefront_cart_line_items_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        &request_context,
        &cart_service,
        cart,
    )
    .await?;

    let event_bus = runtime.event_bus();
    let service = crate::CheckoutService::new(
        runtime.db_clone(),
        event_bus.clone(),
        std::sync::Arc::new(rustok_region::RegionService::new(runtime.db_clone())),
        std::sync::Arc::new(rustok_inventory::InventoryService::new(
            runtime.db_clone(),
            event_bus,
        )),
    );
    let response = service
        .complete_checkout(
            tenant.id,
            actor_id,
            CompleteCheckoutInput {
                cart_id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: input.create_fulfillment,
                metadata: input.metadata,
            },
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    Ok(Json(response))
}
