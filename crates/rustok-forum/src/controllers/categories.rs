use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::Permission;
use rustok_api::{has_any_effective_permission, AuthContext, RequestContext, TenantContext};
use rustok_telemetry::metrics;
use serde::Deserialize;
use std::time::Instant;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    CategoryListItem, CategoryResponse, CategoryService, CreateCategoryInput, SubscriptionService,
    UpdateCategoryInput,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct CategoryListParams {
    pub locale: Option<String>,
    #[serde(flatten)]
    pub pagination: Option<crate::controllers::topics::PaginationParams>,
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
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<CategoryListParams>,
) -> Result<Json<Vec<CategoryListItem>>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_LIST],
        "Permission denied: forum_categories:list required",
    )?;

    let locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();
    let service = CategoryService::new(ctx.db.clone());
    let list_started_at = Instant::now();
    let (categories, _) = service
        .list_paginated_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            &locale,
            pagination.page,
            pagination.limit(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<CategoryListParams>,
) -> Result<Json<CategoryResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_LIST],
        "Permission denied: forum_categories:list required",
    )?;

    let locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let service = CategoryService::new(ctx.db.clone());
    let category = service
        .get_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            &locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    auth: AuthContext,
    Json(input): Json<CreateCategoryInput>,
) -> Result<(StatusCode, Json<CategoryResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_CREATE],
        "Permission denied: forum_categories:create required",
    )?;

    let service = CategoryService::new(ctx.db.clone());
    let category = service
        .create(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            input,
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok((StatusCode::CREATED, Json(category)))
}

#[utoipa::path(
    put,
    path = "/api/forum/categories/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
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
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCategoryInput>,
) -> Result<Json<CategoryResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_UPDATE],
        "Permission denied: forum_categories:update required",
    )?;

    let service = CategoryService::new(ctx.db.clone());
    let category = service
        .update(
            tenant.id,
            id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            input,
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(category))
}

#[utoipa::path(
    delete,
    path = "/api/forum/categories/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
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
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_DELETE],
        "Permission denied: forum_categories:delete required",
    )?;

    let service = CategoryService::new(ctx.db.clone());
    service
        .delete(
            tenant.id,
            id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/forum/categories/{id}/subscription",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    responses(
        (status = 200, description = "Category subscription updated", body = CategoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn subscribe_category(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<CategoryResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_READ],
        "Permission denied: forum_categories:read required",
    )?;

    SubscriptionService::new(ctx.db.clone())
        .set_category_subscription(
            tenant.id,
            id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let category = CategoryService::new(ctx.db.clone())
        .get_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(category))
}

#[utoipa::path(
    delete,
    path = "/api/forum/categories/{id}/subscription",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    responses(
        (status = 200, description = "Category subscription cleared", body = CategoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn unsubscribe_category(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<CategoryResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_READ],
        "Permission denied: forum_categories:read required",
    )?;

    SubscriptionService::new(ctx.db.clone())
        .clear_category_subscription(
            tenant.id,
            id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let category = CategoryService::new(ctx.db.clone())
        .get_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(category))
}

fn ensure_forum_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }

    Ok(())
}
