use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    ForumSubscriptionPolicyResponse, ForumSubscriptionResponse, SubscriptionService,
    UpdateForumSubscriptionInput, UpdateForumSubscriptionPolicyInput,
};

#[utoipa::path(
    get,
    path = "/api/forum/categories/{id}/subscription",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    responses(
        (status = 200, description = "Category subscription settings", body = ForumSubscriptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_category_subscription_settings(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ForumSubscriptionResponse>> {
    ensure_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_READ],
        "Permission denied: forum_categories:read required",
    )?;
    let settings = SubscriptionService::new(runtime.db_clone())
        .get_category_subscription(tenant.id, id, security(&auth))
        .await
        .map_err(operation_error)?;
    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/forum/categories/{id}/subscription",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Category ID")),
    request_body = UpdateForumSubscriptionInput,
    responses(
        (status = 200, description = "Category subscription settings updated", body = ForumSubscriptionResponse),
        (status = 400, description = "Invalid settings or revision conflict"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_category_subscription_settings(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateForumSubscriptionInput>,
) -> HttpResult<Json<ForumSubscriptionResponse>> {
    ensure_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_READ],
        "Permission denied: forum_categories:read required",
    )?;
    let settings = SubscriptionService::new(runtime.db_clone())
        .update_category_subscription(tenant.id, id, security(&auth), input)
        .await
        .map_err(operation_error)?;
    Ok(Json(settings))
}

#[utoipa::path(
    get,
    path = "/api/forum/topics/{id}/subscription",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 200, description = "Topic subscription settings", body = ForumSubscriptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_topic_subscription_settings(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ForumSubscriptionResponse>> {
    ensure_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;
    let settings = SubscriptionService::new(runtime.db_clone())
        .get_topic_subscription(tenant.id, id, security(&auth))
        .await
        .map_err(operation_error)?;
    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/forum/topics/{id}/subscription",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = UpdateForumSubscriptionInput,
    responses(
        (status = 200, description = "Topic subscription settings updated", body = ForumSubscriptionResponse),
        (status = 400, description = "Invalid settings or revision conflict"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_topic_subscription_settings(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateForumSubscriptionInput>,
) -> HttpResult<Json<ForumSubscriptionResponse>> {
    ensure_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;
    let settings = SubscriptionService::new(runtime.db_clone())
        .update_topic_subscription(tenant.id, id, security(&auth), input)
        .await
        .map_err(operation_error)?;
    Ok(Json(settings))
}

#[utoipa::path(
    get,
    path = "/api/forum/subscription-policy",
    tag = "forum",
    responses(
        (status = 200, description = "Tenant forum subscription policy", body = ForumSubscriptionPolicyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_subscription_policy(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
) -> HttpResult<Json<ForumSubscriptionPolicyResponse>> {
    ensure_permission(
        &auth,
        &[
            Permission::FORUM_CATEGORIES_MANAGE,
            Permission::FORUM_TOPICS_MANAGE,
        ],
        "Permission denied: forum manage permission required",
    )?;
    let policy = SubscriptionService::new(runtime.db_clone())
        .get_policy(tenant.id, security(&auth))
        .await
        .map_err(operation_error)?;
    Ok(Json(policy))
}

#[utoipa::path(
    put,
    path = "/api/forum/subscription-policy",
    tag = "forum",
    request_body = UpdateForumSubscriptionPolicyInput,
    responses(
        (status = 200, description = "Tenant forum subscription policy updated", body = ForumSubscriptionPolicyResponse),
        (status = 400, description = "Invalid policy or revision conflict"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_subscription_policy(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<UpdateForumSubscriptionPolicyInput>,
) -> HttpResult<Json<ForumSubscriptionPolicyResponse>> {
    ensure_permission(
        &auth,
        &[
            Permission::FORUM_CATEGORIES_MANAGE,
            Permission::FORUM_TOPICS_MANAGE,
        ],
        "Permission denied: forum manage permission required",
    )?;
    let policy = SubscriptionService::new(runtime.db_clone())
        .update_policy(tenant.id, security(&auth), input)
        .await
        .map_err(operation_error)?;
    Ok(Json(policy))
}

fn security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            message.to_string(),
        ));
    }
    Ok(())
}

fn operation_error(error: crate::ForumError) -> HttpError {
    crate::controllers::map_forum_error(error)
}
