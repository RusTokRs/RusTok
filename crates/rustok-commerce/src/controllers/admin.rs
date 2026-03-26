use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, AuthContext, TenantContext};
use rustok_core::Permission;
use uuid::Uuid;

use crate::{
    dto::{CreateProductInput, ProductResponse, UpdateProductInput},
    CatalogService,
};

use super::{
    common::{ensure_permissions, PaginatedResponse},
    products::{ListProductsParams, ProductListItem},
};

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/products",
            axum::routing::get(list_products).post(create_product),
        )
        .add(
            "/products/{id}",
            axum::routing::get(show_product)
                .post(update_product)
                .delete(delete_product),
        )
        .add(
            "/products/{id}/publish",
            axum::routing::post(publish_product),
        )
        .add(
            "/products/{id}/unpublish",
            axum::routing::post(unpublish_product),
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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_products(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    query: Query<ListProductsParams>,
) -> Result<Json<PaginatedResponse<ProductListItem>>> {
    super::products::list_products(state, tenant, auth, request_context, query).await
}

/// Create admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products",
    tag = "admin",
    request_body = CreateProductInput,
    responses(
        (status = 201, description = "Product created successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateProductInput>,
) -> Result<(StatusCode, Json<ProductResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_CREATE],
        "Permission denied: products:create required",
    )?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .create_product(tenant.id, auth.user_id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn show_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::show_product(state, tenant, auth, path).await
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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProductInput>,
) -> Result<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .update_product(tenant.id, auth.user_id, id, input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

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
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<StatusCode> {
    super::products::delete_product(state, tenant, auth, path).await
}

/// Publish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/publish",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product published successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn publish_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::publish_product(state, tenant, auth, path).await
}

/// Unpublish admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/unpublish",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product unpublished successfully", body = ProductResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn unpublish_product(
    state: State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    path: Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    super::products::unpublish_product(state, tenant, auth, path).await
}
