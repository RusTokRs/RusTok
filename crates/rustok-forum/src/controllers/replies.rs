use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext, has_any_effective_permission};
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use std::time::Instant;
use uuid::Uuid;

use crate::{
    CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse, ReplyService,
    UpdateReplyInput, VoteService,
};

fn clamp_per_page(per_page: u64) -> u64 {
    per_page.min(100)
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[utoipa::path(
    get,
    path = "/api/forum/topics/{id}/replies",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Topic ID"),
        ListRepliesFilter,
    ),
    responses(
        (status = 200, description = "List of replies", body = Vec<ReplyListItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_replies(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
    Query(mut filter): Query<ListRepliesFilter>,
) -> HttpResult<Json<Vec<ReplyListItem>>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_LIST],
        "Permission denied: forum_replies:list required",
    )?;

    filter.locale = filter.locale.or(Some(request_context.locale.clone()));
    let requested_limit = Some(filter.per_page);
    let effective_limit = clamp_per_page(filter.per_page);
    filter.per_page = effective_limit;
    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    let list_started_at = Instant::now();
    let (replies, _) = service
        .list_for_topic_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            topic_id,
            filter,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    metrics::record_read_path_query(
        "http",
        "forum.list_replies",
        "service_list",
        list_started_at.elapsed().as_secs_f64(),
        replies.len() as u64,
    );

    metrics::record_read_path_budget(
        "http",
        "forum.list_replies",
        requested_limit,
        effective_limit,
        replies.len(),
    );

    Ok(Json(replies))
}

#[cfg(test)]
mod tests {
    use super::clamp_per_page;

    #[test]
    fn replies_controller_clamp_per_page_caps_large_values() {
        assert_eq!(clamp_per_page(20), 20);
        assert_eq!(clamp_per_page(100), 100);
        assert_eq!(clamp_per_page(250), 100);
    }
}

#[utoipa::path(
    get,
    path = "/api/forum/replies/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Reply ID"),
        ("locale" = Option<String>, Query, description = "Locale")
    ),
    responses(
        (status = 200, description = "Reply details", body = ReplyResponse),
        (status = 404, description = "Reply not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_reply(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(filter): Query<ListRepliesFilter>,
) -> HttpResult<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_READ],
        "Permission denied: forum_replies:read required",
    )?;

    let locale = filter
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    let reply = service
        .get_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            id,
            &locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(reply))
}

#[utoipa::path(
    post,
    path = "/api/forum/topics/{id}/replies",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = CreateReplyInput,
    responses(
        (status = 201, description = "Reply created", body = ReplyResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_reply(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<CreateReplyInput>,
) -> HttpResult<(StatusCode, Json<ReplyResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_CREATE],
        "Permission denied: forum_replies:create required",
    )?;

    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    let reply = service
        .create(tenant.id, forum_security(&auth), topic_id, input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok((StatusCode::CREATED, Json(reply)))
}

#[utoipa::path(
    put,
    path = "/api/forum/replies/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Reply ID")),
    request_body = UpdateReplyInput,
    responses(
        (status = 200, description = "Reply updated", body = ReplyResponse),
        (status = 404, description = "Reply not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_reply(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateReplyInput>,
) -> HttpResult<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_UPDATE],
        "Permission denied: forum_replies:update required",
    )?;

    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    let reply = service
        .update(tenant.id, id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(reply))
}

#[utoipa::path(
    delete,
    path = "/api/forum/replies/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Reply ID")),
    responses(
        (status = 204, description = "Reply deleted"),
        (status = 404, description = "Reply not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_reply(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_DELETE],
        "Permission denied: forum_replies:delete required",
    )?;

    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    service
        .delete(tenant.id, id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/forum/replies/{reply_id}/vote/{value}",
    tag = "forum",
    params(
        ("reply_id" = Uuid, Path, description = "Reply ID"),
        ("value" = i32, Path, description = "Vote value (-1 or 1)")
    ),
    responses(
        (status = 200, description = "Reply vote updated", body = ReplyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn set_reply_vote(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path((reply_id, value)): Path<(Uuid, i32)>,
) -> HttpResult<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_READ],
        "Permission denied: forum_replies:read required",
    )?;

    VoteService::new(runtime.db_clone())
        .set_reply_vote(tenant.id, reply_id, forum_security(&auth), value)
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    let reply = service
        .get_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            reply_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(reply))
}

#[utoipa::path(
    delete,
    path = "/api/forum/replies/{reply_id}/vote",
    tag = "forum",
    params(("reply_id" = Uuid, Path, description = "Reply ID")),
    responses(
        (status = 200, description = "Reply vote cleared", body = ReplyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn clear_reply_vote(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(reply_id): Path<Uuid>,
) -> HttpResult<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_READ],
        "Permission denied: forum_replies:read required",
    )?;

    VoteService::new(runtime.db_clone())
        .clear_reply_vote(tenant.id, reply_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let service = ReplyService::new(runtime.db_clone(), runtime.event_bus());
    let reply = service
        .get_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            reply_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(reply))
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
