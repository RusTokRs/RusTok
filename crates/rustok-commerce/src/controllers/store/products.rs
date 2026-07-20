use axum::{
    Json,
    extract::{Path, Query, State},
};
use rustok_api::{OptionalAuthContext, PortActor, PortContext, RequestContext, TenantContext};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_fulfillment::FulfillmentService;
use rustok_product::{
    CatalogService,
    entities::{product, product_translation},
};
use rustok_region::{RegionListRequest, RegionReadPort};
use rustok_web::{HttpError, HttpResult};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::{
    super::common::{PaginatedResponse, PaginationMeta},
    StoreContextQuery, StoreListProductsParams,
};
use crate::controllers::{CommerceHttpRuntime, products::ProductListItem};
use crate::{
    dto::{ProductResponse, RegionResponse, ShippingOptionResponse},
    storefront_channel::{
        apply_public_channel_inventory_to_product, is_metadata_visible_for_public_channel,
        public_channel_slug_from_request,
    },
    storefront_shipping::{
        is_shipping_option_compatible_with_profiles, load_cart_shipping_profile_slugs,
        shipping_profile_slug_from_product_metadata,
    },
};

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Query(params): Query<StoreListProductsParams>,
) -> HttpResult<Json<PaginatedResponse<ProductListItem>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

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
        query = query.filter(crate::search::product_translation_title_search_condition(
            runtime.db().get_database_backend(),
            locale,
            search,
        ));
    }

    let visible_products = query
        .order_by_desc(product::Column::PublishedAt)
        .order_by_desc(product::Column::CreatedAt)
        .all(runtime.db())
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?
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
            .all(runtime.db())
            .await
            .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?
    };

    let mut translation_map =
        std::collections::HashMap::<Uuid, Vec<product_translation::Model>>::new();
    for translation in translations {
        translation_map
            .entry(translation.product_id)
            .or_default()
            .push(translation);
    }
    let catalog = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product_tags = catalog
        .load_product_tag_map(
            tenant.id,
            &products,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    let items = products
        .into_iter()
        .map(|product| {
            let translation = translation_map.get(&product.id).and_then(|items| {
                super::pick_product_translation(items, locale, tenant.default_locale.as_str())
            });
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let public_channel_slug = public_channel_slug_from_request(&request_context);
    let mut product = service
        .get_product_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    if product.status != product::ProductStatus::Active
        || product.published_at.is_none()
        || !is_metadata_visible_for_public_channel(
            &product.metadata,
            public_channel_slug.as_deref(),
        )
    {
        return Err(HttpError::not_found(
            "commerce_store_not_found",
            "Commerce resource not found",
        ));
    }

    apply_public_channel_inventory_to_product(
        runtime.db(),
        tenant.id,
        &mut product,
        public_channel_slug.as_deref(),
    )
    .await
    .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
) -> HttpResult<Json<Vec<RegionResponse>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let service = rustok_region::RegionService::new(runtime.db_clone());
    let regions = service
        .list_regions_for_tenant(
            PortContext::new(
                tenant.id.to_string(),
                PortActor::service("commerce.store-regions"),
                request_context.locale.as_str(),
                format!("store-regions:{}", tenant.id),
            )
            .with_deadline(std::time::Duration::from_secs(3)),
            RegionListRequest {
                requested_locale: Some(request_context.locale.clone()),
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            HttpError::bad_request(
                "commerce_operation_failed",
                format!("{}: {}", error.code, error.message),
            )
        })?;
    Ok(Json(
        regions
            .into_iter()
            .map(|projection| projection.region)
            .collect(),
    ))
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
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Query(query): Query<StoreContextQuery>,
) -> HttpResult<Json<Vec<ShippingOptionResponse>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let (context, public_channel_slug, required_shipping_profiles) = if let Some(cart_id) =
        query.cart_id
    {
        let cart = in_process_cart_storefront_port(runtime.db_clone())
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
            .map_err(|error| HttpError::bad_request("commerce_operation_failed", error.message))?;
        super::ensure_store_cart_access(&cart, customer_id)?;
        let required_shipping_profiles =
            load_cart_shipping_profile_slugs(runtime.db(), tenant.id, &cart)
                .await
                .map_err(|err| {
                    HttpError::bad_request("commerce_operation_failed", err.to_string())
                })?;
        (
            super::resolve_context_from_cart_for_db(
                runtime.db(),
                tenant.id,
                &request_context,
                &cart,
            )
            .await?,
            super::storefront_public_channel_slug_for_cart(&cart, &request_context),
            required_shipping_profiles,
        )
    } else {
        (
            super::resolve_context_for_db(
                runtime.db(),
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

    let service = FulfillmentService::new(runtime.db_clone());
    let mut options = service
        .list_shipping_options(
            tenant.id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    if let Some(currency_code) = context.currency_code.as_deref() {
        options.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
    }
    options.retain(|option| {
        is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug.as_deref())
            && is_shipping_option_compatible_with_profiles(option, &required_shipping_profiles)
    });

    Ok(Json(options))
}
