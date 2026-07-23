use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext, has_any_effective_permission};
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use serde::Deserialize;
use std::time::Instant;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    CreateTopicInput, ListTopicsFilter, ModerationService, SubscriptionService, TopicListItem,
    TopicResponse, TopicService, UpdateTopicInput, VoteService,
};

#[derive(Debug, Clone, Copy, Deserialize, IntoParams, ToSchema)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

impl PaginationParams {
    pub fn limit(&self) -> u64 {
        clamp_per_page(self.per_page)
    }
}

fn clamp_per_page(per_page: u64) -> u64 {
    per_page.min(100)
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

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
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(mut filter): Query<ListTopicsFilter>,
) -> HttpResult<Json<Vec<TopicListItem>>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_LIST],
        "Permission denied: forum_topics:list required",
    )?;

    filter.locale = filter.locale.or(Some(request_context.locale.clone()));
    let requested_limit = Some(filter.per_page);
    let effective_limit = clamp_per_page(filter.per_page);
    filter.per_page = effective_limit;
    let service = TopicService::new(runtime.db_clone(), runtime.event_bus());
    let list_started_at = Instant::now();
    let (topics, _) = service
        .list_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            filter,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
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

#[cfg(test)]
mod tests {
    use super::{PaginationParams, clamp_per_page};

    #[test]
    fn pagination_params_limit_clamps_large_page_size() {
        let params = PaginationParams {
            page: 1,
            per_page: 500,
        };
        assert_eq!(params.limit(), 100);
    }

    #[test]
    fn controller_clamp_per_page_caps_large_values() {
        assert_eq!(clamp_per_page(20), 20);
        assert_eq!(clamp_per_page(100), 100);
        assert_eq!(clamp_per_page(1000), 100);
    }
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
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(filter): Query<ListTopicsFilter>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let locale = filter
        .locale
        .unwrap_or_else(|| request_context.locale.clone());
    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .get_with_locale_fallback(
            tenant.id,
            forum_security(&auth),
            id,
            &locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;
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
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateTopicInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_CREATE],
        "Permission denied: forum_topics:create required",
    )?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .create(tenant.id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok((StatusCode::CREATED, Json(topic)))
}

#[utoipa::path(
    put,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = UpdateTopicInput,
    responses(
        (status = 200, description = "Topic updated", body = TopicResponse),
        (status = 404, description = "Topic not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTopicInput>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_UPDATE],
        "Permission denied: forum_topics:update required",
    )?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .update(tenant.id, id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(topic))
}

#[utoipa::path(
    delete,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 204, description = "Topic deleted"),
        (status = 404, description = "Topic not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_DELETE],
        "Permission denied: forum_topics:delete required",
    )?;

    TopicService::new(runtime.db_clone(), runtime.event_bus())
        .delete(tenant.id, id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(StatusCode::NO_CONTENT)
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
    ensure_forum_permission(
        &auth,
        &[
            Permission::FORUM_TOPICS_UPDATE,
            Permission::FORUM_TOPICS_MODERATE,
        ],
        "Permission denied: forum_topics:update or forum_topics:moderate required",
    )?;

    let event_bus = runtime.event_bus();
    ModerationService::new(runtime.db_clone(), event_bus.clone())
        .mark_solution(tenant.id, topic_id, reply_id, forum_security(&auth))
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
    ensure_forum_permission(
        &auth,
        &[
            Permission::FORUM_TOPICS_UPDATE,
            Permission::FORUM_TOPICS_MODERATE,
        ],
        "Permission denied: forum_topics:update or forum_topics:moderate required",
    )?;

    let event_bus = runtime.event_bus();
    ModerationService::new(runtime.db_clone(), event_bus.clone())
        .clear_solution(tenant.id, topic_id, forum_security(&auth))
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
    post,
    path = "/api/forum/topics/{topic_id}/vote/{value}",
    tag = "forum",
    params(
        ("topic_id" = Uuid, Path, description = "Topic ID"),
        ("value" = i32, Path, description = "Vote value (-1 or 1)")
    ),
    responses(
        (status = 200, description = "Topic vote updated", body = TopicResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn set_topic_vote(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path((topic_id, value)): Path<(Uuid, i32)>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    VoteService::new(runtime.db_clone())
        .set_topic_vote(tenant.id, topic_id, forum_security(&auth), value)
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
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
    path = "/api/forum/topics/{topic_id}/vote",
    tag = "forum",
    params(("topic_id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 200, description = "Topic vote cleared", body = TopicResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn clear_topic_vote(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    VoteService::new(runtime.db_clone())
        .clear_topic_vote(tenant.id, topic_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
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
    post,
    path = "/api/forum/topics/{topic_id}/subscription",
    tag = "forum",
    params(("topic_id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 200, description = "Topic subscription updated", body = TopicResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn subscribe_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    SubscriptionService::new(runtime.db_clone())
        .set_topic_subscription(tenant.id, topic_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
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
    path = "/api/forum/topics/{topic_id}/subscription",
    tag = "forum",
    params(("topic_id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 200, description = "Topic subscription cleared", body = TopicResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn unsubscribe_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    SubscriptionService::new(runtime.db_clone())
        .clear_topic_subscription(tenant.id, topic_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
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
