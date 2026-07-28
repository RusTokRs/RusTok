use axum::{Json, extract::{Path, State}};
use rustok_api::{AuthContext, Permission, RequestContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{TopicResponse, TopicService};
use crate::moderation_transport::{
    ForumModerationTransport, moderation_audience_port_context,
};

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

fn ensure_solution_permission(auth: &AuthContext) -> HttpResult<()> {
    let permissions = [
        Permission::FORUM_TOPICS_UPDATE,
        Permission::FORUM_TOPICS_MODERATE,
    ];
    if !has_any_effective_permission(&auth.permissions, &permissions) {
        return Err(HttpError::forbidden(
            "FORBIDDEN",
            "Permission denied: forum_topics:update or forum_topics:moderate required",
        ));
    }
    Ok(())
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
        (status = 200, description = "Topic solution marked", body = TopicResponse),
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
) -> HttpResult<Json<TopicResponse>> {
    ensure_solution_permission(&auth)?;
    let audience_context = moderation_audience_port_context(
        ForumModerationTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;

    let event_bus = runtime.event_bus();
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

    let topic = TopicService::new(runtime.db_clone(), event_bus)
        .get_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            topic_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(topic))
}

#[utoipa::path(
    delete,
    path = "/api/forum/topics/{topic_id}/solution",
    tag = "forum",
    params(("topic_id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 200, description = "Topic solution cleared", body = TopicResponse),
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
) -> HttpResult<Json<TopicResponse>> {
    ensure_solution_permission(&auth)?;
    let audience_context = moderation_audience_port_context(
        ForumModerationTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;

    let event_bus = runtime.event_bus();
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

    let topic = TopicService::new(runtime.db_clone(), event_bus)
        .get_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            topic_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(topic))
}
