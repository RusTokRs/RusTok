use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::super::{
    CommerceHttpRuntime,
    common::{PaginatedResponse, PaginationMeta, ensure_permissions},
    products::{
        AdminProductErrorContext, ListProductsParams, ProductListItem,
        admin_product_command_context, admin_product_command_idempotency_key,
        ensure_admin_product_scope, map_admin_product_port_error,
    },
};
use crate::{
    dto::{CreateProductInput, ProductResponse, UpdateProductInput},
    storefront_shipping::normalize_shipping_profile_slug,
};

/// List admin ecommerce products
#[utoipa::path(
    get,
    path = "/admin/products",
    tag = "admin",
    params(ListProductsParams),
    responses(
        (status = 200, description = "List of products", body = PaginatedResponse<ProductListItem>),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn list_products(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListProductsParams>,
) -> HttpResult<Json<PaginatedResponse<ProductListItem>>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_LIST],
        "Permission denied: products:list required",
    )?;
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = "commerce_admin_product_http",
        operation = "list_products",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        "handling list_products"
    );

    let requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();
    let locale = params
        .locale
        .as_deref()
        .unwrap_or(request_context.locale.as_str())
        .to_string();
    let list_query = rustok_product::AdminProductListQuery {
        search: params.search,
        status: None,
        category_id: None,
        sort_by: rustok_product::StorefrontProductSortBy::CreatedAt,
        sort_direction: rustok_product::StorefrontProductSortDirection::Desc,
        attribute_filters: Vec::new(),
    };
    let port_context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        locale.as_str(),
        request_context.correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    let port_context = match request_context.channel_slug.as_deref() {
        Some(channel) => port_context.with_channel(channel),
        None => port_context,
    };
    let products = runtime
        .product_catalog_read_port()
        .list_admin_products(
            port_context.clone(),
            rustok_product::AdminProductsRequest {
                locale: Some(locale),
                fallback_locale: Some(tenant.default_locale.clone()),
                query: list_query,
                raw_status: params.status,
                vendor: params.vendor,
                product_type: params.product_type,
                empty_missing_title: true,
                page: pagination.page,
                per_page: pagination.limit(),
            },
        )
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, None, "list_products"),
                &port_context,
                error,
            )
        })?;

    let items = products
        .items
        .into_iter()
        .map(|product| ProductListItem {
            id: product.id,
            status: product.status.to_string(),
            title: product.title,
            handle: product.handle,
            seller_id: product.seller_id,
            vendor: product.vendor,
            product_type: product.product_type,
            shipping_profile_slug: Some(
                product
                    .shipping_profile_slug
                    .as_deref()
                    .and_then(normalize_shipping_profile_slug)
                    .unwrap_or_else(|| "default".to_string()),
            ),
            tags: product.tags,
            created_at: product.created_at.to_rfc3339(),
            published_at: product.published_at.map(|value| value.to_rfc3339()),
        })
        .collect::<Vec<_>>();

    rustok_telemetry::metrics::record_read_path_budget(
        "http",
        "commerce.list_products",
        requested_limit,
        products.per_page,
        items.len(),
    );

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(products.page, products.per_page, products.total),
    }))
}

/// Create admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products",
    tag = "admin",
    params(("Idempotency-Key" = String, Header, description = "Caller-owned idempotency key")),
    request_body = CreateProductInput,
    responses(
        (status = 201, description = "Product created successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn create_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Json(input): Json<CreateProductInput>,
) -> HttpResult<(StatusCode, Json<ProductResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_CREATE],
        "Permission denied: products:create required",
    )?;
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = "commerce_admin_product_http",
        operation = "create_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        "handling create_product"
    );

    let idempotency_key = admin_product_command_idempotency_key(&headers)?;
    let port_context = admin_product_command_context(
        tenant.id,
        &auth,
        &request_context,
        None,
        "create_product",
        idempotency_key,
    );
    let product = runtime
        .product_catalog_command_port()
        .create_product(port_context.clone(), input)
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, None, "create_product"),
                &port_context,
                error,
            )
        })?;

    Ok((StatusCode::CREATED, Json(product)))
}

/// Show admin ecommerce product
#[utoipa::path(
    get,
    path = "/admin/products/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product details", body = ProductResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn show_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
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
        boundary = "commerce_admin_product_http",
        operation = "show_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling show_product"
    );

    let port_context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        request_context.correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    let port_context = match request_context.channel_slug.as_deref() {
        Some(channel) => port_context.with_channel(channel),
        None => port_context,
    };
    let product = runtime
        .product_catalog_read_port()
        .read_product_projection(
            port_context.clone(),
            rustok_product::ProductProjectionRequest {
                product_id: id,
                locale: Some(request_context.locale.clone()),
                fallback_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, Some(id), "show_product"),
                &port_context,
                error,
            )
        })?;

    Ok(Json(product))
}

/// Update admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}",
    tag = "admin",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
        ("Idempotency-Key" = String, Header, description = "Caller-owned idempotency key")
    ),
    request_body = UpdateProductInput,
    responses(
        (status = 200, description = "Product updated successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn update_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProductInput>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
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
        boundary = "commerce_admin_product_http",
        operation = "update_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling update_product"
    );

    let idempotency_key = admin_product_command_idempotency_key(&headers)?;
    let port_context = admin_product_command_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "update_product",
        idempotency_key,
    );
    let product = runtime
        .product_catalog_command_port()
        .update_product(port_context.clone(), id, input)
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, Some(id), "update_product"),
                &port_context,
                error,
            )
        })?;

    Ok(Json(product))
}

/// Delete admin ecommerce product
#[utoipa::path(
    delete,
    path = "/admin/products/{id}",
    tag = "admin",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
        ("Idempotency-Key" = String, Header, description = "Stable identity for this logical lifecycle command, maximum 191 bytes")
    ),
    responses(
        (status = 204, description = "Product deleted successfully"),
        (status = 400, description = "Missing or invalid idempotency key"),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    path: Path<Uuid>,
) -> HttpResult<StatusCode> {
    super::super::products::delete_product(state, tenant, auth, request_context, headers, path)
        .await
}

/// Publish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/publish",
    tag = "admin",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
        ("Idempotency-Key" = String, Header, description = "Stable identity for this logical lifecycle command, maximum 191 bytes")
    ),
    responses(
        (status = 200, description = "Product published successfully", body = ProductResponse),
        (status = 400, description = "Missing or invalid idempotency key"),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn publish_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::publish_product(state, tenant, auth, request_context, headers, path)
        .await
}

/// Unpublish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/unpublish",
    tag = "admin",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
        ("Idempotency-Key" = String, Header, description = "Stable identity for this logical lifecycle command, maximum 191 bytes")
    ),
    responses(
        (status = 200, description = "Product unpublished successfully", body = ProductResponse),
        (status = 400, description = "Missing or invalid idempotency key"),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn unpublish_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::unpublish_product(state, tenant, auth, request_context, headers, path)
        .await
}

/// Archive admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/archive",
    tag = "admin",
    params(
        ("id" = Uuid, Path, description = "Product ID"),
        ("Idempotency-Key" = String, Header, description = "Stable identity for this logical lifecycle command, maximum 191 bytes")
    ),
    responses(
        (status = 200, description = "Product archived successfully", body = ProductResponse),
        (status = 400, description = "Missing or invalid idempotency key"),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn archive_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::archive_product(state, tenant, auth, request_context, headers, path)
        .await
}
