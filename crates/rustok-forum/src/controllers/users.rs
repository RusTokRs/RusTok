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
        .map_err(|err| HttpError::bad_request("forum_operation_failed", err.to_string()))?;
    Ok(Json(stats))
}

fn ensure_forum_permission(
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
