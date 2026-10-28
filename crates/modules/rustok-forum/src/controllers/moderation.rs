use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::moderation_transport::{ForumModerationTransport, moderation_audience_port_context};

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

#[utoipa::path(
    post,
    path = "/api/forum/topics/{topic_id}/solution/{reply_id}",
    tag = "forum",
    params(
        ("topic_id" = Uuid, Path, description = "Topic ID"),
        ("reply_id" = Uuid, Path, description = "Reply ID")
    ),
    responses(
        (status = 204, description = "mark topic solution acknowledged"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn mark_topic_solution(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path((topic_id, reply_id)): Path<(Uuid, Uuid)>,
) -> HttpResult<StatusCode> {
    if tenant.id.is_nil() || auth.tenant_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_user_id",
            "Auth user ID must not be nil",
        ));
    }
    if topic_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_topic_id",
            "Topic ID must not be nil",
        ));
    }
    if reply_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_reply_id",
            "Reply ID must not be nil",
        ));
    }
    tracing::debug!(
        tenant_id = %tenant.id,
        topic_id = %topic_id,
        reply_id = %reply_id,
        user_id = %auth.user_id,
        "mark_topic_solution"
    );

    let audience_context = moderation_audience_port_context(
        ForumModerationTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;

    runtime
        .moderation_service()
        .mark_solution_with_audience_context(
            tenant.id,
            topic_id,
            reply_id,
            forum_security(&auth),
            audience_context,
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/forum/topics/{topic_id}/solution",
    tag = "forum",
    params(("topic_id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 204, description = "clear topic solution acknowledged"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn clear_topic_solution(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    if tenant.id.is_nil() || auth.tenant_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_user_id",
            "Auth user ID must not be nil",
        ));
    }
    if topic_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_topic_id",
            "Topic ID must not be nil",
        ));
    }
    tracing::debug!(
        tenant_id = %tenant.id,
        topic_id = %topic_id,
        user_id = %auth.user_id,
        "clear_topic_solution"
    );

    let audience_context = moderation_audience_port_context(
        ForumModerationTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;

    runtime
        .moderation_service()
        .clear_solution_with_audience_context(
            tenant.id,
            topic_id,
            forum_security(&auth),
            audience_context,
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;

    Ok(StatusCode::NO_CONTENT)
}
