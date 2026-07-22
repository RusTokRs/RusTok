use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    Action, AuthContext, Permission, RequestContext, Resource, TenantContext,
    has_effective_permission,
};
use rustok_web::{HttpError, HttpResult};
use std::collections::HashMap;
use uuid::Uuid;

use super::BlogHttpRuntime;
use crate::dto::{
    CategoryListResponse, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    UpdateCategoryInput,
};
use crate::{BlogError, CategoryService};

fn security_context(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_category_permission(auth: &AuthContext, action: Action) -> HttpResult<()> {
    let permission = Permission::new(Resource::BlogCategories, action);
    if !has_effective_permission(&auth.permissions, &permission) {
        return Err(HttpError::forbidden(
            "blog_category_permission_denied",
            format!("Permission denied: {permission} required"),
        ));
    }
    Ok(())
}

fn category_service(runtime: &BlogHttpRuntime) -> CategoryService {
    CategoryService::new(runtime.db_clone(), runtime.event_bus())
}

fn map_category_error(error: BlogError) -> HttpError {
    match error {
        BlogError::CategoryNotFound(category_id) => HttpError::not_found(
            "blog_category_not_found",
            format!("Blog category {category_id} not found"),
        ),
        BlogError::Forbidden(message) => HttpError::forbidden("blog_category_forbidden", message),
        BlogError::Validation(message) => {
            HttpError::bad_request("blog_category_validation_failed", message)
        }
        _ => HttpError::internal("Unable to complete the Blog category operation"),
    }
}

#[utoipa::path(
    get,
    path = "/api/blog/categories",
    tag = "blog",
    params(ListCategoriesFilter),
    responses(
        (status = 200, description = "List of Blog categories", body = CategoryListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_categories(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(mut filter): Query<ListCategoriesFilter>,
) -> HttpResult<Json<CategoryListResponse>> {
    ensure_category_permission(&auth, Action::List)?;
    filter.locale = filter.locale.or(Some(request_context.locale));
    filter.page = filter.page.max(1);
    filter.per_page = filter.per_page.clamp(1, 100);
    let page = filter.page;
    let per_page = filter.per_page;

    let (items, total) = category_service(&runtime)
        .list(tenant.id, security_context(&auth), filter)
        .await
        .map_err(map_category_error)?;

    Ok(Json(CategoryListResponse {
        items,
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/blog/categories/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Category ID"),
        ("locale" = Option<String>, Query, description = "Requested locale")
    ),
    responses(
        (status = 200, description = "Blog category", body = CategoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn get_category(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> HttpResult<Json<CategoryResponse>> {
    ensure_category_permission(&auth, Action::Read)?;
    let locale = params
        .get("locale")
        .map(String::as_str)
        .unwrap_or(request_context.locale.as_str());

    let category = category_service(&runtime)
        .get(tenant.id, security_context(&auth), id, locale)
        .await
        .map_err(map_category_error)?;

    Ok(Json(category))
}

#[utoipa::path(
    post,
    path = "/api/blog/categories",
    tag = "blog",
    request_body = CreateCategoryInput,
    responses(
        (status = 201, description = "Blog category created", body = Uuid),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Parent category not found")
    )
)]
pub async fn create_category(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateCategoryInput>,
) -> HttpResult<(StatusCode, Json<Uuid>)> {
    ensure_category_permission(&auth, Action::Create)?;

    let category_id = category_service(&runtime)
        .create(tenant.id, security_context(&auth), input)
        .await
        .map_err(map_category_error)?;

    Ok((StatusCode::CREATED, Json(category_id)))
}

#[utoipa::path(
    put,
    path = "/api/blog/categories/{id}",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Category ID")),
    request_body = UpdateCategoryInput,
    responses(
        (status = 200, description = "Blog category updated", body = CategoryResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn update_category(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCategoryInput>,
) -> HttpResult<Json<CategoryResponse>> {
    ensure_category_permission(&auth, Action::Update)?;

    let category = category_service(&runtime)
        .update(tenant.id, id, security_context(&auth), input)
        .await
        .map_err(map_category_error)?;

    Ok(Json(category))
}

#[utoipa::path(
    delete,
    path = "/api/blog/categories/{id}",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Category ID")),
    responses(
        (status = 204, description = "Blog category deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn delete_category(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    ensure_category_permission(&auth, Action::Delete)?;

    category_service(&runtime)
        .delete(tenant.id, id, security_context(&auth))
        .await
        .map_err(map_category_error)?;

    Ok(StatusCode::NO_CONTENT)
}
