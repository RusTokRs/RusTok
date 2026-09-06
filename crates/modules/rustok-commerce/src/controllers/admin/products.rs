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
        map_admin_product_port_error,
    },
};
use crate::{
    CommerceError, ShippingProfileService,
    dto::{CreateProductInput, ProductResponse, UpdateProductInput},
    storefront_shipping::normalize_shipping_profile_slug,
};

const ADMIN_PRODUCT_SHIPPING_PROFILE_OWNER: &str = "rustok_commerce.shipping_profile";
const ADMIN_PRODUCT_SHIPPING_PROFILE_BOUNDARY: &str =
    "commerce_admin_product_shipping_profile_http";

type AdminProductShippingProfileHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct AdminProductShippingProfileErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    product_id: Option<Uuid>,
    shipping_profile_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminProductShippingProfileErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            product_id,
            shipping_profile_id: None,
            operation,
        }
    }
}

struct AdminProductShippingProfileDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    product_id: &'static str,
    shipping_profile_id: &'static str,
    operation: &'static str,
}

impl From<&AdminProductShippingProfileErrorContext>
    for AdminProductShippingProfileDiagnosticContext
{
    fn from(context: &AdminProductShippingProfileErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            product_id: optional_uuid_shape(context.product_id),
            shipping_profile_id: optional_uuid_shape(context.shipping_profile_id),
            operation: context.operation,
        }
    }
}

struct AdminProductShippingProfileDiagnosticError;

impl std::fmt::Debug for AdminProductShippingProfileDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "present_nil",
        Some(_) => "present_non_nil",
    }
}

fn admin_product_shipping_profile_error_policy(
    error: &CommerceError,
) -> AdminProductShippingProfileHttpPolicy {
    match error {
        CommerceError::ShippingProfileNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        CommerceError::DuplicateShippingProfileSlug(_) => (
            StatusCode::CONFLICT,
            "commerce_admin_shipping_profile_conflict",
            "A shipping profile with this slug already exists",
            "duplicate_slug",
        ),
        CommerceError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_shipping_profile_invalid",
            "Shipping profile request is invalid",
            "validation",
        ),
        CommerceError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_shipping_profile_storage_unavailable",
            "Shipping profile storage is temporarily unavailable",
            "database",
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::InvalidPrice(_)
        | CommerceError::InsufficientInventory { .. }
        | CommerceError::InvalidOptionCombination
        | CommerceError::NoVariants
        | CommerceError::CannotDeletePublished
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_shipping_profile_failed",
            "Shipping profile operation could not be completed safely",
            "unexpected_commerce_error",
        ),
    }
}

fn adopt_admin_product_shipping_profile_error_identity(
    context: &mut AdminProductShippingProfileErrorContext,
    error: &CommerceError,
) {
    if let CommerceError::ShippingProfileNotFound(id) = error {
        context.shipping_profile_id = Some(*id);
    }
}

fn map_admin_product_shipping_profile_error(
    mut context: AdminProductShippingProfileErrorContext,
    error: CommerceError,
) -> HttpError {
    adopt_admin_product_shipping_profile_error_identity(&mut context, &error);
    let (status, code, message, error_kind) = admin_product_shipping_profile_error_policy(&error);
    let context = AdminProductShippingProfileDiagnosticContext::from(&context);
    let error = AdminProductShippingProfileDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_PRODUCT_SHIPPING_PROFILE_OWNER,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        product_id = %context.product_id,
        shipping_profile_id = %context.shipping_profile_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PRODUCT_SHIPPING_PROFILE_BOUNDARY,
        "commerce admin product shipping-profile validation failed"
    );
    HttpError::new(status, code, message)
}

async fn validate_admin_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    context: AdminProductShippingProfileErrorContext,
    shipping_profile_slug: Option<&str>,
) -> HttpResult<()> {
    let Some(slug) = shipping_profile_slug.and_then(normalize_shipping_profile_slug) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(context.tenant_id, &slug)
        .await
        .map_err(|error| map_admin_product_shipping_profile_error(context, error))?;

    Ok(())
}

/// List admin ecommerce products
#[utoipa::path(
    get,
    path = "/admin/products",
    tag = "admin",
    params(ListProductsParams),
    responses(
        (status = 200, description = "List of products", body = PaginatedResponse<ProductListItem>),
        (status = 401, description = "Unauthorized")
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
        format!(
            "commerce-admin-product:list:{}:{}",
            pagination.page,
            pagination.limit()
        ),
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
    request_body = CreateProductInput,
    responses(
        (status = 201, description = "Product created successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Json(input): Json<CreateProductInput>,
) -> HttpResult<(StatusCode, Json<ProductResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_CREATE],
        "Permission denied: products:create required",
    )?;

    validate_admin_product_shipping_profile_input(
        runtime.db(),
        AdminProductShippingProfileErrorContext::new(
            tenant.id,
            auth.user_id,
            None,
            "create_product_shipping_profile_validation",
        ),
        input.shipping_profile_slug.as_deref(),
    )
    .await?;

    let idempotency_key = admin_product_command_idempotency_key(
        tenant.id,
        auth.user_id,
        None,
        "create_product",
        &input,
    )?;
    let port_context =
        admin_product_command_context(tenant.id, &auth, &request_context, idempotency_key);
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
        (status = 401, description = "Unauthorized")
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

    let port_context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-product:show:{id}"),
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
    params(("id" = Uuid, Path, description = "Product ID")),
    request_body = UpdateProductInput,
    responses(
        (status = 200, description = "Product updated successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn update_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProductInput>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    validate_admin_product_shipping_profile_input(
        runtime.db(),
        AdminProductShippingProfileErrorContext::new(
            tenant.id,
            auth.user_id,
            Some(id),
            "update_product_shipping_profile_validation",
        ),
        input.shipping_profile_slug.as_deref(),
    )
    .await?;

    let idempotency_key = admin_product_command_idempotency_key(
        tenant.id,
        auth.user_id,
        Some(id),
        "update_product",
        &input,
    )?;
    let port_context =
        admin_product_command_context(tenant.id, &auth, &request_context, idempotency_key);
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
        (status = 401, description = "Unauthorized")
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
        (status = 401, description = "Unauthorized")
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
        (status = 401, description = "Unauthorized")
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
