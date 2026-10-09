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
    ForumTopicAudienceListService, ForumTopicAudienceReadService, ForumTopicReadOperation,
    ForumTopicReadTransport, ListTopicsFilter, TopicListItemPage, TopicResponse,
    topic_read_audience_port_context,
};

/// REST query for the topic list. `per_page` stays optional so an omitted value resolves to
/// the tenant's `topics_per_page` setting instead of a transport constant.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListTopicsQuery {
    pub category_id: Option<uuid::Uuid>,
    pub status: Option<crate::TopicStatus>,
    pub locale: Option<String>,
    /// Opaque cursor returned as `next_cursor` by the previous page.
    pub after: Option<String>,
    pub per_page: Option<u64>,
}

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
    crate::dto::bounded_forum_read_limit(Some(per_page))
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
    params(ListTopicsQuery),
    responses(
        (status = 200, description = "One keyset page of topics", body = TopicListItemPage),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_topics(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(query): Query<ListTopicsQuery>,
) -> HttpResult<Json<TopicListItemPage>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_LIST],
        "Permission denied: forum_topics:list required",
    )?;

    let requested_limit = query.per_page;
    let effective_limit = match requested_limit {
        Some(value) => clamp_per_page(value),
        None => runtime
            .settings_providers
            .default_topics_per_page(tenant.id)
            .await
            .map_err(crate::controllers::map_forum_error)?,
    };
    let filter = ListTopicsFilter {
        category_id: query.category_id,
        status: query.status,
        locale: query.locale.or(Some(request_context.locale.clone())),
        after: query.after,
        per_page: effective_limit,
    };
    let audience_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::TopicList,
        tenant.id,
        &auth,
        Some(&request_context),
        filter
            .locale
            .as_deref()
            .unwrap_or(tenant.default_locale.as_str()),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let list_started_at = Instant::now();
    let page = topic_audience_list_service(&runtime)
        .list_authenticated_owner_visible_with_audience_context(
            tenant.id,
            forum_security(&auth),
            audience_context,
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
        page.items.len() as u64,
    );
    metrics::record_read_path_budget(
        "http",
        "forum.list_topics",
        requested_limit,
        effective_limit,
        page.items.len(),
    );

    Ok(Json(TopicListItemPage {
        items: page.items,
        next_cursor: page.next_cursor,
    }))
}

pub(super) fn topic_audience_read_service(
    runtime: &crate::controllers::ForumHttpRuntime,
) -> ForumTopicAudienceReadService {
    match runtime.audience_facts.clone() {
        Some(facts) => ForumTopicAudienceReadService::with_audience_facts(
            runtime.db_clone(),
            runtime.event_bus(),
            facts,
        ),
        None => ForumTopicAudienceReadService::new(runtime.db_clone(), runtime.event_bus()),
    }
}

fn topic_audience_list_service(
    runtime: &crate::controllers::ForumHttpRuntime,
) -> ForumTopicAudienceListService {
    let service = match runtime.audience_facts.clone() {
        Some(facts) => ForumTopicAudienceListService::with_audience_facts(
            runtime.db_clone(),
            runtime.event_bus(),
            facts,
        ),
        None => ForumTopicAudienceListService::new(runtime.db_clone(), runtime.event_bus()),
    };
    service.with_settings_providers(runtime.settings_providers.clone())
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
    let audience_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::SelectedTopic,
        tenant.id,
        &auth,
        Some(&request_context),
        locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let topic = topic_audience_read_service(&runtime)
        .get_authenticated_owner_visible_with_audience_context(
            tenant.id,
            forum_security(&auth),
            audience_context,
            id,
            Some(tenant.default_locale.as_str()),
        )
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

    runtime
        .topic_service()
        .delete(tenant.id, id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/forum/topics/{id}/restore",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 204, description = "Topic restored"),
        (status = 400, description = "Topic restore is unavailable"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Topic cannot be restored"),
        (status = 404, description = "Topic not found")
    )
)]
pub async fn restore_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    runtime
        .topic_service()
        .restore(tenant.id, id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(StatusCode::NO_CONTENT)
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

    let write_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::Vote,
        tenant.id,
        &auth,
        Some(&request_context),
        request_context.locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    runtime
        .vote_service()
        .set_topic_vote(
            tenant.id,
            topic_id,
            forum_security(&auth),
            write_context,
            value,
        )
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let read_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::SelectedTopic,
        tenant.id,
        &auth,
        Some(&request_context),
        request_context.locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let topic = topic_audience_read_service(&runtime)
        .get_authenticated_owner_visible_with_audience_context(
            tenant.id,
            forum_security(&auth),
            read_context,
            topic_id,
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

    runtime
        .vote_service()
        .clear_topic_vote(tenant.id, topic_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let read_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::SelectedTopic,
        tenant.id,
        &auth,
        Some(&request_context),
        request_context.locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let topic = topic_audience_read_service(&runtime)
        .get_authenticated_owner_visible_with_audience_context(
            tenant.id,
            forum_security(&auth),
            read_context,
            topic_id,
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

    let write_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::Subscription,
        tenant.id,
        &auth,
        Some(&request_context),
        request_context.locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    runtime
        .subscription_service()
        .set_topic_subscription(tenant.id, topic_id, forum_security(&auth), write_context)
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let read_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::SelectedTopic,
        tenant.id,
        &auth,
        Some(&request_context),
        request_context.locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let topic = topic_audience_read_service(&runtime)
        .get_authenticated_owner_visible_with_audience_context(
            tenant.id,
            forum_security(&auth),
            read_context,
            topic_id,
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

    runtime
        .subscription_service()
        .clear_topic_subscription(tenant.id, topic_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;

    let read_context = topic_read_audience_port_context(
        ForumTopicReadTransport::Rest,
        ForumTopicReadOperation::SelectedTopic,
        tenant.id,
        &auth,
        Some(&request_context),
        request_context.locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let topic = topic_audience_read_service(&runtime)
        .get_authenticated_owner_visible_with_audience_context(
            tenant.id,
            forum_security(&auth),
            read_context,
            topic_id,
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
