use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    OptionalAuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_cart::{CartStorefrontReadRequest, in_process_cart_storefront_port};
use rustok_fulfillment::ListShippingOptionProjectionsRequest;
use rustok_product::{
    CatalogService, CommerceError as ProductError, StorefrontProductProjectionRequest,
    StorefrontProductProjectionSubject,
    entities::{product, product_translation},
};
use rustok_region::{RegionListRequest, RegionReadPort};
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::{
    super::common::{PaginatedResponse, PaginationMeta},
    StoreContextQuery, StoreListProductsParams,
};
use crate::controllers::{CommerceHttpRuntime, products::ProductListItem};
use crate::{
    CommerceError,
    dto::{ProductResponse, RegionResponse, ShippingOptionResponse},
    storefront_channel::{is_metadata_visible_for_public_channel, public_channel_slug_from_request},
    storefront_shipping::{
        is_shipping_option_compatible_with_profiles, load_cart_shipping_profile_slugs,
        shipping_profile_slug_from_product_metadata,
    },
};

fn map_storefront_product_error(
    error: ProductError,
    operation: &'static str,
    tenant_id: Uuid,
    product_id: Option<Uuid>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        ProductError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_product_unavailable",
            "Product service is temporarily unavailable",
            "database",
        ),
        ProductError::ProductNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        ProductError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_store_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        ProductError::DuplicateHandle { .. }
        | ProductError::DuplicateSku(_)
        | ProductError::NoVariants
        | ProductError::CannotDeletePublished
        | ProductError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_product_failed",
            "Product operation could not be completed safely",
            "unexpected_owner_error",
        ),
    };
    tracing::error!(
        error = ?error,
        operation,
        tenant_id = %tenant_id,
        product_id = ?product_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_product_http",
        "storefront product operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_storefront_product_database_error(
    error: sea_orm::DbErr,
    operation: &'static str,
    tenant_id: Uuid,
    product_id: Option<Uuid>,
) -> HttpError {
    map_storefront_product_error(
        ProductError::Database(error),
        operation,
        tenant_id,
        product_id,
    )
}

fn storefront_product_port_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    public_channel_slug: Option<&str>,
    product_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-product"),
        request_context.locale.as_str(),
        format!("commerce-storefront-product:read:{product_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match public_channel_slug {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_storefront_product_port_error(
    error: PortError,
    context: &PortContext,
    operation: &'static str,
    tenant_id: Uuid,
    product_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_store_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_product_unavailable",
            "Product service is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::Conflict | PortErrorKind::Forbidden | PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_product_failed",
            "Product operation could not be completed safely",
            "owner_failure",
        ),
    };
    tracing::error!(
        owner = "rustok_product",
        owner_operation = operation,
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        product_id_non_nil = !product_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_product_http",
        "storefront product owner read failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

fn map_storefront_auxiliary_port_error(
    error: PortError,
    owner: &'static str,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
) -> HttpError {
    let public = port_error_to_http_error(error.clone());
    tracing::error!(
        error = ?error,
        owner,
        operation,
        tenant_id = %tenant_id,
        cart_id = ?cart_id,
        error_kind = ?error.kind,
        retryable = error.retryable,
        public_code = %public.code,
        status = %public.status,
        boundary = "commerce_storefront_auxiliary_http",
        "storefront auxiliary port operation failed"
    );
    public
}

fn storefront_auxiliary_public_error<E>(
    error: &E,
    owner: &'static str,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    error_kind: &'static str,
) -> HttpError
where
    E: std::fmt::Debug,
{
    tracing::error!(
        error = ?error,
        owner,
        operation,
        tenant_id = %tenant_id,
        cart_id = ?cart_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_auxiliary_http",
        "storefront auxiliary operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_storefront_shipping_context_error(
    error: CommerceError,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        CommerceError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_shipping_unavailable",
            "Shipping service is temporarily unavailable",
            "database",
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::ShippingProfileNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        CommerceError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_store_shipping_invalid",
            "Shipping request is invalid",
            "validation",
        ),
        CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::InvalidPrice(_)
        | CommerceError::InsufficientInventory { .. }
        | CommerceError::InvalidOptionCombination
        | CommerceError::DuplicateShippingProfileSlug(_)
        | CommerceError::NoVariants
        | CommerceError::CannotDeletePublished
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_shipping_failed",
            "Shipping operation could not be completed safely",
            "unexpected_owner_error",
        ),
    };
    storefront_auxiliary_public_error(
        &error,
        "rustok_commerce.storefront_shipping",
        operation,
        tenant_id,
        cart_id,
        status,
        code,
        message,
        error_kind,
    )
}

fn storefront_shipping_option_port_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&rustok_api::AuthContext>,
    public_channel_slug: Option<&str>,
    requested_cart_id: Option<Uuid>,
) -> PortContext {
    let actor = auth
        .map(|value| PortActor::user(value.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("rustok-commerce.storefront-shipping-options"));
    let resource_id = requested_cart_id.unwrap_or(tenant_id);
    let context = PortContext::new(
        tenant_id.to_string(),
        actor,
        request_context.locale.as_str(),
        format!("commerce-store-shipping-options:list:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match public_channel_slug {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_storefront_shipping_port_error(
    error: PortError,
    context: &PortContext,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_store_shipping_invalid",
            "Shipping request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_store_shipping_state_conflict",
            "Shipping operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_store_denied",
            "Store access is denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_shipping_unavailable",
            "Shipping service is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_shipping_failed",
            "Shipping operation could not be completed safely",
            "invariant_violation",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = "rustok_fulfillment",
        owner_operation = "list_shipping_option_projections",
        operation,
        correlation_id = %context.correlation_id,
        tenant_id = %tenant_id,
        cart_id = ?cart_id,
        actor = ?context.actor,
        channel = ?context.channel,
        locale = %context.locale,
        deadline_ms = ?context.deadline_ms,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_auxiliary_http",
        "storefront shipping-option owner read failed"
    );
    HttpError::new(status, code, message)
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
        .map_err(|error| {
            map_storefront_product_database_error(error, "list_products", tenant.id, None)
        })?
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
            .map_err(|error| {
                map_storefront_product_database_error(
                    error,
                    "list_product_translations",
                    tenant.id,
                    None,
                )
            })?
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
        .map_err(|error| {
            map_storefront_product_error(error, "list_product_tags", tenant.id, None)
        })?;

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

    let public_channel_slug = public_channel_slug_from_request(&request_context);
    let port_context = storefront_product_port_context(
        tenant.id,
        &request_context,
        public_channel_slug.as_deref(),
        id,
    );
    let product = runtime
        .product_catalog_read_port()
        .read_storefront_product_projection(
            port_context.clone(),
            StorefrontProductProjectionRequest {
                subject: StorefrontProductProjectionSubject::ProductId { product_id: id },
                locale: Some(request_context.locale.clone()),
                fallback_locale: Some(tenant.default_locale.clone()),
                public_channel_slug,
            },
        )
        .await
        .map_err(|error| {
            map_storefront_product_port_error(
                error,
                &port_context,
                "read_storefront_product_projection",
                tenant.id,
                id,
            )
        })?
        .ok_or_else(|| {
            HttpError::not_found("commerce_store_not_found", "Commerce resource not found")
        })?;

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
            map_storefront_auxiliary_port_error(
                error,
                "rustok_region",
                "list_regions",
                tenant.id,
                None,
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
    let requested_cart_id = query.cart_id;
    let (context, public_channel_slug, required_shipping_profiles) =
        if let Some(cart_id) = requested_cart_id {
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
                .map_err(|error| {
                    map_storefront_auxiliary_port_error(
                        error,
                        "rustok_cart",
                        "read_shipping_options_cart",
                        tenant.id,
                        Some(cart_id),
                    )
                })?;
            super::ensure_store_cart_access(&cart, customer_id)?;
            let required_shipping_profiles =
                load_cart_shipping_profile_slugs(runtime.db(), tenant.id, &cart)
                    .await
                    .map_err(|error| {
                        map_storefront_shipping_context_error(
                            error,
                            "load_cart_shipping_profiles",
                            tenant.id,
                            Some(cart_id),
                        )
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

    let read_context = storefront_shipping_option_port_context(
        tenant.id,
        &request_context,
        auth.0.as_ref(),
        public_channel_slug.as_deref(),
        requested_cart_id,
    );
    let mut options = runtime
        .shipping_option_read_port()
        .list_shipping_option_projections(
            read_context.clone(),
            ListShippingOptionProjectionsRequest {
                requested_locale: Some(request_context.locale.clone()),
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_storefront_shipping_port_error(
                error,
                &read_context,
                "list_shipping_options",
                tenant.id,
                requested_cart_id,
            )
        })?;

    if let Some(currency_code) = context.currency_code.as_deref() {
        options.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
    }
    options.retain(|option| {
        is_metadata_visible_for_public_channel(&option.metadata, public_channel_slug.as_deref())
            && is_shipping_option_compatible_with_profiles(option, &required_shipping_profiles)
    });

    Ok(Json(options))
}
