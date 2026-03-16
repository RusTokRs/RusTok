use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use rustok_forum::{
    CreateTopicInput, ListTopicsFilter, TopicListItem, TopicResponse, TopicService,
    UpdateTopicInput,
};
use rustok_telemetry::metrics;
use std::time::Instant;
use uuid::Uuid;

use crate::extractors::rbac::{
    RequireForumTopicsCreate, RequireForumTopicsDelete, RequireForumTopicsList,
    RequireForumTopicsRead, RequireForumTopicsUpdate,
};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::{common::RequestContext, context::TenantContext};

#[utoipa::path(
    get,
    path = "/api/forum/topics",
    tag = "forum",
    params(ListTopicsFilter),
    responses(
        (status = 200, description = "List of topics", body = Vec<TopicListItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_topics(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumTopicsList(user): RequireForumTopicsList,
    request_context: RequestContext,
    Query(mut filter): Query<ListTopicsFilter>,
) -> Result<Json<Vec<TopicListItem>>> {
    filter.locale = filter.locale.or(Some(request_context.locale.clone()));
    let requested_limit = Some(filter.per_page);
    let effective_limit = filter.per_page.min(100);
    let service = TopicService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let (topics, _) = service
        .list_with_locale_fallback(
            tenant.id,
            user.security_context(),
            filter,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    metrics::record_read_path_query(
        "http",
        "forum.list_topics",
        "service_list",
        list_started_at.elapsed().as_secs_f64(),
        topics.len() as u64,
    );

    metrics::record_read_path_budget(
        "http",
        "forum.list_topics",
        requested_limit,
        effective_limit,
        topics.len(),
    );

    Ok(Json(topics))
}

#[utoipa::path(
    get,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Topic ID"),
        ("locale" = Option<String>, Query, description = "Locale")
    ),
    responses(
        (status = 200, description = "Topic details", body = TopicResponse),
        (status = 404, description = "Topic not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_topic(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _user: RequireForumTopicsRead,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(filter): Query<ListTopicsFilter>,
) -> Result<Json<TopicResponse>> {
    let locale = filter
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let service = TopicService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let topic = service
        .get_with_locale_fallback(tenant.id, id, &locale, Some(tenant.default_locale.as_str()))
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(topic))
}

#[utoipa::path(
    post,
    path = "/api/forum/topics",
    tag = "forum",
    request_body = CreateTopicInput,
    responses(
        (status = 201, description = "Topic created", body = TopicResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_topic(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumTopicsCreate(user): RequireForumTopicsCreate,
    Json(input): Json<CreateTopicInput>,
) -> Result<(StatusCode, Json<TopicResponse>)> {
    let service = TopicService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let topic = service
        .create(tenant.id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok((StatusCode::CREATED, Json(topic)))
}

#[utoipa::path(
    put,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Topic ID")
    ),
    request_body = UpdateTopicInput,
    responses(
        (status = 200, description = "Topic updated", body = TopicResponse),
        (status = 404, description = "Topic not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_topic(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumTopicsUpdate(user): RequireForumTopicsUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTopicInput>,
) -> Result<Json<TopicResponse>> {
    let service = TopicService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let topic = service
        .update(tenant.id, id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(topic))
}

#[utoipa::path(
    delete,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(
        ("id" = Uuid, Path, description = "Topic ID")
    ),
    responses(
        (status = 204, description = "Topic deleted"),
        (status = 404, description = "Topic not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_topic(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireForumTopicsDelete(user): RequireForumTopicsDelete,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let service = TopicService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
