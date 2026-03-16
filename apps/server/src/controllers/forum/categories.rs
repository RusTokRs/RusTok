use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use rustok_forum::{
    CategoryListItem, CategoryResponse, CategoryService, CreateCategoryInput, UpdateCategoryInput,
};
use rustok_telemetry::metrics;
use serde::Deserialize;
use std::time::Instant;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::common::PaginationParams;
use crate::extractors::rbac::{
    RequireForumCategoriesCreate, RequireForumCategoriesDelete, RequireForumCategoriesList,
    RequireForumCategoriesUpdate,
};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::{common::RequestContext, context::TenantContext};

#[derive(Debug, Deserialize, IntoParams)]
pub struct CategoryListParams {
    pub locale: Option<String>,
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
}

#[utoipa::path(
    get,
    path = "/api/forum/categories",
    tag = "forum",
    params(CategoryListParams),
    responses(
        (status = 200, description = "List of categories", body = Vec<CategoryListItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_categories(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumCategoriesList(user): RequireForumCategoriesList,
    request_context: RequestContext,
    Query(params): Query<CategoryListParams>,
) -> Result<Json<Vec<CategoryListItem>>> {
    let locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();
    let service = CategoryService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let (categories, _) = service
        .list_paginated_with_locale_fallback(
            tenant.id,
            user.security_context(),
            &locale,
            pagination.page,
            pagination.limit(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    metrics::record_read_path_query(
        "http",
        "forum.list_categories",
        "service_list",
        list_started_at.elapsed().as_secs_f64(),
        categories.len() as u64,
    );

    metrics::record_read_path_budget(
        "http",
        "forum.list_categories",
        requested_limit,
        pagination.limit(),
        categories.len(),
    );

    Ok(Json(categories))
}

#[utoipa::path(
    get,
    path = "/api/forum/categories/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Category ID"),
        ("locale" = Option<String>, Query, description = "Locale")
    ),
    responses(
        (status = 200, description = "Category details", body = CategoryResponse),
        (status = 404, description = "Category not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_category(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumCategoriesList(_user): RequireForumCategoriesList,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<CategoryListParams>,
) -> Result<Json<CategoryResponse>> {
    let locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let service = CategoryService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let category = service
        .get_with_locale_fallback(tenant.id, id, &locale, Some(tenant.default_locale.as_str()))
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(category))
}

#[utoipa::path(
    post,
    path = "/api/forum/categories",
    tag = "forum",
    request_body = CreateCategoryInput,
    responses(
        (status = 201, description = "Category created", body = CategoryResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_category(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumCategoriesCreate(user): RequireForumCategoriesCreate,
    Json(input): Json<CreateCategoryInput>,
) -> Result<(StatusCode, Json<CategoryResponse>)> {
    let service = CategoryService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let category = service
        .create(tenant.id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok((StatusCode::CREATED, Json(category)))
}

#[utoipa::path(
    put,
    path = "/api/forum/categories/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Category ID")
    ),
    request_body = UpdateCategoryInput,
    responses(
        (status = 200, description = "Category updated", body = CategoryResponse),
        (status = 404, description = "Category not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_category(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumCategoriesUpdate(user): RequireForumCategoriesUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCategoryInput>,
) -> Result<Json<CategoryResponse>> {
    let service = CategoryService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let category = service
        .update(tenant.id, id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(category))
}

#[utoipa::path(
    delete,
    path = "/api/forum/categories/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Category ID")
    ),
    responses(
        (status = 204, description = "Category deleted"),
        (status = 404, description = "Category not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_category(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumCategoriesDelete(user): RequireForumCategoriesDelete,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let service = CategoryService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
