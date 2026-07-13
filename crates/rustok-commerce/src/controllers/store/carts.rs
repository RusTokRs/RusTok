use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{OptionalAuthContext, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime, StoreAddCartLineItemInput, StoreCartContextPatch,
    StoreCartResponse, StoreCreateCartInput, StoreUpdateCartInput, StoreUpdateCartLineItemInput,
};
use crate::dto::CartResponse;
use rustok_cart::CartService;
use rustok_pricing::PricingService;

/// Create a storefront cart
#[utoipa::path(
    post,
    path = "/store/carts",
    tag = "store",
    request_body = StoreCreateCartInput,
    responses(
        (status = 201, description = "Cart created", body = StoreCartResponse),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn create_cart(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Json(input): Json<StoreCreateCartInput>,
) -> HttpResult<(StatusCode, Json<StoreCartResponse>)> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let context = super::resolve_context_for_db(
        runtime.db(),
        tenant.id,
        &request_context,
        input.region_id,
        input.country_code.clone(),
        input.locale.clone(),
        input.currency_code.clone(),
    )
    .await?;
    let currency_code = context
        .currency_code
        .clone()
        .or(input.currency_code.clone())
        .ok_or_else(|| {
            HttpError::bad_request(
                "commerce_operation_failed",
                "currency_code is required unless it can be resolved from region/country"
                    .to_string(),
            )
        })?;

    let service = CartService::new(runtime.db_clone());
    let cart = service
        .create_cart_with_channel(
            tenant.id,
            crate::dto::CreateCartInput {
                customer_id,
                email: input.email,
                region_id: context.region.as_ref().map(|region| region.id),
                country_code: input.country_code,
                locale_code: Some(context.locale.clone()),
                selected_shipping_option_id: None,
                currency_code,
                metadata: input.metadata,
            },
            request_context.channel_id,
            request_context.channel_slug.clone(),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
    let cart = super::enrich_storefront_cart_for_db(
        runtime.db(),
        tenant.id,
        &request_context,
        tenant.default_locale.as_str(),
        cart,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(StoreCartResponse { cart, context }),
    ))
}

/// Get storefront cart
#[utoipa::path(
    get,
    path = "/store/carts/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Cart ID")),
    responses(
        (status = 200, description = "Cart details", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn get_cart(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(runtime.db_clone());
    let cart = service
        .get_cart(tenant.id, id)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
    super::ensure_store_cart_access(&cart, customer_id)?;
    Ok(Json(
        super::enrich_storefront_cart_for_db(
            runtime.db(),
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Update storefront cart context
#[utoipa::path(
    post,
    path = "/store/carts/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Cart ID")),
    request_body = StoreUpdateCartInput,
    responses(
        (status = 200, description = "Updated cart context", body = StoreCartResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn update_cart_context(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<StoreUpdateCartInput>,
) -> HttpResult<Json<StoreCartResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let cart_service = CartService::new(runtime.db_clone());
    let cart = cart_service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&cart, customer_id)?;

    let updated = super::apply_cart_context_patch_for_db(
        runtime.db(),
        runtime.event_bus(),
        tenant.id,
        &request_context,
        tenant.default_locale.as_str(),
        &cart,
        StoreCartContextPatch {
            email: input.email,
            region_id: input.region_id,
            country_code: input.country_code,
            locale: input.locale,
            selected_shipping_option_id: input.selected_shipping_option_id,
            shipping_selections: input.shipping_selections.map(|items| {
                items
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<crate::dto::CartShippingSelectionInput>>()
            }),
        },
    )
    .await?;

    Ok(Json(updated))
}

/// Add storefront cart line item
#[utoipa::path(
    post,
    path = "/store/carts/{id}/line-items",
    tag = "store",
    params(("id" = Uuid, Path, description = "Cart ID")),
    request_body = StoreAddCartLineItemInput,
    responses(
        (status = 200, description = "Updated cart", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn add_cart_line_item(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<StoreAddCartLineItemInput>,
) -> HttpResult<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(runtime.db_clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&existing, customer_id)?;
    let event_bus = runtime.event_bus();
    let pricing_service = PricingService::new(runtime.db_clone(), event_bus.clone());
    let pricing_context =
        super::build_store_pricing_context(&existing, &request_context, input.quantity);
    let public_channel_slug =
        super::storefront_public_channel_slug_for_cart(&existing, &request_context);
    let resolved_input = super::resolve_store_line_item_input(
        runtime.db(),
        tenant.id,
        super::StoreLineItemResolution {
            pricing_service: &pricing_service,
            pricing_context: &pricing_context,
            locale: existing
                .locale_code
                .as_deref()
                .unwrap_or(request_context.locale.as_str()),
            default_locale: tenant.default_locale.as_str(),
            public_channel_slug: public_channel_slug.as_deref(),
            input,
        },
    )
    .await?;

    let cart = service
        .add_line_item_with_pricing_adjustment(
            tenant.id,
            id,
            resolved_input.add_line_item,
            resolved_input.pricing_adjustment,
        )
        .await
        .map_err(super::map_cart_error)?;
    Ok(Json(
        super::enrich_storefront_cart_for_db(
            runtime.db(),
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Update storefront cart line item quantity
#[utoipa::path(
    post,
    path = "/store/carts/{id}/line-items/{line_id}",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("line_id" = Uuid, Path, description = "Cart line item ID")
    ),
    request_body = StoreUpdateCartLineItemInput,
    responses(
        (status = 200, description = "Updated cart", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart or line item not found")
    )
)]
pub async fn update_cart_line_item(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path((id, line_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<StoreUpdateCartLineItemInput>,
) -> HttpResult<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(runtime.db_clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&existing, customer_id)?;
    let event_bus = runtime.event_bus();
    if let Some(existing_line_item) = existing.line_items.iter().find(|item| item.id == line_id) {
        if let Some(variant_id) = existing_line_item.variant_id {
            super::validate_store_line_item_quantity(
                runtime.db(),
                tenant.id,
                variant_id,
                input.quantity,
                super::storefront_public_channel_slug_for_cart(&existing, &request_context)
                    .as_deref(),
            )
            .await?;
        }
    }

    let cart = if let Some(variant_id) = existing
        .line_items
        .iter()
        .find(|item| item.id == line_id)
        .and_then(|item| item.variant_id)
    {
        let pricing_service = PricingService::new(runtime.db_clone(), event_bus);
        let pricing_context =
            super::build_store_pricing_context(&existing, &request_context, input.quantity);
        let resolved_price = pricing_service
            .resolve_variant_price(tenant.id, variant_id, pricing_context)
            .await
            .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?
            .ok_or_else(|| {
                HttpError::bad_request(
                    "commerce_operation_failed",
                    format!(
                        "No storefront price for variant {} in currency {}",
                        variant_id, existing.currency_code
                    ),
                )
            })?;

        let pricing_update =
            super::storefront_cart_pricing_update(line_id, input.quantity, &resolved_price);
        service
            .update_line_item_pricing(
                tenant.id,
                id,
                line_id,
                input.quantity,
                pricing_update.unit_price,
                pricing_update.pricing_adjustment,
            )
            .await
            .map_err(super::map_cart_error)?
    } else {
        service
            .update_line_item_quantity(tenant.id, id, line_id, input.quantity)
            .await
            .map_err(super::map_cart_error)?
    };
    Ok(Json(
        super::enrich_storefront_cart_for_db(
            runtime.db(),
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Remove storefront cart line item
#[utoipa::path(
    delete,
    path = "/store/carts/{id}/line-items/{line_id}",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("line_id" = Uuid, Path, description = "Cart line item ID")
    ),
    responses(
        (status = 200, description = "Updated cart", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart or line item not found")
    )
)]
pub async fn remove_cart_line_item(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path((id, line_id)): Path<(Uuid, Uuid)>,
) -> HttpResult<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(runtime.db_clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&existing, customer_id)?;

    let cart = service
        .remove_line_item(tenant.id, id, line_id)
        .await
        .map_err(super::map_cart_error)?;
    Ok(Json(
        super::enrich_storefront_cart_for_db(
            runtime.db(),
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}
