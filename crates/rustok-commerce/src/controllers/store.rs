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
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    dto::{
        AddCartLineItemInput, CartResponse, CompleteCheckoutInput, CompleteCheckoutResponse,
        CreateCartInput, CustomerResponse, OrderResponse, PaymentCollectionResponse,
        RegionResponse, ResolveStoreContextInput, ShippingOptionResponse, StoreContextResponse,
    },
    entities::{price, product, product_translation, product_variant, variant_translation},
    search::product_translation_title_search_condition,
    CartService, CatalogService, CustomerService, FulfillmentService, OrderService, PaymentService,
    ProductResponse, RegionService, StoreContextService,
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
    let _requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();
    let locale = params
        .locale
        .as_deref()
        .unwrap_or(request_context.locale.as_str());

    let mut query = product::Entity::find()
        .filter(product::Column::TenantId.eq(tenant.id))
        .filter(product::Column::Status.eq("published"));

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

    let total = query
        .clone()
        .count(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let products = query
        .order_by_desc(product::Column::CreatedAt)
        .offset(pagination.offset())
        .limit(pagination.limit())
        .all(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let product_ids = products
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let translations = if product_ids.is_empty() {
        Vec::new()
    } else {
        product_translation::Entity::find()
            .filter(product_translation::Column::ProductId.is_in(product_ids))
            .filter(product_translation::Column::Locale.eq(locale))
            .all(&ctx.db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
    };

    let translation_map = translations
        .into_iter()
        .map(|translation| (translation.product_id, translation))
        .collect::<std::collections::HashMap<_, _>>();

    let items = products
        .into_iter()
        .map(|product| {
            let translation = translation_map.get(&product.id);
            ProductListItem {
                id: product.id,
                status: product.status.to_string(),
                title: translation
                    .map(|value| value.title.clone())
                    .unwrap_or_default(),
                handle: translation
                    .map(|value| value.handle.clone())
                    .unwrap_or_default(),
                vendor: product.vendor,
                product_type: product.product_type,
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
    Path(id): Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .get_product(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if product.status.to_string() != "published" {
        return Err(Error::NotFound);
    }

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
) -> Result<Json<Vec<RegionResponse>>> {
    let service = RegionService::new(ctx.db.clone());
    let regions = service
        .list_regions(tenant.id)
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
    request_context: RequestContext,
    Query(query): Query<StoreContextQuery>,
) -> Result<Json<Vec<ShippingOptionResponse>>> {
    let context = resolve_context(
        &ctx,
        tenant.id,
        &request_context,
        query.region_id,
        query.country_code.clone(),
        query.locale.clone(),
        query.currency_code.clone(),
    )
    .await?;

    let service = FulfillmentService::new(ctx.db.clone());
    let mut options = service
        .list_shipping_options(tenant.id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    if let Some(currency_code) = context.currency_code.as_deref() {
        options.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
    }

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
        .create_cart(
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
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

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
    Path(id): Path<Uuid>,
) -> Result<Json<CartResponse>> {
    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let cart = service
        .get_cart(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    ensure_store_cart_access(&cart, customer_id)?;
    Ok(Json(cart))
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
    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(map_cart_error)?;
    ensure_store_cart_access(&existing, customer_id)?;
    let resolved_input = resolve_store_line_item_input(
        &ctx.db,
        tenant.id,
        &existing.currency_code,
        &request_context.locale,
        input,
    )
    .await?;

    let cart = service
        .add_line_item(tenant.id, id, resolved_input)
        .await
        .map_err(map_cart_error)?;
    Ok(Json(cart))
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
    Path((id, line_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<StoreUpdateCartLineItemInput>,
) -> Result<Json<CartResponse>> {
    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let service = CartService::new(ctx.db.clone());
    let existing = service
        .get_cart(tenant.id, id)
        .await
        .map_err(map_cart_error)?;
    ensure_store_cart_access(&existing, customer_id)?;

    let cart = service
        .update_line_item_quantity(tenant.id, id, line_id, input.quantity)
        .await
        .map_err(map_cart_error)?;
    Ok(Json(cart))
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
    Path((id, line_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CartResponse>> {
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
    Ok(Json(cart))
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
    Json(input): Json<StoreCreatePaymentCollectionInput>,
) -> Result<(StatusCode, Json<PaymentCollectionResponse>)> {
    let customer_id = current_customer_id(&ctx, tenant.id, auth.0.as_ref()).await?;
    let cart_service = CartService::new(ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, input.cart_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    ensure_store_cart_access(&cart, customer_id)?;

    let service = PaymentService::new(ctx.db.clone());
    let collection = service
        .create_collection(
            tenant.id,
            rustok_payment::dto::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: input.metadata,
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
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Cart not found")
    )
)]
pub async fn complete_cart_checkout(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: rustok_api::AuthContext,
    Path(cart_id): Path<Uuid>,
    Json(input): Json<StoreCompleteCartInput>,
) -> Result<Json<CompleteCheckoutResponse>> {
    let cart_service = CartService::new(ctx.db.clone());
    let cart = cart_service
        .get_cart(tenant.id, cart_id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let customer_id = current_customer_id(&ctx, tenant.id, Some(&auth)).await?;
    ensure_store_cart_access(&cart, customer_id)?;

    let service =
        crate::CheckoutService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let response = service
        .complete_checkout(
            tenant.id,
            auth.user_id,
            CompleteCheckoutInput {
                cart_id,
                shipping_option_id: input.shipping_option_id,
                region_id: input.region_id,
                country_code: input.country_code,
                locale: input.locale,
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
    auth: rustok_api::AuthContext,
) -> Result<Json<CustomerResponse>> {
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
    auth: rustok_api::AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderResponse>> {
    let customer_id = current_customer_id(&ctx, tenant.id, Some(&auth))
        .await?
        .ok_or_else(|| Error::Unauthorized("Customer account required".to_string()))?;
    let service = OrderService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let order = service
        .get_order(tenant.id, id)
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

async fn resolve_store_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    currency_code: &str,
    locale: &str,
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

    let product_translation_model = product_translation::Entity::find()
        .filter(product_translation::Column::ProductId.eq(product_model.id))
        .filter(product_translation::Column::Locale.eq(locale))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    let fallback_translation_model = if product_translation_model.is_none() {
        product_translation::Entity::find()
            .filter(product_translation::Column::ProductId.eq(product_model.id))
            .order_by_asc(product_translation::Column::Locale)
            .one(db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
    } else {
        None
    };

    let variant_translation_model = variant_translation::Entity::find()
        .filter(variant_translation::Column::VariantId.eq(variant.id))
        .filter(variant_translation::Column::Locale.eq(locale))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let selected_price = price::Entity::find()
        .filter(price::Column::VariantId.eq(variant.id))
        .filter(price::Column::CurrencyCode.eq(currency_code.to_ascii_uppercase()))
        .order_by_asc(price::Column::RegionId)
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or_else(|| {
            Error::BadRequest(format!(
                "No storefront price for variant {} in currency {}",
                variant.id, currency_code
            ))
        })?;

    let base_title = product_translation_model
        .as_ref()
        .or(fallback_translation_model.as_ref())
        .map(|translation| translation.title.clone())
        .unwrap_or_else(|| {
            variant
                .sku
                .clone()
                .unwrap_or_else(|| format!("Variant {}", variant.id))
        });
    let title = match variant_translation_model.and_then(|translation| translation.title) {
        Some(variant_title) if !variant_title.trim().is_empty() => {
            format!("{base_title} / {}", variant_title.trim())
        }
        _ => base_title,
    };

    Ok(AddCartLineItemInput {
        product_id: Some(product_model.id),
        variant_id: Some(variant.id),
        sku: variant.sku.clone(),
        title,
        quantity: input.quantity,
        unit_price: selected_price.amount,
        metadata: input.metadata,
    })
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
pub struct StoreCreatePaymentCollectionInput {
    pub cart_id: Uuid,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCompleteCartInput {
    pub shipping_option_id: Option<Uuid>,
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
