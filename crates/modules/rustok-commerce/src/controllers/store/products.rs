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
use rustok_region::RegionListRequest;
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::{
    super::common::{PaginatedResponse, PaginationMeta},
    StoreContextQuery, StoreListProductsParams,
};
use crate::controllers::{CommerceHttpRuntime, products::ProductListItem};
use crate::{
    CommerceError,
    dto::{ProductResponse, RegionResponse, ShippingOptionResponse},
    storefront_channel::{
        is_metadata_visible_for_public_channel, public_channel_slug_from_request,
    },
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
        ProductError::Core(error) => crate::controllers::products::product_core_http_policy(
            error,
            "commerce_store_product_failed",
        ),
        ProductError::Conflict(_) => (
            StatusCode::CONFLICT,
            "commerce_store_product_conflict",
            "Product request conflicts with the current state",
            "conflict",
        ),
        ProductError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_product_unavailable",
            "Product service is temporarily unavailable",
            "database",
        ),
        ProductError::ProductNotFound(_)
        | ProductError::VariantNotFound(_)
        | ProductError::ImageNotFound(_) => (
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
        | ProductError::CannotDeleteOnlyVariant => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_product_failed",
            "Product operation could not be completed safely",
            "unexpected_owner_error",
        ),
    };
    tracing::error!(
        owner = "rustok_product",
        operation,
        tenant_non_nil = !tenant_id.is_nil(),
        product_id_non_nil = product_id.is_some_and(|value| !value.is_nil()),
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_product_http",
        "storefront product operation failed with bounded diagnostics"
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
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-product"),
        request_context.locale.as_str(),
        Uuid::new_v4().to_string(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match public_channel_slug {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

// Transport presentation only: Product remains the owner of domain/Core classification.
pub(super) fn storefront_product_port_http_policy(
    kind: &PortErrorKind,
) -> (StatusCode, &'static str, &'static str, &'static str) {
    match kind {
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
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_store_product_conflict",
            "Product request conflicts with the current state",
            "conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::FORBIDDEN,
            "commerce_product_access_denied",
            "Product access is denied",
            "forbidden",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_product_failed",
            "Product operation could not be completed safely",
            "owner_failure",
        ),
    }
}

fn map_storefront_product_port_error(
    error: PortError,
    context: &PortContext,
    operation: &'static str,
    tenant_id: Uuid,
    product_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = storefront_product_port_http_policy(&error.kind);
    tracing::error!(
        owner = "rustok_product",
        owner_operation = operation,
        correlation_id_length = context.correlation_id.chars().count(),
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

fn storefront_port_error_kind(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

fn map_storefront_auxiliary_port_error(
    error: PortError,
    owner: &'static str,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
) -> HttpError {
    let error_kind = storefront_port_error_kind(&error.kind);
    let owner_code_length = error.code.chars().count();
    let retryable = error.retryable;
    let public = port_error_to_http_error(error);
    tracing::error!(
        owner,
        operation,
        tenant_non_nil = !tenant_id.is_nil(),
        cart_id_present = cart_id.is_some(),
        owner_error_kind = error_kind,
        owner_code_length,
        retryable,
        public_status = %public.status,
        boundary = "commerce_storefront_auxiliary_http",
        "storefront auxiliary port operation failed with bounded diagnostics"
    );
    public
}

fn storefront_auxiliary_public_error(
    owner: &'static str,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    error_kind: &'static str,
) -> HttpError {
    tracing::error!(
        owner,
        operation,
        tenant_non_nil = !tenant_id.is_nil(),
        cart_id_present = cart_id.is_some(),
        error_kind,
        public_status = %status,
        boundary = "commerce_storefront_auxiliary_http",
        "storefront auxiliary operation failed with bounded diagnostics"
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
            StatusCode::FORBIDDEN,
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
        owner = "rustok_fulfillment",
        owner_operation = "list_shipping_option_projections",
        operation,
        correlation_id = %context.correlation_id,
        tenant_non_nil = !tenant_id.is_nil(),
        cart_id_present = cart_id.is_some(),
        owner_error_kind = error_kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        public_status = %status,
        boundary = "commerce_storefront_auxiliary_http",
        "storefront shipping-option owner read failed with bounded diagnostics"
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
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = "commerce_storefront_product_http",
        operation = "list_products",
        tenant_id = %tenant.id,
        "handling list_products"
    );

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
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Product not found"),
        (status = 403, description = "Product access denied"),
        (status = 409, description = "Product state conflict"),
        (status = 503, description = "Product service unavailable"),
        (status = 500, description = "Product invariant failure")
    )
)]
pub async fn show_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_product_id",
            "Product ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = "commerce_storefront_product_http",
        operation = "show_product",
        tenant_id_non_nil = !tenant.id.is_nil(),
        product_id_non_nil = !id.is_nil(),
        "handling show_product"
    );

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let public_channel_slug = public_channel_slug_from_request(&request_context);
    let port_context = storefront_product_port_context(
        tenant.id,
        &request_context,
        public_channel_slug.as_deref(),
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
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = "commerce_storefront_auxiliary_http",
        operation = "list_regions",
        tenant_id = %tenant.id,
        "handling list_regions"
    );

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let regions = rustok_region::in_process_region_read_port(runtime.db_clone())
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
        (status = 200, description = "Shipping options", body = Vec<ShippingOptionResponse>),
        (status = 401, description = "Authentication required for customer-owned carts"),
        (status = 403, description = "Cart access denied: guest capability or customer ownership")
    )
)]
pub async fn list_shipping_options(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Query(query): Query<StoreContextQuery>,
) -> HttpResult<Json<Vec<ShippingOptionResponse>>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = "commerce_storefront_auxiliary_http",
        operation = "list_shipping_options",
        tenant_id = %tenant.id,
        "handling list_shipping_options"
    );

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
            super::ensure_store_cart_access(&cart, customer_id, auth.0.as_ref())?;
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

#[cfg(test)]
mod product_port_policy_tests {
    use super::*;

    #[test]
    fn all_product_port_kinds_have_explicit_safe_http_outcomes() {
        for (kind, expected, code) in [
            (
                PortErrorKind::Validation,
                StatusCode::BAD_REQUEST,
                "commerce_store_product_invalid",
            ),
            (
                PortErrorKind::NotFound,
                StatusCode::NOT_FOUND,
                "commerce_store_not_found",
            ),
            (
                PortErrorKind::Forbidden,
                StatusCode::FORBIDDEN,
                "commerce_product_access_denied",
            ),
            (
                PortErrorKind::Conflict,
                StatusCode::CONFLICT,
                "commerce_store_product_conflict",
            ),
            (
                PortErrorKind::Unavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "commerce_store_product_unavailable",
            ),
            (
                PortErrorKind::Timeout,
                StatusCode::SERVICE_UNAVAILABLE,
                "commerce_store_product_unavailable",
            ),
            (
                PortErrorKind::InvariantViolation,
                StatusCode::INTERNAL_SERVER_ERROR,
                "commerce_store_product_failed",
            ),
        ] {
            let policy = storefront_product_port_http_policy(&kind);
            assert_eq!(policy.0, expected);
            assert_eq!(policy.1, code);
            // Poison the entire port envelope, including a forged code, not just Validation copy.
            let error = PortError {
                kind,
                code: "private-code".into(),
                message: "private-message".into(),
                retryable: true,
            };
            let context = PortContext::new(
                "private-tenant",
                PortActor::system(),
                "en",
                "private-correlation",
            );
            let mapped = map_storefront_product_port_error(
                error,
                &context,
                "scenario",
                Uuid::nil(),
                Uuid::nil(),
            );
            assert_eq!(mapped.status, expected);
            assert_eq!(mapped.code, code);
            assert_eq!(mapped.message, policy.2);
            assert!(!mapped.message.contains("private-"));
        }
    }

    #[test]
    fn detail_owner_context_uses_opaque_correlation_and_preserves_scope() {
        let tenant_id = Uuid::new_v4();
        let request = RequestContext {
            tenant_id,
            user_id: None,
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: "ru".into(),
            correlation_id: "private-header".into(),
        };
        let first = storefront_product_port_context(tenant_id, &request, Some("web"));
        let second = storefront_product_port_context(tenant_id, &request, Some("web"));
        assert!(Uuid::parse_str(&first.correlation_id).is_ok());
        assert_ne!(first.correlation_id, second.correlation_id);
        assert_ne!(first.correlation_id, request.correlation_id);
        assert_eq!(first.tenant_id, tenant_id.to_string());
        assert_eq!(first.locale, "ru");
        assert_eq!(first.channel.as_deref(), Some("web"));
        assert_eq!(first.deadline_ms, Some(2000));
    }
}
