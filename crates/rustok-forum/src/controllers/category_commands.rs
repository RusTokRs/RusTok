use axum::{
    extract::{Path, State},
    Json,
};
use rustok_api::Permission;
use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    CategoryService, ForumError, MoveCategoryInput, MoveCategoryResponse,
    ReorderCategorySiblingsInput, ReorderCategorySiblingsResponse,
};

#[utoipa::path(
    put,
    path = "/api/forum/categories/{id}/move",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    request_body = MoveCategoryInput,
    responses(
        (status = 200, description = "Category moved and sibling positions normalized", body = MoveCategoryResponse),
        (status = 400, description = "Invalid move or hierarchy"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn move_category(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(category_id): Path<Uuid>,
    Json(input): Json<MoveCategoryInput>,
) -> HttpResult<Json<MoveCategoryResponse>> {
    ensure_manage_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .move_category(tenant.id, category_id, forum_security(&auth), input)
        .await
        .map_err(category_command_error)?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/forum/categories/reorder",
    tag = "forum",
    request_body = ReorderCategorySiblingsInput,
    responses(
        (status = 200, description = "Complete sibling order replaced atomically", body = ReorderCategorySiblingsResponse),
        (status = 400, description = "Invalid or incomplete sibling order"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn reorder_category_siblings(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<ReorderCategorySiblingsInput>,
) -> HttpResult<Json<ReorderCategorySiblingsResponse>> {
    ensure_manage_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .reorder_siblings(tenant.id, forum_security(&auth), input)
        .await
        .map_err(category_command_error)?;
    Ok(Json(response))
}

fn category_command_error(error: ForumError) -> HttpError {
    match error {
        ForumError::Database(error) => HttpError::internal(error.to_string()),
        ForumError::Content(error) => HttpError::internal(error.to_string()),
        ForumError::Internal(error) => HttpError::internal(error.to_string()),
        ForumError::CategoryNotFound(category_id) => HttpError::not_found(
            "forum_category_not_found",
            format!("Category not found: {category_id}"),
        ),
        ForumError::Forbidden(message) => HttpError::forbidden("forum_permission_denied", message),
        error => HttpError::bad_request("forum_category_command_failed", error.to_string()),
    }
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    if !has_any_effective_permission(
        &auth.permissions,
        &[Permission::FORUM_CATEGORIES_MANAGE],
    ) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            "Permission denied: forum_categories:manage required".to_string(),
        ));
    }
    Ok(())
}
