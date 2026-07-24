use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_product::CatalogService;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::super::{
    CommerceHttpRuntime,
    common::{PaginatedResponse, ensure_permissions},
    products::{ListProductsParams, ProductListItem},
};
use crate::{
    CommerceError,
    dto::{CreateProductInput, ProductResponse, UpdateProductInput},
};

fn map_product_write_error(error: CommerceError) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        CommerceError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_product_storage_unavailable",
            "Product storage is temporarily unavailable",
            "database",
        ),
        CommerceError::ProductNotFound(_) | CommerceError::VariantNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        CommerceError::DuplicateHandle { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_product_handle_conflict",
            "A product with this handle already exists",
            "duplicate_handle",
        ),
        CommerceError::DuplicateSku(_) => (
            StatusCode::CONFLICT,
            "commerce_admin_product_sku_conflict",
            "A product variant with this SKU already exists",
            "duplicate_sku",
        ),
        CommerceError::InvalidPrice(_)
        | CommerceError::InvalidOptionCombination
        | CommerceError::Validation(_)
        | CommerceError::NoVariants => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        CommerceError::InsufficientInventory { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_product_inventory_conflict",
            "Product inventory conflicts with the requested operation",
            "inventory_conflict",
        ),
        CommerceError::CannotDeletePublished => (
            StatusCode::CONFLICT,
            "commerce_admin_product_state_conflict",
            "Product operation conflicts with the current state",
            "state_conflict",
        ),
        CommerceError::ShippingProfileNotFound(_)
        | CommerceError::DuplicateShippingProfileSlug(_)
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_product_failed",
            "Product operation could not be completed safely",
            "unexpected_owner_error",
        ),
    };
    super::admin_public_error(
        &error,
        "rustok_product.catalog",
        error_kind,
        status,
        code,
        message,
    )
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
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    query: Query<ListProductsParams>,
) -> HttpResult<Json<PaginatedResponse<ProductListItem>>> {
    super::super::products::list_products(state, tenant, auth, request_context, query).await
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
    Json(input): Json<CreateProductInput>,
) -> HttpResult<(StatusCode, Json<ProductResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_CREATE],
        "Permission denied: products:create required",
    )?;

    super::validate_product_shipping_profile_input(
        runtime.db(),
        tenant.id,
        input.shipping_profile_slug.as_deref(),
    )
    .await?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product = service
        .create_product(tenant.id, auth.user_id, input)
        .await
        .map_err(map_product_write_error)?;

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
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::show_product(state, tenant, auth, request_context, path).await
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
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProductInput>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    super::validate_product_shipping_profile_input(
        runtime.db(),
        tenant.id,
        input.shipping_profile_slug.as_deref(),
    )
    .await?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product = service
        .update_product(tenant.id, auth.user_id, id, input)
        .await
        .map_err(map_product_write_error)?;

    Ok(Json(product))
}

/// Delete admin ecommerce product
#[utoipa::path(
    delete,
    path = "/admin/products/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 204, description = "Product deleted successfully"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn delete_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> HttpResult<StatusCode> {
    super::super::products::delete_product(state, tenant, auth, path).await
}

/// Publish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/publish",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product published successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn publish_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::publish_product(state, tenant, auth, path).await
}

/// Unpublish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/unpublish",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product unpublished successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn unpublish_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::unpublish_product(state, tenant, auth, path).await
}
