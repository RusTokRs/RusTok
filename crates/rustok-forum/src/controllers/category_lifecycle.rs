use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{CategoryService, CategorySubtreeLifecycleResponse, ForumError};

#[utoipa::path(
    post,
    path = "/api/forum/categories/{id}/archive-subtree",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category subtree root ID")),
    responses(
        (status = 200, description = "Category subtree archived atomically", body = CategorySubtreeLifecycleResponse),
        (status = 400, description = "Invalid category hierarchy"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn archive_category_subtree(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(category_id): Path<Uuid>,
) -> HttpResult<Json<CategorySubtreeLifecycleResponse>> {
    ensure_manage_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .archive_subtree(tenant.id, category_id, forum_security(&auth))
        .await
        .map_err(category_lifecycle_error)?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/forum/categories/{id}/restore-subtree",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category subtree root ID")),
    responses(
        (status = 200, description = "Category subtree restored atomically", body = CategorySubtreeLifecycleResponse),
        (status = 400, description = "Invalid category hierarchy"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn restore_category_subtree(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(category_id): Path<Uuid>,
) -> HttpResult<Json<CategorySubtreeLifecycleResponse>> {
    ensure_manage_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .restore_subtree(tenant.id, category_id, forum_security(&auth))
        .await
        .map_err(category_lifecycle_error)?;
    Ok(Json(response))
}

fn category_lifecycle_error(error: ForumError) -> HttpError {
    crate::controllers::map_forum_error(error)
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_MANAGE]) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            "Permission denied: forum_categories:manage required".to_string(),
        ));
    }
    Ok(())
}
