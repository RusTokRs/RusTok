use axum::{
    extract::{Path, State},
    Json,
};
use rustok_api::Permission;
use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    CategoryService, CategoryTopicPolicyResponse, ForumError, UpdateCategoryTopicPolicyInput,
};

#[utoipa::path(
    get,
    path = "/api/forum/categories/{id}/topic-policy",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    responses(
        (status = 200, description = "Category topic creation policy", body = CategoryTopicPolicyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn get_category_topic_policy(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(category_id): Path<Uuid>,
) -> HttpResult<Json<CategoryTopicPolicyResponse>> {
    ensure_read_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .topic_policy(tenant.id, category_id, forum_security(&auth))
        .await
        .map_err(category_policy_error)?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/forum/categories/{id}/topic-policy",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    request_body = UpdateCategoryTopicPolicyInput,
    responses(
        (status = 200, description = "Category topic creation policy updated", body = CategoryTopicPolicyResponse),
        (status = 400, description = "Invalid policy"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn update_category_topic_policy(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(category_id): Path<Uuid>,
    Json(input): Json<UpdateCategoryTopicPolicyInput>,
) -> HttpResult<Json<CategoryTopicPolicyResponse>> {
    ensure_manage_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .set_topic_policy(tenant.id, category_id, forum_security(&auth), input)
        .await
        .map_err(category_policy_error)?;
    Ok(Json(response))
}

fn category_policy_error(error: ForumError) -> HttpError {
    match error {
        ForumError::Database(error) => HttpError::internal(error.to_string()),
        ForumError::Content(error) => HttpError::internal(error.to_string()),
        ForumError::Internal(error) => HttpError::internal(error.to_string()),
        ForumError::CategoryNotFound(category_id) => HttpError::not_found(
            "forum_category_not_found",
            format!("Category not found: {category_id}"),
        ),
        ForumError::Forbidden(message) => HttpError::forbidden("forum_permission_denied", message),
        error => HttpError::bad_request("forum_category_policy_failed", error.to_string()),
    }
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_read_permission(auth: &AuthContext) -> HttpResult<()> {
    if !has_any_effective_permission(
        &auth.permissions,
        &[Permission::FORUM_CATEGORIES_READ],
    ) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            "Permission denied: forum_categories:read required".to_string(),
        ));
    }
    Ok(())
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
