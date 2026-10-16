use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{ForumUserStatsResponse, UserStatsService};

#[utoipa::path(
    get,
    path = "/api/forum/users/{user_id}/stats",
    tag = "forum",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "Forum user statistics", body = ForumUserStatsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_user_stats(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> HttpResult<Json<ForumUserStatsResponse>> {
    tracing::debug!(
        tenant_id = %tenant.id,
        auth_user_id = %auth.user_id,
        target_user_id = %user_id,
        "Forum get user stats requested"
    );
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if user_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_user_id",
            "User ID must not be nil",
        ));
    }
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let stats = UserStatsService::new(runtime.db_clone())
        .get(
            tenant.id,
            rustok_core::security_context_from_access_token(
                auth.user_id,
                &auth.grant_type,
                &auth.permissions,
            ),
            user_id,
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(stats))
}

fn ensure_forum_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if auth.tenant_id.is_nil() {
        return Err(HttpError::unauthorized(
            "invalid_auth_tenant_id",
            "Authenticated tenant ID must not be nil",
        ));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::unauthorized(
            "invalid_auth_user_id",
            "Authenticated user ID must not be nil",
        ));
    }
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            message.to_string(),
        ));
    }

    Ok(())
}
