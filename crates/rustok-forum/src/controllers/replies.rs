use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::Permission;
use rustok_api::{has_any_effective_permission, AuthContext, RequestContext, TenantContext};
use rustok_outbox::loco::transactional_event_bus_from_context;
use rustok_telemetry::metrics;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse, ReplyService,
    UpdateReplyInput, VoteService,
};

fn clamp_per_page(per_page: u64) -> u64 {
    per_page.min(100)
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
    Query(mut filter): Query<ListRepliesFilter>,
) -> Result<Json<Vec<ReplyListItem>>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_LIST],
        "Permission denied: forum_replies:list required",
    )?;

    filter.locale = filter.locale.or(Some(request_context.locale.clone()));
    let requested_limit = Some(filter.per_page);
    let effective_limit = clamp_per_page(filter.per_page);
    filter.per_page = effective_limit;
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let (replies, _) = service
        .list_for_topic_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            topic_id,
            filter,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(filter): Query<ListRepliesFilter>,
) -> Result<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_READ],
        "Permission denied: forum_replies:read required",
    )?;

    let locale = filter
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .get_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            id,
            &locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<CreateReplyInput>,
) -> Result<(StatusCode, Json<ReplyResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_CREATE],
        "Permission denied: forum_replies:create required",
    )?;

    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .create(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            topic_id,
            input,
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateReplyInput>,
) -> Result<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_UPDATE],
        "Permission denied: forum_replies:update required",
    )?;

    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .update(
            tenant.id,
            id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            input,
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_DELETE],
        "Permission denied: forum_replies:delete required",
    )?;

    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete(
            tenant.id,
            id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path((reply_id, value)): Path<(Uuid, i32)>,
) -> Result<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_READ],
        "Permission denied: forum_replies:read required",
    )?;

    VoteService::new(ctx.db.clone())
        .set_reply_vote(
            tenant.id,
            reply_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            value,
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .get_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            reply_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(reply_id): Path<Uuid>,
) -> Result<Json<ReplyResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_REPLIES_READ],
        "Permission denied: forum_replies:read required",
    )?;

    VoteService::new(ctx.db.clone())
        .clear_reply_vote(
            tenant.id,
            reply_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .get_with_locale_fallback(
            tenant.id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            reply_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(reply))
}

fn ensure_forum_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }

    Ok(())
}
