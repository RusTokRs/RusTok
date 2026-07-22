use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    ForumQuoteCommandService, ForumRelationSnapshotResponse, SetForumQuotesInput,
};

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_permission(
    auth: &AuthContext,
    permission: Permission,
    message: &'static str,
) -> HttpResult<()> {
    if has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(())
    } else {
        Err(HttpError::forbidden("forum_permission_denied", message))
    }
}

#[utoipa::path(
    put,
    path = "/api/forum/topics/{id}/quotes",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = SetForumQuotesInput,
    responses(
        (status = 200, description = "Topic quote relations replaced", body = ForumRelationSnapshotResponse),
        (status = 400, description = "Invalid quote relation"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn set_topic_quotes(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<SetForumQuotesInput>,
) -> HttpResult<Json<ForumRelationSnapshotResponse>> {
    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_UPDATE,
        "Permission denied: forum_topics:update required",
    )?;
    let response = ForumQuoteCommandService::new(runtime.db_clone())
        .set_topic_quotes(tenant.id, topic_id, forum_security(&auth), input)
        .await
        .map_err(|error| HttpError::bad_request(error.stable_code(), error.to_string()))?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/forum/replies/{id}/quotes",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Reply ID")),
    request_body = SetForumQuotesInput,
    responses(
        (status = 200, description = "Reply quote relations replaced", body = ForumRelationSnapshotResponse),
        (status = 400, description = "Invalid quote relation"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn set_reply_quotes(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(reply_id): Path<Uuid>,
    Json(input): Json<SetForumQuotesInput>,
) -> HttpResult<Json<ForumRelationSnapshotResponse>> {
    ensure_permission(
        &auth,
        Permission::FORUM_REPLIES_UPDATE,
        "Permission denied: forum_replies:update required",
    )?;
    let response = ForumQuoteCommandService::new(runtime.db_clone())
        .set_reply_quotes(tenant.id, reply_id, forum_security(&auth), input)
        .await
        .map_err(|error| HttpError::bad_request(error.stable_code(), error.to_string()))?;
    Ok(Json(response))
}
