use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{
    loco::transactional_event_bus_from_context, OptionalAuthContext, RequestContext, TenantContext,
};
use uuid::Uuid;

use crate::{
    dto::CartResponse,
    CartService, PricingService,
};
use super::{
    StoreCartResponse, StoreCreateCartInput, StoreUpdateCartInput,
    StoreAddCartLineItemInput, StoreUpdateCartLineItemInput, StoreCartContextPatch,
};

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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Json(input): Json<StoreCreateCartInput>,
) -> Result<(StatusCode, Json<StoreCartResponse>)> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let context = super::resolve_context(
        &ctx,
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
            Error::BadRequest(
                "currency_code is required unless it can be resolved from region/country"
                    .to_string(),
            )
        })?;

    let service = CartService::new(ctx.db.clone());
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
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let cart = super::enrich_storefront_cart(
        &ctx,
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let cart = service
        .get_cart(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    super::ensure_store_cart_access(&cart, customer_id)?;
    Ok(Json(
        super::enrich_storefront_cart(
            &ctx,
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<StoreUpdateCartInput>,
) -> Result<Json<StoreCartResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let cart_service = CartService::new(ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&cart, customer_id)?;

    let updated = super::apply_cart_context_patch(
        &ctx,
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<StoreAddCartLineItemInput>,
) -> Result<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&existing, customer_id)?;
    let pricing_service =
        PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let pricing_context = super::build_store_pricing_context(&existing, &request_context, input.quantity);
    let resolved_input = super::resolve_store_line_item_input(
        &ctx.db,
        tenant.id,
        &pricing_service,
        &pricing_context,
        existing
            .locale_code
            .as_deref()
            .unwrap_or(request_context.locale.as_str()),
        tenant.default_locale.as_str(),
        super::storefront_public_channel_slug_for_cart(&existing, &request_context).as_deref(),
        input,
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
        super::enrich_storefront_cart(
            &ctx,
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path((id, line_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<StoreUpdateCartLineItemInput>,
) -> Result<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(super::map_cart_error)?;
    super::ensure_store_cart_access(&existing, customer_id)?;
    if let Some(existing_line_item) = existing.line_items.iter().find(|item| item.id == line_id) {
        if let Some(variant_id) = existing_line_item.variant_id {
            super::validate_store_line_item_quantity(
                &ctx.db,
                tenant.id,
                variant_id,
                input.quantity,
                super::storefront_public_channel_slug_for_cart(&existing, &request_context).as_deref(),
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
        let pricing_service =
            PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
        let pricing_context =
            super::build_store_pricing_context(&existing, &request_context, input.quantity);
        let resolved_price = pricing_service
            .resolve_variant_price(tenant.id, variant_id, pricing_context)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
            .ok_or_else(|| {
                Error::BadRequest(format!(
                    "No storefront price for variant {} in currency {}",
                    variant_id, existing.currency_code
                ))
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
        super::enrich_storefront_cart(
            &ctx,
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path((id, line_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CartResponse>> {
    super::ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = super::current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
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
        super::enrich_storefront_cart(
            &ctx,
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}
