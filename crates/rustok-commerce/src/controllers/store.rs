use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rustok_api::{
    loco::transactional_event_bus_from_context, OptionalAuthContext, RequestContext, TenantContext,
};
use rustok_cart::CartError;
use rustok_core::locale_tags_match;
use rustok_pricing::PriceResolutionContext;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    dto::{
        AddCartLineItemInput, CartResponse, CompleteCheckoutInput, CompleteCheckoutResponse,
        CreateCartInput, CustomerResponse, OrderResponse, PaymentCollectionResponse,
        RegionResponse, ResolveStoreContextInput, ShippingOptionResponse, StoreContextResponse,
        UpdateCartContextInput,
    },
    entities::{product, product_translation, product_variant, variant_translation},
    search::product_translation_title_search_condition,
    storefront_channel::{
        apply_public_channel_inventory_to_product, is_metadata_visible_for_public_channel,
        is_module_enabled_for_request_channel,
        load_available_inventory_for_variant_in_public_channel, normalize_public_channel_slug,
        public_channel_slug_from_request,
    },
    storefront_shipping::{
        effective_shipping_profile_slug, enrich_cart_delivery_groups,
        is_shipping_option_compatible_with_profiles, load_cart_shipping_profile_slugs,
        normalize_shipping_profile_slug, shipping_profile_slug_from_product_metadata,
    },
    CartService, CatalogService, CustomerService, FulfillmentService, OrderService, PaymentService,
    PricingService, ProductResponse, RegionService, StoreContextService,
};

use super::{
    common::{PaginatedResponse, PaginationMeta, PaginationParams},
    products::ProductListItem,
};

pub fn routes() -> Routes {
    Routes::new()
        .add("/products", axum::routing::get(list_products))
        .add("/products/{id}", axum::routing::get(show_product))
        .add("/regions", axum::routing::get(list_regions))
        .add(
            "/shipping-options",
            axum::routing::get(list_shipping_options),
        )
        .add("/carts", axum::routing::post(create_cart))
        .add(
            "/carts/{id}",
            axum::routing::get(get_cart).post(update_cart_context),
        )
        .add(
            "/carts/{id}/line-items",
            axum::routing::post(add_cart_line_item),
        )
        .add(
            "/carts/{id}/line-items/{line_id}",
            axum::routing::post(update_cart_line_item).delete(remove_cart_line_item),
        )
        .add(
            "/carts/{id}/complete",
            axum::routing::post(complete_cart_checkout),
        )
        .add(
            "/payment-collections",
            axum::routing::post(create_payment_collection),
        )
        .add("/orders/{id}", axum::routing::get(get_order))
        .add("/customers/me", axum::routing::get(get_me))
}

const MODULE_SLUG: &str = "commerce";

/// List published storefront products
#[utoipa::path(
    get,
    path = "/store/products",
    tag = "store",
    params(StoreListProductsParams),
    responses(
        (status = 200, description = "Published storefront products", body = PaginatedResponse<ProductListItem>),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn list_products(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    Query(params): Query<StoreListProductsParams>,
) -> Result<Json<PaginatedResponse<ProductListItem>>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let _requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();
    let locale = params
        .locale
        .as_deref()
        .unwrap_or(request_context.locale.as_str());

    let public_channel_slug = public_channel_slug_from_request(&request_context);
    let mut query = product::Entity::find()
        .filter(product::Column::TenantId.eq(tenant.id))
        .filter(product::Column::Status.eq(product::ProductStatus::Active))
        .filter(product::Column::PublishedAt.is_not_null());

    if let Some(vendor) = &params.vendor {
        query = query.filter(product::Column::Vendor.eq(vendor));
    }
    if let Some(product_type) = &params.product_type {
        query = query.filter(product::Column::ProductType.eq(product_type));
    }
    if let Some(search) = &params.search {
        query = query.filter(product_translation_title_search_condition(
            ctx.db.get_database_backend(),
            locale,
            search,
        ));
    }

    let visible_products = query
        .order_by_desc(product::Column::PublishedAt)
        .order_by_desc(product::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .into_iter()
        .filter(|product| {
            is_metadata_visible_for_public_channel(
                &product.metadata,
                public_channel_slug.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let total = visible_products.len() as u64;
    let products = visible_products
        .into_iter()
        .skip(pagination.offset() as usize)
        .take(pagination.limit() as usize)
        .collect::<Vec<_>>();

    let product_ids = products
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let translations = if product_ids.is_empty() {
        Vec::new()
    } else {
        product_translation::Entity::find()
            .filter(product_translation::Column::ProductId.is_in(product_ids))
            .all(&ctx.db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
    };

    let mut translation_map =
        std::collections::HashMap::<Uuid, Vec<product_translation::Model>>::new();
    for translation in translations {
        translation_map
            .entry(translation.product_id)
            .or_default()
            .push(translation);
    }
    let catalog = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product_tags = catalog
        .load_product_tag_map(
            tenant.id,
            &products,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let items = products
        .into_iter()
        .map(|product| {
            let translation = translation_map
                .get(&product.id)
                .and_then(|items| pick_product_translation(items, locale, tenant.default_locale.as_str()));
            ProductListItem {
                id: product.id,
                status: product.status.to_string(),
                title: translation
                    .map(|value| value.title.clone())
                    .unwrap_or_default(),
                handle: translation
                    .map(|value| value.handle.clone())
                    .unwrap_or_default(),
                seller_id: product.seller_id,
                vendor: product.vendor,
                product_type: product.product_type,
                shipping_profile_slug: Some(shipping_profile_slug_from_product_metadata(
                    &product.metadata,
                )),
                tags: product_tags.get(&product.id).cloned().unwrap_or_default(),
                created_at: product.created_at.to_rfc3339(),
                published_at: product.published_at.map(|value| value.to_rfc3339()),
            }
        })
        .collect::<Vec<_>>();

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Show published storefront product
#[utoipa::path(
    get,
    path = "/store/products/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product details", body = ProductResponse),
        (status = 404, description = "Product not found")
    )
)]
pub async fn show_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let public_channel_slug = public_channel_slug_from_request(&request_context);
    let mut product = service
        .get_product_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if product.status != product::ProductStatus::Active
        || product.published_at.is_none()
        || !is_metadata_visible_for_public_channel(
            &product.metadata,
            public_channel_slug.as_deref(),
        )
    {
        return Err(Error::NotFound);
    }

    apply_public_channel_inventory_to_product(
        &ctx.db,
        tenant.id,
        &mut product,
        public_channel_slug.as_deref(),
    )
    .await
    .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(product))
}

/// List available storefront regions
#[utoipa::path(
    get,
    path = "/store/regions",
    tag = "store",
    responses(
        (status = 200, description = "Store regions", body = Vec<RegionResponse>)
    )
)]
pub async fn list_regions(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
) -> Result<Json<Vec<RegionResponse>>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let service = RegionService::new(ctx.db.clone());
    let regions = service
        .list_regions(
            tenant.id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(regions))
}

/// List active storefront shipping options
#[utoipa::path(
    get,
    path = "/store/shipping-options",
    tag = "store",
    params(StoreContextQuery),
    responses(
        (status = 200, description = "Shipping options", body = Vec<ShippingOptionResponse>)
    )
)]
pub async fn list_shipping_options(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Query(query): Query<StoreContextQuery>,
) -> Result<Json<Vec<ShippingOptionResponse>>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let (context, public_channel_slug, required_shipping_profiles) =
        if let Some(cart_id) = query.cart_id {
            let cart_service = CartService::new(ctx.db.clone());
            let cart = cart_service
                .get_cart(tenant.id, cart_id)
                .await
                .map_err(map_cart_error)?;
            ensure_store_cart_access(&cart, customer_id)?;
            let required_shipping_profiles =
                load_cart_shipping_profile_slugs(&ctx.db, tenant.id, &cart)
                    .await
                    .map_err(|err| Error::BadRequest(err.to_string()))?;
            (
                resolve_context_from_cart(&ctx, tenant.id, &request_context, &cart).await?,
                storefront_public_channel_slug_for_cart(&cart, &request_context),
                required_shipping_profiles,
            )
        } else {
            (
                resolve_context(
                    &ctx,
                    tenant.id,
                    &request_context,
                    query.region_id,
                    query.country_code.clone(),
                    query.locale.clone(),
                    query.currency_code.clone(),
                )
                .await?,
                public_channel_slug_from_request(&request_context),
                Default::default(),
            )
        };

    let service = FulfillmentService::new(ctx.db.clone());
    let mut options = service
        .list_shipping_options(
            tenant.id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if let Some(currency_code) = context.currency_code.as_deref() {
        options.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
    }
    options.retain(|option| {
        is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug.as_deref())
            && is_shipping_option_compatible_with_profiles(option, &required_shipping_profiles)
    });

    Ok(Json(options))
}

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
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let context = resolve_context(
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
            CreateCartInput {
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
    let cart = enrich_storefront_cart(
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
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let cart = service
        .get_cart(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    ensure_store_cart_access(&cart, customer_id)?;
    Ok(Json(
        enrich_storefront_cart(
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
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let cart_service = CartService::new(ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, id)
        .await
        .map_err(map_cart_error)?;
    ensure_store_cart_access(&cart, customer_id)?;

    let updated = apply_cart_context_patch(
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
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(map_cart_error)?;
    ensure_store_cart_access(&existing, customer_id)?;
    let pricing_service =
        PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let pricing_context = build_store_pricing_context(&existing, &request_context, input.quantity);
    let resolved_input = resolve_store_line_item_input(
        &ctx.db,
        tenant.id,
        &pricing_service,
        &pricing_context,
        existing
            .locale_code
            .as_deref()
            .unwrap_or(request_context.locale.as_str()),
        tenant.default_locale.as_str(),
        storefront_public_channel_slug_for_cart(&existing, &request_context).as_deref(),
        input,
    )
    .await?;

    let cart = service
        .add_line_item(tenant.id, id, resolved_input)
        .await
        .map_err(map_cart_error)?;
    let cart =
        reprice_storefront_cart_line_items(&ctx, tenant.id, &request_context, &service, cart)
            .await?;
    Ok(Json(
        enrich_storefront_cart(
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
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(map_cart_error)?;
    ensure_store_cart_access(&existing, customer_id)?;
    if let Some(existing_line_item) = existing.line_items.iter().find(|item| item.id == line_id) {
        if let Some(variant_id) = existing_line_item.variant_id {
            validate_store_line_item_quantity(
                &ctx.db,
                tenant.id,
                variant_id,
                input.quantity,
                storefront_public_channel_slug_for_cart(&existing, &request_context).as_deref(),
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
            build_store_pricing_context(&existing, &request_context, input.quantity);
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
            storefront_cart_pricing_update(line_id, input.quantity, &resolved_price);
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
            .map_err(map_cart_error)?
    } else {
        service
            .update_line_item_quantity(tenant.id, id, line_id, input.quantity)
            .await
            .map_err(map_cart_error)?
    };
    Ok(Json(
        enrich_storefront_cart(
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
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(map_cart_error)?;
    ensure_store_cart_access(&existing, customer_id)?;

    let cart = service
        .remove_line_item(tenant.id, id, line_id)
        .await
        .map_err(map_cart_error)?;
    Ok(Json(
        enrich_storefront_cart(
            &ctx,
            tenant.id,
            &request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Create payment collection from storefront cart
#[utoipa::path(
    post,
    path = "/store/payment-collections",
    tag = "store",
    request_body = StoreCreatePaymentCollectionInput,
    responses(
        (status = 201, description = "Payment collection created", body = PaymentCollectionResponse),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn create_payment_collection(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Json(input): Json<StoreCreatePaymentCollectionInput>,
) -> Result<(StatusCode, Json<PaymentCollectionResponse>)> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let cart_service = CartService::new(ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, input.cart_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    ensure_store_cart_access(&cart, customer_id)?;
    let cart =
        reprice_storefront_cart_line_items(&ctx, tenant.id, &request_context, &cart_service, cart)
            .await?;
    let context = resolve_context_from_cart(&ctx, tenant.id, &request_context, &cart).await?;

    let service = PaymentService::new(ctx.db.clone());
    if let Some(existing) = service
        .find_reusable_collection_by_cart(tenant.id, cart.id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
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
                metadata: merge_metadata(input.metadata, cart_context_metadata(&cart, &context)),
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(cart_id): Path<Uuid>,
    Json(input): Json<StoreCompleteCartInput>,
) -> Result<Json<CompleteCheckoutResponse>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let cart_service = CartService::new(ctx.db.clone());
    let mut cart = cart_service
        .get_cart(tenant.id, cart_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    ensure_store_cart_access(&cart, customer_id)?;
    let actor_id = checkout_actor_id(auth.0.as_ref());

    if input.shipping_option_id.is_some()
        || input.shipping_selections.is_some()
        || input.region_id.is_some()
        || input.country_code.is_some()
        || input.locale.is_some()
    {
        cart = apply_cart_context_patch(
            &ctx,
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
    let _ =
        reprice_storefront_cart_line_items(&ctx, tenant.id, &request_context, &cart_service, cart)
            .await?;

    let service =
        crate::CheckoutService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
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
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(response))
}

/// Get current storefront customer
#[utoipa::path(
    get,
    path = "/store/customers/me",
    tag = "store",
    responses(
        (status = 200, description = "Current customer", body = CustomerResponse),
        (status = 401, description = "Authentication required")
    )
)]
pub async fn get_me(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
) -> Result<Json<CustomerResponse>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let service = CustomerService::new(ctx.db.clone());
    let customer = service
        .get_customer_by_user(tenant.id, auth.user_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(customer))
}

/// Get customer-owned storefront order
#[utoipa::path(
    get,
    path = "/store/orders/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Order ID")),
    responses(
        (status = 200, description = "Order details", body = OrderResponse),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn get_order(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderResponse>> {
    ensure_storefront_channel_enabled(&ctx, &request_context).await?;

    let customer_id = current_customer_id(&ctx, tenant.id, Some(&auth))
        .await?
        .ok_or_else(|| Error::Unauthorized("Customer account required".to_string()))?;
    let service = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let order = service
        .get_order_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if order.customer_id != Some(customer_id) {
        return Err(Error::Unauthorized(
            "Order does not belong to the current customer".to_string(),
        ));
    }

    Ok(Json(order))
}

async fn resolve_context(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    region_id: Option<Uuid>,
    country_code: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<StoreContextResponse> {
    let service = StoreContextService::new(ctx.db.clone());
    service
        .resolve_context(
            tenant_id,
            ResolveStoreContextInput {
                region_id,
                country_code,
                locale: locale.or_else(|| Some(request_context.locale.clone())),
                currency_code,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))
}

async fn resolve_context_from_cart(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart: &CartResponse,
) -> Result<StoreContextResponse> {
    resolve_context(
        ctx,
        tenant_id,
        request_context,
        cart.region_id,
        cart.country_code.clone(),
        cart.locale_code.clone(),
        Some(cart.currency_code.clone()),
    )
    .await
}

async fn current_customer_id(
    ctx: &AppContext,
    tenant_id: Uuid,
    auth: Option<&rustok_api::AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    let service = CustomerService::new(ctx.db.clone());
    match service.get_customer_by_user(tenant_id, auth.user_id).await {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(err) => Err(Error::BadRequest(err.to_string())),
    }
}

async fn ensure_storefront_channel_enabled(
    ctx: &AppContext,
    request_context: &RequestContext,
) -> Result<()> {
    let enabled = is_module_enabled_for_request_channel(&ctx.db, request_context, MODULE_SLUG)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if !enabled {
        return Err(Error::Unauthorized(format!(
            "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
            request_context.channel_slug.as_deref().unwrap_or("current"),
        )));
    }

    Ok(())
}

fn storefront_public_channel_slug_for_cart(
    cart: &CartResponse,
    request_context: &RequestContext,
) -> Option<String> {
    normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| public_channel_slug_from_request(request_context))
}

fn ensure_store_cart_access(cart: &CartResponse, customer_id: Option<Uuid>) -> Result<()> {
    if let Some(expected_customer_id) = cart.customer_id {
        if customer_id != Some(expected_customer_id) {
            return Err(Error::Unauthorized(
                "Cart belongs to another customer".to_string(),
            ));
        }
    }

    Ok(())
}

fn checkout_actor_id(auth: Option<&rustok_api::AuthContext>) -> Uuid {
    auth.map(|auth| auth.user_id).unwrap_or_else(Uuid::nil)
}

async fn apply_cart_context_patch(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: &CartResponse,
    patch: StoreCartContextPatch,
) -> Result<StoreCartResponse> {
    let requested = requested_cart_context(cart, request_context, patch);

    let context = resolve_context(
        ctx,
        tenant_id,
        request_context,
        requested.region_id,
        requested.country_code.clone(),
        requested.locale,
        Some(cart.currency_code.clone()),
    )
    .await?;

    validate_selected_shipping_option(
        ctx,
        tenant_id,
        cart,
        requested.selected_shipping_option_id,
        Some(requested.shipping_selections.as_slice()),
        &cart.currency_code,
        storefront_public_channel_slug_for_cart(cart, request_context).as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await?;

    let cart_service = CartService::new(ctx.db.clone());
    let updated_cart = cart_service
        .update_context(
            tenant_id,
            cart.id,
            UpdateCartContextInput {
                email: requested.email,
                region_id: context.region.as_ref().map(|region| region.id),
                country_code: requested.country_code,
                locale_code: Some(context.locale.clone()),
                selected_shipping_option_id: requested.selected_shipping_option_id,
                shipping_selections: Some(requested.shipping_selections.clone()),
            },
        )
        .await
        .map_err(map_cart_error)?;
    let updated_cart = reprice_storefront_cart_line_items(
        ctx,
        tenant_id,
        request_context,
        &cart_service,
        updated_cart,
    )
    .await?;
    let updated_cart = enrich_storefront_cart(
        ctx,
        tenant_id,
        request_context,
        tenant_default_locale,
        updated_cart,
    )
    .await?;

    Ok(StoreCartResponse {
        cart: updated_cart,
        context,
    })
}

async fn reprice_storefront_cart_line_items(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart_service: &CartService,
    cart: CartResponse,
) -> Result<CartResponse> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let pricing_service =
        PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(ctx));
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_context =
            build_store_pricing_context(&cart, request_context, line_item.quantity);
        let resolved_price = pricing_service
            .resolve_variant_price(tenant_id, variant_id, pricing_context)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
            .ok_or_else(|| {
                Error::BadRequest(format!(
                    "No storefront price for variant {} in currency {}",
                    variant_id, cart.currency_code
                ))
            })?;
        updates.push(storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        Ok(cart)
    } else {
        cart_service
            .reprice_line_items(tenant_id, cart.id, updates)
            .await
            .map_err(map_cart_error)
    }
}

fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

async fn enrich_storefront_cart(
    ctx: &AppContext,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: CartResponse,
) -> Result<CartResponse> {
    let public_channel_slug = storefront_public_channel_slug_for_cart(&cart, request_context);
    enrich_cart_delivery_groups(
        &ctx.db,
        tenant_id,
        cart,
        public_channel_slug.as_deref(),
        Some(request_context.locale.as_str()),
        Some(tenant_default_locale),
    )
    .await
    .map_err(|err| Error::BadRequest(err.to_string()))
}

fn requested_cart_context(
    cart: &CartResponse,
    request_context: &RequestContext,
    patch: StoreCartContextPatch,
) -> RequestedCartContext {
    let region_was_explicit = patch.region_id.is_some();

    RequestedCartContext {
        email: patch.email.unwrap_or_else(|| cart.email.clone()),
        region_id: patch.region_id.unwrap_or(cart.region_id),
        country_code: match patch.country_code {
            Some(country_code) => country_code,
            None if region_was_explicit => None,
            None => cart.country_code.clone(),
        },
        locale: patch
            .locale
            .unwrap_or_else(|| cart.locale_code.clone())
            .or_else(|| Some(request_context.locale.clone())),
        selected_shipping_option_id: patch
            .selected_shipping_option_id
            .unwrap_or(cart.selected_shipping_option_id),
        shipping_selections: patch
            .shipping_selections
            .unwrap_or_else(|| current_shipping_selections(cart)),
    }
}

async fn validate_selected_shipping_option(
    ctx: &AppContext,
    tenant_id: Uuid,
    cart: &CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    let service = FulfillmentService::new(ctx.db.clone());
    let selections = if let Some(shipping_selections) = shipping_selections {
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = selected_shipping_option_id {
        if cart.delivery_groups.len() > 1 {
            return Err(Error::BadRequest(
                "selected_shipping_option_id can only be used for carts with a single delivery group"
                    .to_string(),
            ));
        }
        cart.delivery_groups
            .first()
            .map(|group| {
                vec![crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: group.shipping_profile_slug.clone(),
                    seller_id: group.seller_id.clone(),
                    seller_scope: group.seller_scope.clone(),
                    selected_shipping_option_id: Some(selected_shipping_option_id),
                }]
            })
            .unwrap_or_default()
    } else {
        current_shipping_selections(cart)
    };

    for selection in selections {
        let Some(selected_shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let required_shipping_profiles = BTreeSet::from([normalize_shipping_profile_slug(
            selection.shipping_profile_slug.as_str(),
        )
        .unwrap_or_else(|| "default".to_string())]);
        let option = service
            .get_shipping_option(
                tenant_id,
                selected_shipping_option_id,
                requested_locale,
                tenant_default_locale,
            )
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;
        if !option.currency_code.eq_ignore_ascii_case(currency_code) {
            return Err(Error::BadRequest(format!(
                "Shipping option {} uses currency {}, expected {}",
                option.id, option.currency_code, currency_code
            )));
        }
        if !is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug) {
            return Err(Error::BadRequest(format!(
                "Shipping option {} is not available for the current channel",
                option.id
            )));
        }
        if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
            return Err(Error::BadRequest(format!(
                "Shipping option {} is not compatible with shipping profile {}",
                option.id, selection.shipping_profile_slug
            )));
        }
    }

    Ok(())
}

fn current_shipping_selections(cart: &CartResponse) -> Vec<crate::dto::CartShippingSelectionInput> {
    cart.delivery_groups
        .iter()
        .map(|group| crate::dto::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: group.seller_scope.clone(),
            selected_shipping_option_id: group.selected_shipping_option_id,
        })
        .collect()
}

fn build_store_pricing_context(
    cart: &CartResponse,
    request_context: &RequestContext,
    quantity: i32,
) -> PriceResolutionContext {
    PriceResolutionContext {
        currency_code: cart.currency_code.to_ascii_uppercase(),
        region_id: cart.region_id,
        price_list_id: None,
        channel_id: cart.channel_id.or(request_context.channel_id),
        channel_slug: storefront_public_channel_slug_for_cart(cart, request_context),
        quantity: Some(quantity),
    }
}

async fn resolve_store_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    pricing_service: &PricingService,
    pricing_context: &PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: StoreAddCartLineItemInput,
) -> Result<AddCartLineItemInput> {
    let variant = product_variant::Entity::find_by_id(input.variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let product_model = product::Entity::find_by_id(variant.product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;
    if product_model.status != product::ProductStatus::Active
        || product_model.published_at.is_none()
        || !is_metadata_visible_for_public_channel(&product_model.metadata, public_channel_slug)
    {
        return Err(Error::NotFound);
    }

    let product_translation_models = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .all(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let variant_translation_models = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .all(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let resolved_price = pricing_service
        .resolve_variant_price(tenant_id, variant.id, pricing_context.clone())
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or_else(|| {
            Error::BadRequest(format!(
                "No storefront price for variant {} in currency {}",
                variant.id, pricing_context.currency_code
            ))
        })?;
    validate_store_variant_inventory(db, tenant_id, &variant, input.quantity, public_channel_slug)
        .await?;

    let base_title = pick_product_translation(
        &product_translation_models,
        locale,
        default_locale,
    )
        .map(|translation| translation.title.clone())
        .unwrap_or_else(|| {
            variant
                .sku
                .clone()
                .unwrap_or_else(|| format!("Variant {}", variant.id))
        });
    let title = match pick_variant_translation(&variant_translation_models, locale, default_locale)
        .and_then(|translation| translation.title.clone())
    {
        Some(variant_title) if !variant_title.trim().is_empty() => {
            format!("{base_title} / {}", variant_title.trim())
        }
        _ => base_title,
    };

    Ok(AddCartLineItemInput {
        product_id: Some(product_model.id),
        variant_id: Some(variant.id),
        shipping_profile_slug: Some(effective_shipping_profile_slug(
            product_model.shipping_profile_slug.as_deref(),
            &product_model.metadata,
            variant.shipping_profile_slug.as_deref(),
        )),
        sku: variant.sku.clone(),
        title,
        quantity: input.quantity,
        unit_price: resolved_price.amount,
        metadata: merge_metadata(
            input.metadata,
            seller_snapshot_metadata(product_model.seller_id.as_deref()),
        ),
    })
}

fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

fn pick_variant_translation<'a>(
    translations: &'a [variant_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a variant_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

async fn validate_store_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    let Some(variant) = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
    else {
        return Ok(());
    };

    validate_store_variant_inventory(
        db,
        tenant_id,
        &variant,
        requested_quantity,
        public_channel_slug,
    )
    .await
}

async fn validate_store_variant_inventory(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    if variant.inventory_policy == "continue" {
        return Ok(());
    }

    let available_inventory = load_available_inventory_for_variant_in_public_channel(
        db,
        tenant_id,
        variant.id,
        public_channel_slug,
    )
    .await
    .map_err(|err| Error::BadRequest(err.to_string()))?;
    if available_inventory < requested_quantity {
        return Err(Error::BadRequest(format!(
            "Variant {} does not have enough available inventory for the current channel",
            variant.id
        )));
    }

    Ok(())
}

fn map_cart_error(error: CartError) -> Error {
    match error {
        CartError::CartNotFound(_) | CartError::CartLineItemNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

fn default_metadata() -> Value {
    json!({})
}

fn deserialize_patch_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn normalize_store_seller_scope(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_store_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn seller_snapshot_metadata(seller_id: Option<&str>) -> Value {
    let seller_id = normalize_store_seller_id(seller_id);
    let seller_scope = seller_id
        .as_deref()
        .and_then(|value| normalize_store_seller_scope(Some(value)));

    json!({
        "seller": {
            "id": seller_id,
            "scope": seller_scope,
        }
    })
}

fn cart_context_metadata(cart: &CartResponse, context: &StoreContextResponse) -> Value {
    json!({
        "cart_context": {
            "channel_id": cart.channel_id,
            "channel_slug": cart.channel_slug.clone(),
            "region_id": context.region.as_ref().map(|region| region.id),
            "country_code": cart.country_code.clone(),
            "locale": context.locale.clone(),
            "currency_code": cart.currency_code.clone(),
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "shipping_selections": current_shipping_selections(cart),
            "customer_id": cart.customer_id,
            "email": cart.email.clone(),
        }
    })
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct StoreListProductsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreContextQuery {
    pub cart_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCreateCartInput {
    pub email: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StoreCartResponse {
    pub cart: CartResponse,
    pub context: StoreContextResponse,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreUpdateCartInput {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub region_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub country_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub locale: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub selected_shipping_option_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub shipping_selections: Option<Vec<StoreCartShippingSelectionInput>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCreatePaymentCollectionInput {
    pub cart_id: Uuid,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCompleteCartInput {
    pub shipping_option_id: Option<Uuid>,
    pub shipping_selections: Option<Vec<StoreCartShippingSelectionInput>>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_true")]
    pub create_fulfillment: bool,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreAddCartLineItemInput {
    pub variant_id: Uuid,
    pub quantity: i32,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreUpdateCartLineItemInput {
    pub quantity: i32,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
struct StoreCartContextPatch {
    email: Option<Option<String>>,
    region_id: Option<Option<Uuid>>,
    country_code: Option<Option<String>>,
    locale: Option<Option<String>>,
    selected_shipping_option_id: Option<Option<Uuid>>,
    shipping_selections: Option<Vec<crate::dto::CartShippingSelectionInput>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestedCartContext {
    email: Option<String>,
    region_id: Option<Uuid>,
    country_code: Option<String>,
    locale: Option<String>,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Vec<crate::dto::CartShippingSelectionInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct StoreCartShippingSelectionInput {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

impl From<StoreCartShippingSelectionInput> for crate::dto::CartShippingSelectionInput {
    fn from(value: StoreCartShippingSelectionInput) -> Self {
        Self {
            shipping_profile_slug: value.shipping_profile_slug,
            seller_id: value.seller_id,
            seller_scope: value.seller_scope,
            selected_shipping_option_id: value.selected_shipping_option_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cart_context_metadata, checkout_actor_id, ensure_store_cart_access, merge_metadata,
        requested_cart_context, resolve_store_line_item_input, RequestedCartContext,
        StoreAddCartLineItemInput, StoreCartContextPatch, MODULE_SLUG,
    };
    use axum::body::{to_bytes, Body};
    use axum::extract::{Path, State};
    use axum::http::{Request, StatusCode};
    use axum::middleware::{from_fn_with_state, Next};
    use axum::response::Response;
    use axum::Router;
    use loco_rs::app::{AppContext, SharedStore};
    use loco_rs::cache;
    use loco_rs::environment::Environment;
    use loco_rs::storage::{self, Storage};
    use loco_rs::tests_cfg::config::test_config;
    use rust_decimal::Decimal;
    use rustok_api::context::ChannelResolutionSource;
    use rustok_api::RequestContext;
    use rustok_api::{
        AuthContext, AuthContextExtension, ChannelContext, ChannelContextExtension, TenantContext,
        TenantContextExtension,
    };
    use rustok_cart::dto::SetCartAdjustmentInput;
    use rustok_core::events::EventTransport;
    use rustok_core::Permission;
    use rustok_pricing::PriceResolutionContext;
    use rustok_region::dto::{CreateRegionInput, RegionResponse, RegionTranslationInput};
    use rustok_region::services::RegionService;
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::{mock_transactional_event_bus, MockEventTransport};
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    use serde_json::json;
    use std::str::FromStr;
    use std::sync::Arc;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::dto::{
        AddCartLineItemInput, CartResponse, CreateCartInput, CreateProductInput,
        CreateShippingOptionInput, CreateVariantInput, PriceInput, ProductTranslationInput,
        ShippingOptionTranslationInput, StoreContextResponse,
    };
    use crate::{CartService, CatalogService, CustomerService, FulfillmentService, PricingService};
    use rustok_customer::dto::CreateCustomerInput;

    mod support {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support.rs"));
    }

    fn sample_cart(customer_id: Option<Uuid>) -> CartResponse {
        CartResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            channel_id: None,
            channel_slug: None,
            customer_id,
            email: Some("buyer@example.com".to_string()),
            region_id: None,
            country_code: None,
            locale_code: None,
            selected_shipping_option_id: None,
            status: "active".to_string(),
            currency_code: "USD".to_string(),
            subtotal_amount: Decimal::ZERO,
            adjustment_total: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            line_items: Vec::new(),
            adjustments: Vec::new(),
            delivery_groups: Vec::new(),
        }
    }

    fn pricing_context(currency_code: &str, quantity: i32) -> PriceResolutionContext {
        PriceResolutionContext {
            currency_code: currency_code.to_ascii_uppercase(),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            quantity: Some(quantity),
        }
    }

    #[test]
    fn guest_cart_allows_missing_customer_context() {
        let cart = sample_cart(None);
        assert!(ensure_store_cart_access(&cart, None).is_ok());
    }

    #[test]
    fn customer_owned_cart_allows_matching_customer() {
        let customer_id = Uuid::new_v4();
        let cart = sample_cart(Some(customer_id));
        assert!(ensure_store_cart_access(&cart, Some(customer_id)).is_ok());
    }

    #[test]
    fn customer_owned_cart_rejects_missing_customer_context() {
        let cart = sample_cart(Some(Uuid::new_v4()));
        let error = ensure_store_cart_access(&cart, None).expect_err("customer auth required");
        assert_eq!(error.to_string(), "Cart belongs to another customer");
    }

    #[test]
    fn customer_owned_cart_rejects_different_customer() {
        let cart = sample_cart(Some(Uuid::new_v4()));
        let error = ensure_store_cart_access(&cart, Some(Uuid::new_v4()))
            .expect_err("foreign customer access must be rejected");
        assert_eq!(error.to_string(), "Cart belongs to another customer");
    }

    #[test]
    fn guest_checkout_uses_nil_actor_without_auth() {
        assert_eq!(checkout_actor_id(None), Uuid::nil());
    }

    #[test]
    fn authenticated_checkout_uses_user_actor() {
        let user_id = Uuid::new_v4();
        let auth = AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        assert_eq!(checkout_actor_id(Some(&auth)), user_id);
    }

    fn sample_request_context(locale: &str) -> RequestContext {
        RequestContext {
            tenant_id: Uuid::new_v4(),
            user_id: None,
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: locale.to_string(),
        }
    }

    fn sample_channel_context(slug: &str) -> ChannelContext {
        ChannelContext {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            slug: slug.to_string(),
            name: format!("Channel {slug}"),
            is_active: true,
            status: "active".to_string(),
            target_type: Some("web_domain".to_string()),
            target_value: Some(format!("{slug}.example.test")),
            settings: json!({}),
            resolution_source: ChannelResolutionSource::Host,
        }
    }

    async fn seed_channel_binding(
        db: &sea_orm::DatabaseConnection,
        channel: &ChannelContext,
        module_slug: &str,
        is_enabled: bool,
    ) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                channel.id.into(),
                channel.tenant_id.into(),
                channel.slug.clone().into(),
                channel.name.clone().into(),
                channel.is_active.into(),
                false.into(),
                channel.status.clone().into(),
                channel.settings.to_string().into(),
            ],
        ))
        .await
        .expect("channel should be inserted for test");

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO channel_module_bindings (id, channel_id, module_slug, is_enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                channel.id.into(),
                module_slug.into(),
                is_enabled.into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("channel module binding should be inserted for test");
    }

    async fn set_stock_location_channel_visibility(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        allowed_channel_slugs: &[&str],
    ) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE stock_locations SET metadata = ? WHERE tenant_id = ?",
            vec![
                json!({
                    "channel_visibility": {
                        "allowed_channel_slugs": allowed_channel_slugs
                    }
                })
                .to_string()
                .into(),
                tenant_id.into(),
            ],
        ))
        .await
        .expect("stock location visibility should be updated");
    }

    #[test]
    fn cart_context_patch_keeps_existing_values_when_fields_are_omitted() {
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let mut cart = sample_cart(None);
        cart.email = Some("keep@example.com".to_string());
        cart.region_id = Some(region_id);
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());
        cart.selected_shipping_option_id = Some(shipping_option_id);

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: None,
                region_id: None,
                country_code: None,
                locale: None,
                selected_shipping_option_id: None,
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: Some("keep@example.com".to_string()),
                region_id: Some(region_id),
                country_code: Some("DE".to_string()),
                locale: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option_id),
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn cart_context_patch_applies_explicit_values() {
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let cart = sample_cart(None);

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: Some(Some("set@example.com".to_string())),
                region_id: Some(Some(region_id)),
                country_code: Some(Some("fr".to_string())),
                locale: Some(Some("fr".to_string())),
                selected_shipping_option_id: Some(Some(shipping_option_id)),
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: Some("set@example.com".to_string()),
                region_id: Some(region_id),
                country_code: Some("fr".to_string()),
                locale: Some("fr".to_string()),
                selected_shipping_option_id: Some(shipping_option_id),
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn cart_context_patch_clears_country_when_region_is_explicitly_cleared() {
        let mut cart = sample_cart(None);
        cart.region_id = Some(Uuid::new_v4());
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: None,
                region_id: Some(None),
                country_code: None,
                locale: None,
                selected_shipping_option_id: None,
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: None,
                locale: Some("de".to_string()),
                selected_shipping_option_id: None,
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn cart_context_patch_can_clear_individual_fields_and_falls_back_to_request_locale() {
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let mut cart = sample_cart(None);
        cart.region_id = Some(region_id);
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());
        cart.selected_shipping_option_id = Some(shipping_option_id);

        let requested = requested_cart_context(
            &cart,
            &sample_request_context("en"),
            StoreCartContextPatch {
                email: Some(None),
                region_id: None,
                country_code: Some(None),
                locale: Some(None),
                selected_shipping_option_id: Some(None),
                shipping_selections: None,
            },
        );

        assert_eq!(
            requested,
            RequestedCartContext {
                email: None,
                region_id: Some(region_id),
                country_code: None,
                locale: Some("en".to_string()),
                selected_shipping_option_id: None,
                shipping_selections: Vec::new(),
            }
        );
    }

    #[test]
    fn merge_metadata_keeps_existing_fields_and_overrides_conflicts() {
        let merged = merge_metadata(
            json!({
                "source": "request",
                "cart_context": { "locale": "de", "currency_code": "EUR" }
            }),
            json!({
                "cart_context": { "locale": "en" },
                "attempt": 2
            }),
        );

        assert_eq!(
            merged,
            json!({
                "source": "request",
                "cart_context": { "locale": "en" },
                "attempt": 2
            })
        );
    }

    #[test]
    fn cart_context_metadata_embeds_storefront_context_for_payment_collection() {
        let tenant_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let region_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let mut cart = sample_cart(Some(customer_id));
        cart.channel_id = Some(channel_id);
        cart.channel_slug = Some("web-store".to_string());
        cart.region_id = Some(region_id);
        cart.country_code = Some("DE".to_string());
        cart.locale_code = Some("de".to_string());
        cart.selected_shipping_option_id = Some(shipping_option_id);

        let metadata = cart_context_metadata(
            &cart,
            &StoreContextResponse {
                region: Some(RegionResponse {
                    id: region_id,
                    tenant_id,
                    name: "Europe".to_string(),
                    currency_code: "EUR".to_string(),
                    tax_rate: Decimal::from(20),
                    tax_included: true,
                    countries: vec!["DE".to_string()],
                    metadata: json!({}),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }),
                locale: "de".to_string(),
                default_locale: "en".to_string(),
                available_locales: vec!["en".to_string(), "de".to_string()],
                currency_code: Some("EUR".to_string()),
            },
        );

        assert_eq!(
            metadata,
            json!({
                "cart_context": {
                    "channel_id": channel_id,
                    "channel_slug": "web-store",
                    "region_id": region_id,
                    "country_code": "DE",
                    "locale": "de",
                    "currency_code": "USD",
                    "selected_shipping_option_id": shipping_option_id,
                    "shipping_selections": [],
                    "customer_id": customer_id,
                    "email": "buyer@example.com"
                }
            })
        );
    }

    #[tokio::test]
    async fn store_cart_transport_persists_channel_snapshot() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        let channel_id = channel.id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "channel-cart@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&body).expect("create cart response should be JSON");
        assert_eq!(created_cart["cart"]["channel_id"], json!(channel_id));
        assert_eq!(created_cart["cart"]["channel_slug"], json!("web-store"));
    }

    #[tokio::test]
    async fn store_products_transport_rejects_disabled_channel_module() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, false).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/products")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("store products request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn store_products_transport_filters_channel_hidden_products() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());

        let mut visible_input = storefront_product_input();
        visible_input.translations[0].title = "Visible Product".to_string();
        visible_input.translations[0].handle = Some("visible-storefront-product-en".to_string());
        visible_input.translations[1].title = "Sichtbares Produkt".to_string();
        visible_input.translations[1].handle = Some("sichtbares-storefront-product-de".to_string());
        visible_input.variants[0].sku = Some("STOREFRONT-VISIBLE-SKU-1".to_string());
        let visible = catalog
            .create_product(tenant_id, actor_id, visible_input)
            .await
            .expect("visible product should be created");
        catalog
            .publish_product(tenant_id, actor_id, visible.id)
            .await
            .expect("visible product should be published");

        let mut hidden_input = storefront_product_input();
        hidden_input.translations[0].title = "Hidden Product".to_string();
        hidden_input.translations[0].handle = Some("hidden-storefront-product-en".to_string());
        hidden_input.translations[1].title = "Verstecktes Produkt".to_string();
        hidden_input.translations[1].handle = Some("verstecktes-storefront-product-de".to_string());
        hidden_input.variants[0].sku = Some("STOREFRONT-HIDDEN-SKU-1".to_string());
        hidden_input.metadata = json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["mobile-app"]
            }
        });
        let hidden = catalog
            .create_product(tenant_id, actor_id, hidden_input)
            .await
            .expect("hidden product should be created");
        catalog
            .publish_product(tenant_id, actor_id, hidden.id)
            .await
            .expect("hidden product should be published");

        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/products?locale=de")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("store products request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("store products body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("store products response should be JSON");
        let items = json["data"]
            .as_array()
            .expect("product list should be an array");
        assert_eq!(json["meta"]["total"], json!(1));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], json!("Sichtbares Produkt"));
    }

    #[tokio::test]
    async fn store_shipping_options_transport_filters_channel_hidden_options() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let fulfillment = FulfillmentService::new(db.clone());
        let visible_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Visible Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({}),
                },
            )
            .await
            .expect("visible shipping option should be created");
        fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Hidden Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("19.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({
                        "channel_visibility": {
                            "allowed_channel_slugs": ["mobile-app"]
                        }
                    }),
                },
            )
            .await
            .expect("hidden shipping option should be created");

        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/shipping-options?currency_code=eur&locale=de")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("shipping options response should be JSON");
        let options = json
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0]["id"], json!(visible_option.id));
    }

    #[tokio::test]
    async fn store_shipping_options_transport_filters_incompatible_shipping_profiles() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };

        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let mut product_input = storefront_product_input();
        product_input.metadata = json!({
            "shipping_profile": {
                "slug": "bulky"
            }
        });
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product should include variant");

        let fulfillment = FulfillmentService::new(db.clone());
        fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Default Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                    metadata: json!({
                        "shipping_profiles": {
                            "allowed_slugs": ["default"]
                        }
                    }),
                },
            )
            .await
            .expect("default shipping option should be created");
        let bulky_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Bulky Freight".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("29.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: Some(vec!["bulky".to_string()]),
                    metadata: json!({
                        "shipping_profiles": {
                            "allowed_slugs": ["bulky"]
                        }
                    }),
                },
            )
            .await
            .expect("bulky shipping option should be created");

        let cart_service = CartService::new(db.clone());
        let cart = cart_service
            .create_cart_with_channel(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: Some("de".to_string()),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-shipping-profile-filter" }),
                },
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("cart should be created");
        cart_service
            .add_line_item(
                tenant_id,
                cart.id,
                AddCartLineItemInput {
                    product_id: Some(published.id),
                    variant_id: Some(variant.id),
                    shipping_profile_slug: Some("bulky".to_string()),
                    sku: variant.sku.clone(),
                    title: variant.title.clone(),
                    quantity: 1,
                    unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                    metadata: json!({ "slot": 1 }),
                },
            )
            .await
            .expect("line item should be added");

        let mut channel = sample_channel_context("web-store");
        channel.id = channel_id;
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/shipping-options?cart_id={}&currency_code=eur&locale=de",
                        cart.id
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("shipping options response should be JSON");
        let options = json
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0]["id"], json!(bulky_option.id));
        assert_eq!(
            options[0]["allowed_shipping_profile_slugs"],
            json!(["bulky"])
        );
    }

    #[tokio::test]
    async fn store_update_cart_context_rejects_incompatible_shipping_profile_option() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };

        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let mut product_input = storefront_product_input();
        product_input.metadata = json!({
            "shipping_profile": {
                "slug": "bulky"
            }
        });
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product should include variant");

        let incompatible_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Default Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                    metadata: json!({
                        "shipping_profiles": {
                            "allowed_slugs": ["default"]
                        }
                    }),
                },
            )
            .await
            .expect("shipping option should be created");

        let cart_service = CartService::new(db.clone());
        let cart = cart_service
            .create_cart_with_channel(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: Some("de".to_string()),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-shipping-profile-update" }),
                },
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("cart should be created");
        cart_service
            .add_line_item(
                tenant_id,
                cart.id,
                AddCartLineItemInput {
                    product_id: Some(published.id),
                    variant_id: Some(variant.id),
                    shipping_profile_slug: Some("bulky".to_string()),
                    sku: variant.sku.clone(),
                    title: variant.title.clone(),
                    quantity: 1,
                    unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                    metadata: json!({ "slot": 1 }),
                },
            )
            .await
            .expect("line item should be added");

        let mut channel = sample_channel_context("web-store");
        channel.id = channel_id;
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{}", cart.id))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": incompatible_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should complete");

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("update cart body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected update cart body: {}",
            String::from_utf8_lossy(&body)
        );
        let updated_cart: serde_json::Value =
            serde_json::from_slice(&body).expect("updated cart response should be JSON");
        assert_eq!(
            updated_cart["cart"]["selected_shipping_option_id"],
            json!(null)
        );
        assert_eq!(
            updated_cart["cart"]["delivery_groups"][0]["shipping_profile_slug"],
            json!("bulky")
        );
        assert_eq!(
            updated_cart["cart"]["delivery_groups"][0]["available_shipping_options"],
            json!([])
        );
    }

    #[tokio::test]
    async fn store_cart_line_item_transport_rejects_channel_hidden_product() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let mut hidden_input = storefront_product_input();
        hidden_input.translations[0].handle = Some("channel-hidden-variant-en".to_string());
        hidden_input.translations[1].handle = Some("channel-hidden-variant-de".to_string());
        hidden_input.variants[0].sku = Some("STOREFRONT-CHANNEL-HIDDEN-SKU-1".to_string());
        hidden_input.metadata = json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["mobile-app"]
            }
        });
        let hidden = catalog
            .create_product(tenant_id, actor_id, hidden_input)
            .await
            .expect("hidden product should be created");
        let hidden = catalog
            .publish_product(tenant_id, actor_id, hidden.id)
            .await
            .expect("hidden product should be published");
        let variant = hidden
            .variants
            .first()
            .expect("hidden product should have variant");

        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db),
            tenant,
            None,
            Some(channel),
        );

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": {}
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should complete");

        assert_eq!(add_response.status(), StatusCode::NOT_FOUND);
    }

    fn storefront_product_input() -> CreateProductInput {
        CreateProductInput {
            translations: vec![
                ProductTranslationInput {
                    locale: "en".to_string(),
                    title: "Storefront Product".to_string(),
                    description: Some("English description".to_string()),
                    handle: Some("storefront-product-en".to_string()),
                    meta_title: None,
                    meta_description: None,
                },
                ProductTranslationInput {
                    locale: "de".to_string(),
                    title: "Storefront Produkt".to_string(),
                    description: Some("German description".to_string()),
                    handle: Some("storefront-product-de".to_string()),
                    meta_title: None,
                    meta_description: None,
                },
            ],
            options: vec![],
            variants: vec![CreateVariantInput {
                sku: Some("STOREFRONT-SKU-1".to_string()),
                barcode: None,
                shipping_profile_slug: None,
                option1: Some("Default".to_string()),
                option2: None,
                option3: None,
                prices: vec![PriceInput {
                    currency_code: "EUR".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: Decimal::from_str("19.99").expect("valid decimal"),
                    compare_at_amount: None,
                }],
                inventory_quantity: 0,
                inventory_policy: "deny".to_string(),
                weight: None,
                weight_unit: None,
            }],
            seller_id: None,
            vendor: Some("Storefront Vendor".to_string()),
            product_type: Some("physical".to_string()),
            shipping_profile_slug: None,
            tags: vec![],
            publish: false,
            metadata: json!({}),
        }
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> AppContext {
        let shared_store = Arc::new(SharedStore::default());
        let event_transport: Arc<dyn EventTransport> = Arc::new(MockEventTransport::new());
        shared_store.insert(event_transport);

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store,
        }
    }

    async fn seed_store_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                tenant_id.into(),
                "Store Test Tenant".into(),
                format!("store-test-{tenant_id}").into(),
                sea_orm::Value::String(None),
                json!({}).to_string().into(),
                "en".into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");

        for (locale, name, native_name, is_default) in [
            ("en", "English", "English", true),
            ("de", "German", "Deutsch", false),
        ] {
            db.execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    locale.into(),
                    name.into(),
                    native_name.into(),
                    is_default.into(),
                    true.into(),
                    sea_orm::Value::String(None),
                ],
            ))
            .await
            .expect("tenant locale should be inserted");
        }

        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                "commerce".into(),
                true.into(),
                json!({}).to_string().into(),
            ],
        ))
        .await
        .expect("commerce module should be enabled for tenant");
    }

    async fn create_customer_for_user(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        user_id: Uuid,
        email: &str,
    ) -> Uuid {
        CustomerService::new(db.clone())
            .create_customer(
                tenant_id,
                CreateCustomerInput {
                    user_id: Some(user_id),
                    email: email.to_string(),
                    first_name: Some("Store".to_string()),
                    last_name: Some("Customer".to_string()),
                    phone: None,
                    locale: Some("de".to_string()),
                    metadata: json!({}),
                },
            )
            .await
            .expect("customer should be created")
            .id
    }

    #[derive(Clone)]
    struct TransportRequestContext {
        tenant: TenantContext,
        auth: Option<AuthContext>,
        channel: Option<ChannelContext>,
    }

    async fn inject_transport_context(
        State(context): State<TransportRequestContext>,
        mut req: axum::extract::Request,
        next: Next,
    ) -> Response {
        req.extensions_mut()
            .insert(TenantContextExtension(context.tenant));
        if let Some(auth) = context.auth {
            req.extensions_mut().insert(AuthContextExtension(auth));
        }
        if let Some(channel) = context.channel {
            req.extensions_mut()
                .insert(ChannelContextExtension(channel));
        }
        next.run(req).await
    }

    fn commerce_transport_router(ctx: AppContext, tenant: TenantContext) -> Router {
        commerce_transport_router_with_auth(ctx, tenant, None)
    }

    fn commerce_transport_router_with_auth(
        ctx: AppContext,
        tenant: TenantContext,
        auth: Option<AuthContext>,
    ) -> Router {
        commerce_transport_router_with_context(ctx, tenant, auth, None)
    }

    fn commerce_transport_router_with_context(
        ctx: AppContext,
        tenant: TenantContext,
        auth: Option<AuthContext>,
        channel: Option<ChannelContext>,
    ) -> Router {
        let routes = crate::controllers::routes();
        let mut router = Router::new();
        for handler in routes.handlers {
            router = router.route(&handler.uri, handler.method.with_state(ctx.clone()));
        }

        router.layer(from_fn_with_state(
            TransportRequestContext {
                tenant,
                auth,
                channel,
            },
            inject_transport_context,
        ))
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_uses_backend_variant_title_and_price() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;

        let created = service
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 2);

        let resolved = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 2,
                metadata: json!({ "source": "store-line-item-test" }),
            },
        )
        .await
        .expect("store line item should resolve from backend catalog");

        assert_eq!(resolved.product_id, Some(published.id));
        assert_eq!(resolved.variant_id, Some(variant.id));
        assert_eq!(resolved.sku.as_deref(), Some("STOREFRONT-SKU-1"));
        assert_eq!(resolved.title, "Storefront Produkt / Default");
        assert_eq!(
            resolved.unit_price,
            Decimal::from_str("19.99").expect("valid decimal")
        );
        assert_eq!(resolved.quantity, 2);
        assert_eq!(
            resolved.metadata,
            json!({
                "seller": { "id": null, "scope": null },
                "source": "store-line-item-test"
            })
        );
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_rejects_missing_price_for_cart_currency() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let created = service
            .create_product(tenant_id, actor_id, storefront_product_input())
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("USD", 1);

        let error = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect_err("store line item must reject missing price in cart currency");

        assert_eq!(
            error.to_string(),
            format!(
                "No storefront price for variant {} in currency USD",
                variant.id
            )
        );
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_falls_back_to_first_product_translation_when_locale_missing(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;

        let created = service
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 1);

        let resolved = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "fr",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect("store line item should fall back to an existing product translation");

        assert_eq!(resolved.product_id, Some(published.id));
        assert_eq!(resolved.variant_id, Some(variant.id));
        assert_eq!(resolved.sku.as_deref(), Some("STOREFRONT-SKU-1"));
        assert_eq!(resolved.title, "Storefront Produkt");
        assert_eq!(
            resolved.unit_price,
            Decimal::from_str("19.99").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_returns_not_found_for_unknown_variant() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 1);

        let error = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            None,
            StoreAddCartLineItemInput {
                variant_id: Uuid::new_v4(),
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect_err("unknown variant must not resolve");

        assert_eq!(error.to_string(), "not found");
    }

    #[tokio::test]
    async fn storefront_line_item_resolution_rejects_quantity_above_channel_visible_inventory() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let mut input = storefront_product_input();
        input.variants[0].inventory_quantity = 5;
        let created = service
            .create_product(tenant_id, actor_id, input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let pricing_service = PricingService::new(db.clone(), mock_transactional_event_bus());
        let pricing_context = pricing_context("EUR", 1);
        set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;

        let error = resolve_store_line_item_input(
            &db,
            tenant_id,
            &pricing_service,
            &pricing_context,
            "de",
            "en",
            Some("web-store"),
            StoreAddCartLineItemInput {
                variant_id: variant.id,
                quantity: 1,
                metadata: json!({}),
            },
        )
        .await
        .expect_err("hidden inventory should reject storefront line item resolution");

        assert_eq!(
            error.to_string(),
            format!(
                "Variant {} does not have enough available inventory for the current channel",
                variant.id
            )
        );
    }

    #[tokio::test]
    async fn store_product_transport_uses_channel_visible_inventory() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let service = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut channel = sample_channel_context("web-store");
        channel.tenant_id = tenant_id;

        let mut input = storefront_product_input();
        input.variants[0].inventory_quantity = 7;
        let created = service
            .create_product(tenant_id, actor_id, input)
            .await
            .expect("product should be created");
        let published = service
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        set_stock_location_channel_visibility(&db, tenant_id, &["mobile-app"]).await;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let request_context = RequestContext {
            tenant_id,
            user_id: None,
            channel_id: Some(channel.id),
            channel_slug: Some(channel.slug.clone()),
            channel_resolution_source: Some(ChannelResolutionSource::Host),
            locale: "de".to_string(),
        };

        let product = super::show_product(
            State(test_app_context(db)),
            tenant,
            request_context,
            Path(published.id),
        )
        .await
        .expect("store product handler should succeed")
        .0;

        assert_eq!(product.variants.len(), 1);
        assert_eq!(product.variants[0].inventory_quantity, 0);
        assert!(!product.variants[0].in_stock);
    }

    #[tokio::test]
    async fn store_cart_transport_uses_tristate_update_semantics_end_to_end() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_status = create_response.status();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        assert_eq!(
            create_status,
            StatusCode::CREATED,
            "unexpected create cart body: {}",
            String::from_utf8_lossy(&create_body)
        );

        let created: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(created["cart"]["email"], json!("buyer@example.com"));
        assert_eq!(created["cart"]["locale_code"], json!("de"));
        assert_eq!(created["context"]["locale"], json!("de"));

        let update_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "en")
                    .body(Body::from(
                        json!({
                            "email": null,
                            "locale": null
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        let update_status = update_response.status();
        let update_body = to_bytes(update_response.into_body(), usize::MAX)
            .await
            .expect("update cart body should read");
        assert_eq!(
            update_status,
            StatusCode::OK,
            "unexpected update cart body: {}",
            String::from_utf8_lossy(&update_body)
        );

        let updated: serde_json::Value =
            serde_json::from_slice(&update_body).expect("update cart response should be JSON");
        assert_eq!(updated["cart"]["id"], json!(cart_id));
        assert!(updated["cart"]["email"].is_null());
        assert_eq!(updated["cart"]["locale_code"], json!("en"));
        assert_eq!(updated["context"]["locale"], json!("en"));
    }

    #[tokio::test]
    async fn store_cart_transport_rejects_currency_mismatch_for_region() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let region = RegionService::new(db.clone())
            .create_region(
                tenant_id,
                CreateRegionInput {
                    translations: vec![RegionTranslationInput {
                        locale: "en".to_string(),
                        name: "Europe".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                    tax_included: true,
                    countries: vec!["de".to_string()],
                    metadata: json!({ "source": "store-cart-region-mismatch" }),
                },
            )
            .await
            .expect("region should be created");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "region_id": region.id,
                            "currency_code": "usd",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should complete");
        let status = create_response.status();
        let body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let body_text = String::from_utf8_lossy(&body);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected create cart body: {body_text}",
        );
        assert!(
            body_text.contains("USD"),
            "body should mention requested currency: {body_text}"
        );
        assert!(
            body_text.contains("EUR"),
            "body should mention region currency: {body_text}"
        );
        assert!(
            body_text.contains(&region.id.to_string()),
            "body should mention conflicting region: {body_text}"
        );
    }

    #[tokio::test]
    async fn store_shipping_options_transport_uses_cart_context_currency_over_query_drift() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let fulfillment = FulfillmentService::new(db.clone());
        let eur_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "EU Standard".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "store-shipping-options-eur" }),
                },
            )
            .await
            .expect("EUR shipping option should be created");
        let usd_option = fulfillment
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "US Express".to_string(),
                    }],
                    currency_code: "usd".to_string(),
                    amount: Decimal::from_str("19.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "store-shipping-options-usd" }),
                },
            )
            .await
            .expect("USD shipping option should be created");
        let app = commerce_transport_router(test_app_context(db), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let shipping_options_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/shipping-options?cart_id={cart_id}&currency_code=usd"
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        let status = shipping_options_response.status();
        let body = to_bytes(shipping_options_response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected shipping options body: {}",
            String::from_utf8_lossy(&body)
        );

        let shipping_options: serde_json::Value =
            serde_json::from_slice(&body).expect("shipping options response should be JSON");
        let options = shipping_options
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1, "cart context should override query drift");
        assert_eq!(options[0]["id"], json!(eur_option.id));
        assert_eq!(options[0]["currency_code"], json!("EUR"));
        assert_ne!(options[0]["id"], json!(usd_option.id));
    }

    #[tokio::test]
    async fn store_cart_line_item_transport_resolves_backend_title_and_price() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 2,
                            "metadata": { "source": "transport-line-item-test" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        let line_item_status = line_item_response.status();
        let line_item_body = to_bytes(line_item_response.into_body(), usize::MAX)
            .await
            .expect("line item body should read");
        assert_eq!(
            line_item_status,
            StatusCode::OK,
            "unexpected add line item body: {}",
            String::from_utf8_lossy(&line_item_body)
        );

        let updated_cart: serde_json::Value =
            serde_json::from_slice(&line_item_body).expect("updated cart should be JSON");
        assert_eq!(
            updated_cart["line_items"][0]["variant_id"],
            json!(variant.id)
        );
        assert_eq!(
            updated_cart["line_items"][0]["product_id"],
            json!(published.id)
        );
        assert_eq!(
            updated_cart["line_items"][0]["sku"],
            json!("STOREFRONT-SKU-1")
        );
        assert_eq!(
            updated_cart["line_items"][0]["title"],
            json!("Storefront Produkt / Default")
        );
        assert_eq!(updated_cart["line_items"][0]["unit_price"], json!("19.99"));
        assert_eq!(updated_cart["line_items"][0]["quantity"], json!(2));
        assert_eq!(
            updated_cart["line_items"][0]["metadata"],
            json!({
                "seller": { "id": null, "scope": null },
                "source": "transport-line-item-test"
            })
        );
    }

    #[tokio::test]
    async fn store_cart_line_item_transport_returns_not_found_for_unknown_variant() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let app = commerce_transport_router(test_app_context(db), tenant);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": Uuid::new_v4(),
                            "quantity": 1,
                            "metadata": {}
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should complete");

        assert_eq!(line_item_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn store_payment_collection_transport_reuses_active_collection_and_preserves_cart_context_metadata(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Standard".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "transport-checkout-test-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-payment-test-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let create_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-payment-test" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        let create_collection_status = create_collection_response.status();
        let create_collection_body = to_bytes(create_collection_response.into_body(), usize::MAX)
            .await
            .expect("payment collection body should read");
        assert_eq!(
            create_collection_status,
            StatusCode::CREATED,
            "unexpected payment collection body: {}",
            String::from_utf8_lossy(&create_collection_body)
        );

        let first_collection: serde_json::Value = serde_json::from_slice(&create_collection_body)
            .expect("payment collection response should be JSON");
        assert_eq!(first_collection["status"], json!("pending"));
        assert_eq!(first_collection["currency_code"], json!("EUR"));
        assert_eq!(
            first_collection["metadata"]["source"],
            json!("transport-payment-test")
        );
        assert_eq!(
            first_collection["metadata"]["cart_context"]["locale"],
            json!("de")
        );
        assert_eq!(
            first_collection["metadata"]["cart_context"]["currency_code"],
            json!("EUR")
        );
        assert_eq!(
            first_collection["metadata"]["cart_context"]["email"],
            json!("buyer@example.com")
        );

        let reuse_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-payment-test-retry" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("retry payment collection request should succeed");
        let reuse_collection_status = reuse_collection_response.status();
        let reuse_collection_body = to_bytes(reuse_collection_response.into_body(), usize::MAX)
            .await
            .expect("reused payment collection body should read");
        assert_eq!(
            reuse_collection_status,
            StatusCode::OK,
            "unexpected reused payment collection body: {}",
            String::from_utf8_lossy(&reuse_collection_body)
        );

        let reused_collection: serde_json::Value = serde_json::from_slice(&reuse_collection_body)
            .expect("reused payment collection response should be JSON");
        assert_eq!(reused_collection["id"], first_collection["id"]);
        assert_eq!(reused_collection["metadata"], first_collection["metadata"]);
    }

    #[tokio::test]
    async fn store_checkout_transport_end_to_end_preserves_updated_cart_context() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let region = RegionService::new(db.clone())
            .create_region(
                tenant_id,
                CreateRegionInput {
                    translations: vec![RegionTranslationInput {
                        locale: "en".to_string(),
                        name: "Europe".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                    tax_included: true,
                    countries: vec!["de".to_string()],
                    metadata: json!({ "source": "store-checkout-flow-region" }),
                },
            )
            .await
            .expect("region should be created");
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Standard".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "store-checkout-flow-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db), tenant);

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "buyer@example.com",
                            "currency_code": "eur",
                            "locale": "en"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "checkout@example.com",
                            "region_id": region.id,
                            "country_code": "de",
                            "locale": "de",
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        let update_cart_status = update_cart_response.status();
        let update_cart_body = to_bytes(update_cart_response.into_body(), usize::MAX)
            .await
            .expect("update cart body should read");
        assert_eq!(
            update_cart_status,
            StatusCode::OK,
            "unexpected update cart body: {}",
            String::from_utf8_lossy(&update_cart_body)
        );
        let updated_cart: serde_json::Value =
            serde_json::from_slice(&update_cart_body).expect("update cart response should be JSON");
        assert_eq!(updated_cart["cart"]["email"], json!("checkout@example.com"));
        assert_eq!(updated_cart["cart"]["country_code"], json!("DE"));
        assert_eq!(updated_cart["cart"]["locale_code"], json!("de"));
        assert_eq!(updated_cart["cart"]["region_id"], json!(region.id));
        assert_eq!(
            updated_cart["cart"]["selected_shipping_option_id"],
            json!(null)
        );
        assert_eq!(updated_cart["context"]["locale"], json!("de"));
        assert_eq!(updated_cart["context"]["region"]["id"], json!(region.id));

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "store-checkout-flow-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let shipping_options_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/shipping-options?cart_id={cart_id}&currency_code=usd"
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should succeed");
        let shipping_options_status = shipping_options_response.status();
        let shipping_options_body = to_bytes(shipping_options_response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        assert_eq!(
            shipping_options_status,
            StatusCode::OK,
            "unexpected shipping options body: {}",
            String::from_utf8_lossy(&shipping_options_body)
        );
        let shipping_options: serde_json::Value = serde_json::from_slice(&shipping_options_body)
            .expect("shipping options response should be JSON");
        let options = shipping_options
            .as_array()
            .expect("shipping options should be an array");
        assert_eq!(options.len(), 1);
        assert_eq!(options[0]["id"], json!(shipping_option.id));

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "store-checkout-flow-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        let payment_collection_status = payment_collection_response.status();
        let payment_collection_body = to_bytes(payment_collection_response.into_body(), usize::MAX)
            .await
            .expect("payment collection body should read");
        assert_eq!(
            payment_collection_status,
            StatusCode::CREATED,
            "unexpected payment collection body: {}",
            String::from_utf8_lossy(&payment_collection_body)
        );
        let payment_collection: serde_json::Value =
            serde_json::from_slice(&payment_collection_body)
                .expect("payment collection response should be JSON");
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["region_id"],
            json!(region.id)
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["country_code"],
            json!("DE")
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["locale"],
            json!("de")
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["selected_shipping_option_id"],
            json!(shipping_option.id)
        );
        assert_eq!(
            payment_collection["metadata"]["cart_context"]["email"],
            json!("checkout@example.com")
        );

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "store-checkout-flow-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        let complete_checkout_status = complete_checkout_response.status();
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        assert_eq!(
            complete_checkout_status,
            StatusCode::OK,
            "unexpected complete checkout body: {}",
            String::from_utf8_lossy(&complete_checkout_body)
        );
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        assert_eq!(completed["cart"]["status"], json!("completed"));
        assert_eq!(completed["cart"]["country_code"], json!("DE"));
        assert_eq!(completed["cart"]["locale_code"], json!("de"));
        assert_eq!(completed["cart"]["region_id"], json!(region.id));
        assert_eq!(
            completed["cart"]["selected_shipping_option_id"],
            json!(shipping_option.id)
        );
        assert_eq!(completed["context"]["locale"], json!("de"));
        assert_eq!(completed["context"]["region"]["id"], json!(region.id));
        assert_eq!(completed["order"]["status"], json!("paid"));
        assert_eq!(
            completed["payment_collection"]["id"],
            payment_collection["id"]
        );
        assert_eq!(completed["payment_collection"]["status"], json!("captured"));
        assert!(completed["fulfillment"].is_null());
    }

    #[tokio::test]
    async fn store_checkout_transport_completes_guest_cart_with_existing_payment_and_no_fulfillment(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let app = commerce_transport_router(test_app_context(db.clone()), tenant.clone());
        let authed_app =
            commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(auth));

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "guest@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Standard".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "transport-checkout-test-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-checkout-test-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-checkout-test-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-checkout-test-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        let complete_checkout_status = complete_checkout_response.status();
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        assert_eq!(
            complete_checkout_status,
            StatusCode::OK,
            "unexpected complete checkout body: {}",
            String::from_utf8_lossy(&complete_checkout_body)
        );

        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        assert_eq!(completed["cart"]["status"], json!("completed"));
        assert_eq!(completed["order"]["status"], json!("paid"));
        assert_eq!(completed["payment_collection"]["status"], json!("captured"));
        assert!(completed["fulfillment"].is_null());
        assert_eq!(completed["context"]["locale"], json!("de"));

        let get_order_response = authed_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/store/orders/{}",
                        completed["order"]["id"]
                            .as_str()
                            .expect("order id should exist")
                    ))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get order request should complete");
        assert_eq!(get_order_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn store_shipping_options_transport_rejects_customer_owned_cart_for_foreign_customer() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let other_auth = AuthContext {
            user_id: other_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;
        create_customer_for_user(&db, tenant_id, other_user_id, "other@example.com").await;

        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let other_app =
            commerce_transport_router_with_auth(test_app_context(db), tenant, Some(other_auth));

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let shipping_options_response = other_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/shipping-options?cart_id={cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("shipping options request should complete");
        let status = shipping_options_response.status();
        let body = to_bytes(shipping_options_response.into_body(), usize::MAX)
            .await
            .expect("shipping options body should read");
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "unexpected shipping options body: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    async fn store_checkout_transport_rejects_customer_owned_cart_without_auth() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;

        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let guest_app = commerce_transport_router(test_app_context(db), tenant);

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");

        let complete_checkout_response = guest_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-checkout-owner-guard" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should complete");
        let status = complete_checkout_response.status();
        let body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "unexpected complete checkout body: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    async fn store_checkout_transport_carries_cart_channel_snapshot_into_order() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let mut channel = sample_channel_context("marketplace-eu");
        channel.tenant_id = tenant_id;
        let channel_id = channel.id;
        seed_channel_binding(&db, &channel, MODULE_SLUG, true).await;
        let app = commerce_transport_router_with_context(
            test_app_context(db.clone()),
            tenant,
            None,
            Some(channel),
        );

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "guest@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(created_cart["cart"]["channel_id"], json!(channel_id));
        assert_eq!(
            created_cart["cart"]["channel_slug"],
            json!("marketplace-eu")
        );
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Channel Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "channel-checkout-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "channel-checkout-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "channel-checkout-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "channel-checkout-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        assert_eq!(complete_checkout_response.status(), StatusCode::OK);

        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        assert_eq!(completed["order"]["channel_id"], json!(channel_id));
        assert_eq!(completed["order"]["channel_slug"], json!("marketplace-eu"));
    }

    #[tokio::test]
    async fn store_order_transport_returns_customer_owned_order_after_checkout() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let customer_id =
            create_customer_for_user(&db, tenant_id, actor_id, "customer@example.com").await;
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let app =
            commerce_transport_router_with_auth(test_app_context(db.clone()), tenant, Some(auth));

        let create_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "customer@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(created_cart["cart"]["customer_id"], json!(customer_id));
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Order Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "transport-order-test-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-order-test-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-order-test-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-order-test-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        assert_eq!(complete_checkout_response.status(), StatusCode::OK);
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        let order_id = completed["order"]["id"]
            .as_str()
            .expect("order id should be returned");

        let get_order_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/orders/{order_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get order request should succeed");
        let get_order_status = get_order_response.status();
        let get_order_body = to_bytes(get_order_response.into_body(), usize::MAX)
            .await
            .expect("get order body should read");
        assert_eq!(
            get_order_status,
            StatusCode::OK,
            "unexpected get order body: {}",
            String::from_utf8_lossy(&get_order_body)
        );

        let order: serde_json::Value =
            serde_json::from_slice(&get_order_body).expect("order response should be JSON");
        assert_eq!(order["id"], completed["order"]["id"]);
        assert_eq!(order["customer_id"], json!(customer_id));
        assert_eq!(order["status"], json!("paid"));
        assert_eq!(order["currency_code"], json!("EUR"));
    }

    #[tokio::test]
    async fn store_cart_transport_returns_typed_adjustments_and_totals() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, storefront_product_input())
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let cart_service = CartService::new(db.clone());
        let app = commerce_transport_router(test_app_context(db.clone()), tenant);
        let cart = cart_service
            .create_cart(
                tenant_id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("buyer@example.com".to_string()),
                    region_id: None,
                    country_code: None,
                    currency_code: "eur".to_string(),
                    metadata: json!({ "source": "store-cart-adjustment-cart" }),
                    locale_code: Some("de".to_string()),
                    selected_shipping_option_id: None,
                },
            )
            .await
            .expect("cart should be created");
        let cart = cart_service
            .add_line_item(
                tenant_id,
                cart.id,
                AddCartLineItemInput {
                    product_id: Some(published.id),
                    variant_id: Some(variant.id),
                    shipping_profile_slug: None,
                    sku: variant.sku.clone(),
                    title: variant.title.clone(),
                    quantity: 1,
                    unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                    metadata: json!({ "source": "store-cart-adjustment-line-item" }),
                },
            )
            .await
            .expect("line item should be added");
        let cart_id = cart.id;
        let line_item_id = cart.line_items[0].id;

        cart_service
            .set_adjustments(
                tenant_id,
                cart_id,
                vec![SetCartAdjustmentInput {
                    line_item_id: Some(line_item_id),
                    source_type: "Promotion".to_string(),
                    source_id: Some("promo-store".to_string()),
                    amount: Decimal::from_str("4.99").expect("valid decimal"),
                    metadata: json!({
                        "rule_code": "store-adjustment",
                        "display_label": "Store promotion"
                    }),
                }],
            )
            .await
            .expect("cart adjustment should be stored");

        let get_cart_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get cart request should succeed");
        let get_cart_status = get_cart_response.status();
        let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
            .await
            .expect("get cart body should read");
        assert_eq!(
            get_cart_status,
            StatusCode::OK,
            "unexpected get cart adjustment body: {}",
            String::from_utf8_lossy(&get_cart_body)
        );

        let cart: serde_json::Value =
            serde_json::from_slice(&get_cart_body).expect("cart response should be JSON");
        assert_eq!(cart["subtotal_amount"], json!("19.99"));
        assert_eq!(cart["adjustment_total"], json!("4.99"));
        assert_eq!(cart["total_amount"], json!("15.00"));
        assert_eq!(cart["adjustments"][0]["line_item_id"], json!(line_item_id));
        assert_eq!(cart["adjustments"][0]["source_type"], json!("promotion"));
        assert_eq!(cart["adjustments"][0]["source_id"], json!("promo-store"));
        assert_eq!(cart["adjustments"][0]["amount"], json!("4.99"));
        assert_eq!(cart["adjustments"][0]["currency_code"], json!("EUR"));
        assert_eq!(
            cart["adjustments"][0]["metadata"],
            json!({ "rule_code": "store-adjustment" })
        );
    }

    #[tokio::test]
    async fn store_order_transport_rejects_order_for_another_customer() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_CREATE, Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let other_auth = AuthContext {
            user_id: other_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        create_customer_for_user(&db, tenant_id, owner_user_id, "owner@example.com").await;
        create_customer_for_user(&db, tenant_id, other_user_id, "other@example.com").await;
        let actor_id = owner_user_id;
        let mut product_input = storefront_product_input();
        product_input.variants[0].inventory_quantity = 5;
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let created = catalog
            .create_product(tenant_id, actor_id, product_input)
            .await
            .expect("product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("product should be published");
        let variant = published
            .variants
            .first()
            .expect("published product must include variant");
        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let other_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant,
            Some(other_auth),
        );

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        let shipping_option = FulfillmentService::new(db.clone())
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    translations: vec![ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Ownership Shipping".to_string(),
                    }],
                    currency_code: "eur".to_string(),
                    amount: Decimal::from_str("9.99").expect("valid decimal"),
                    provider_id: None,
                    allowed_shipping_profile_slugs: None,
                    metadata: json!({ "source": "transport-order-ownership-shipping-option" }),
                },
            )
            .await
            .expect("shipping option should be created");
        let update_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "selected_shipping_option_id": shipping_option.id
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("update cart request should succeed");
        assert_eq!(update_cart_response.status(), StatusCode::OK);

        let add_line_item_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/line-items"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "variant_id": variant.id,
                            "quantity": 1,
                            "metadata": { "source": "transport-order-ownership-line-item" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("add line item request should succeed");
        assert_eq!(add_line_item_response.status(), StatusCode::OK);

        let payment_collection_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/payment-collections")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .header("x-medusa-locale", "de")
                    .body(Body::from(
                        json!({
                            "cart_id": cart_id,
                            "metadata": { "source": "transport-order-ownership-payment" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create payment collection request should succeed");
        assert_eq!(payment_collection_response.status(), StatusCode::CREATED);

        let complete_checkout_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/store/carts/{cart_id}/complete"))
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "create_fulfillment": false,
                            "metadata": { "source": "transport-order-ownership-complete" }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("complete checkout request should succeed");
        assert_eq!(complete_checkout_response.status(), StatusCode::OK);
        let complete_checkout_body = to_bytes(complete_checkout_response.into_body(), usize::MAX)
            .await
            .expect("complete checkout body should read");
        let completed: serde_json::Value = serde_json::from_slice(&complete_checkout_body)
            .expect("complete checkout response should be JSON");
        let order_id = completed["order"]["id"]
            .as_str()
            .expect("order id should be returned");

        let get_order_response = other_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/orders/{order_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get order request should complete");
        let get_order_status = get_order_response.status();
        let get_order_body = to_bytes(get_order_response.into_body(), usize::MAX)
            .await
            .expect("get order body should read");
        assert_eq!(
            get_order_status,
            StatusCode::UNAUTHORIZED,
            "unexpected get order body: {}",
            String::from_utf8_lossy(&get_order_body)
        );
    }

    #[tokio::test]
    async fn store_customer_me_transport_returns_customer_for_authenticated_user() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let customer_id =
            create_customer_for_user(&db, tenant_id, user_id, "customer-me@example.com").await;
        let app = commerce_transport_router_with_auth(test_app_context(db), tenant, Some(auth));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/customers/me")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get me request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("get me body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected get me body: {}",
            String::from_utf8_lossy(&body)
        );

        let customer: serde_json::Value =
            serde_json::from_slice(&body).expect("get me response should be JSON");
        assert_eq!(customer["id"], json!(customer_id));
        assert_eq!(customer["user_id"], json!(user_id));
        assert_eq!(customer["email"], json!("customer-me@example.com"));
        assert_eq!(customer["locale"], json!("de"));
    }

    #[tokio::test]
    async fn store_cart_transport_rejects_customer_owned_cart_for_another_customer() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let owner_user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        seed_store_tenant_context(&db, tenant_id).await;
        let tenant = TenantContext {
            id: tenant_id,
            name: "Store Test Tenant".to_string(),
            slug: format!("store-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let owner_auth = AuthContext {
            user_id: owner_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let other_auth = AuthContext {
            user_id: other_user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let owner_customer_id =
            create_customer_for_user(&db, tenant_id, owner_user_id, "cart-owner@example.com").await;
        create_customer_for_user(&db, tenant_id, other_user_id, "cart-other@example.com").await;
        let owner_app = commerce_transport_router_with_auth(
            test_app_context(db.clone()),
            tenant.clone(),
            Some(owner_auth),
        );
        let other_app =
            commerce_transport_router_with_auth(test_app_context(db), tenant, Some(other_auth));

        let create_cart_response = owner_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/store/carts")
                    .header("content-type", "application/json")
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::from(
                        json!({
                            "email": "cart-owner@example.com",
                            "currency_code": "eur",
                            "locale": "de"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("create cart request should succeed");
        assert_eq!(create_cart_response.status(), StatusCode::CREATED);
        let create_cart_body = to_bytes(create_cart_response.into_body(), usize::MAX)
            .await
            .expect("create cart body should read");
        let created_cart: serde_json::Value =
            serde_json::from_slice(&create_cart_body).expect("create cart response should be JSON");
        let cart_id = created_cart["cart"]["id"]
            .as_str()
            .expect("cart id should be returned");
        assert_eq!(
            created_cart["cart"]["customer_id"],
            json!(owner_customer_id)
        );

        let get_cart_response = other_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/store/carts/{cart_id}"))
                    .header("X-Tenant-ID", tenant_id.to_string())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("get cart request should complete");
        let get_cart_status = get_cart_response.status();
        let get_cart_body = to_bytes(get_cart_response.into_body(), usize::MAX)
            .await
            .expect("get cart body should read");
        assert_eq!(
            get_cart_status,
            StatusCode::UNAUTHORIZED,
            "unexpected get cart body: {}",
            String::from_utf8_lossy(&get_cart_body)
        );
    }
}
