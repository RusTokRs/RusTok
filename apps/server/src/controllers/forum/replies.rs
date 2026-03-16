use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use rustok_forum::{
    CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse, ReplyService,
    UpdateReplyInput,
};
use rustok_telemetry::metrics;
use std::time::Instant;
use uuid::Uuid;

use crate::extractors::rbac::{
    RequireForumRepliesCreate, RequireForumRepliesRead, RequireForumTopicsModerate,
};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::{common::RequestContext, context::TenantContext};

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
    RequireForumRepliesRead(user): RequireForumRepliesRead,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
    Query(mut filter): Query<ListRepliesFilter>,
) -> Result<Json<Vec<ReplyListItem>>> {
    filter.locale = filter.locale.or(Some(request_context.locale.clone()));
    let requested_limit = Some(filter.per_page);
    let effective_limit = filter.per_page.min(100);
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let (replies, _) = service
        .list_for_topic_with_locale_fallback(
            tenant.id,
            user.security_context(),
            topic_id,
            filter,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    _user: RequireForumRepliesRead,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(filter): Query<ListRepliesFilter>,
) -> Result<Json<ReplyResponse>> {
    let locale = filter
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .get_with_locale_fallback(tenant.id, id, &locale, Some(tenant.default_locale.as_str()))
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(reply))
}

#[utoipa::path(
    post,
    path = "/api/forum/topics/{id}/replies",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Topic ID")
    ),
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
    RequireForumRepliesCreate(user): RequireForumRepliesCreate,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<CreateReplyInput>,
) -> Result<(StatusCode, Json<ReplyResponse>)> {
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .create(tenant.id, user.security_context(), topic_id, input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok((StatusCode::CREATED, Json(reply)))
}

#[utoipa::path(
    put,
    path = "/api/forum/replies/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Reply ID")
    ),
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
    RequireForumTopicsModerate(user): RequireForumTopicsModerate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateReplyInput>,
) -> Result<Json<ReplyResponse>> {
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let reply = service
        .update(tenant.id, id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(reply))
}

#[utoipa::path(
    delete,
    path = "/api/forum/replies/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Reply ID")
    ),
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
    RequireForumTopicsModerate(user): RequireForumTopicsModerate,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let service = ReplyService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
