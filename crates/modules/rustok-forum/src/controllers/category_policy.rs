use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %auth.user_id,
        category_id = %category_id,
        "Forum category topic policy requested"
    );
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request("invalid_tenant_id", "Tenant ID must not be nil"));
    }
    if category_id.is_nil() {
        return Err(HttpError::bad_request("invalid_category_id", "Category ID must not be nil"));
    }
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %auth.user_id,
        category_id = %category_id,
        topic_creation = ?input.topic_creation,
        "Forum category topic policy update requested"
    );
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request("invalid_tenant_id", "Tenant ID must not be nil"));
    }
    if category_id.is_nil() {
        return Err(HttpError::bad_request("invalid_category_id", "Category ID must not be nil"));
    }
    ensure_manage_permission(&auth)?;
    let response = CategoryService::new(runtime.db_clone())
        .set_topic_policy(tenant.id, category_id, forum_security(&auth), input)
        .await
        .map_err(category_policy_error)?;
    Ok(Json(response))
}

fn category_policy_error(error: ForumError) -> HttpError {
    crate::controllers::map_forum_error(error)
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_read_permission(auth: &AuthContext) -> HttpResult<()> {
    if auth.tenant_id.is_nil() {
        return Err(HttpError::unauthorized("invalid_auth_tenant_id", "Authenticated tenant ID must not be nil"));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::unauthorized("invalid_auth_user_id", "Authenticated user ID must not be nil"));
    }
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_READ]) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            "Permission denied: forum_categories:read required".to_string(),
        ));
    }
    Ok(())
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    if auth.tenant_id.is_nil() {
        return Err(HttpError::unauthorized("invalid_auth_tenant_id", "Authenticated tenant ID must not be nil"));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::unauthorized("invalid_auth_user_id", "Authenticated user ID must not be nil"));
    }
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_MANAGE]) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            "Permission denied: forum_categories:manage required".to_string(),
        ));
    }
    Ok(())
}
