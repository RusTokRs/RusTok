use axum::{
    Json,
    extract::{Path, Query, State},
};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext, has_any_effective_permission,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    ForumReadModelService, ForumTopicReadState, ForumTopicReadStateService,
    MarkForumTopicReadInput, MarkForumTopicsReadBatchInput, MarkForumTopicsReadBatchResult,
    TopicUnreadCursorPage, TopicUnreadCursorQuery,
};

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

#[utoipa::path(
    get,
    path = "/api/forum/topics/unread",
    tag = "forum",
    params(
        ("cursor" = Option<String>, Query, description = "Opaque topic cursor"),
        ("limit" = Option<u64>, Query, description = "Bounded page size, maximum 100"),
        ("category_id" = Option<Uuid>, Query, description = "Optional direct category filter"),
        ("status" = Option<crate::TopicStatus>, Query, description = "Optional topic status"),
        ("locale" = Option<String>, Query, description = "Requested locale"),
        ("fallback_locale" = Option<String>, Query, description = "Fallback locale"),
        ("unread_only" = Option<bool>, Query, description = "Return only unread topics")
    ),
    responses(
        (status = 200, description = "Authenticated unread topic projection", body = TopicUnreadCursorPage),
        (status = 400, description = "Invalid cursor or query"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_unread_topics(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(mut query): Query<TopicUnreadCursorQuery>,
) -> HttpResult<Json<TopicUnreadCursorPage>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_LIST],
        "Permission denied: forum_topics:list required",
    )?;

    query.locale = query.locale.or(Some(request_context.locale));
    query.fallback_locale = query
        .fallback_locale
        .or(Some(tenant.default_locale.clone()));
    let page = ForumReadModelService::new(runtime.db_clone())
        .list_topics_with_unread(tenant.id, forum_security(&auth), query)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/api/forum/topics/{id}/read-state",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    responses(
        (status = 200, description = "Current authenticated topic read state", body = ForumTopicReadState),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Topic not found")
    )
)]
pub async fn get_topic_read_state(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
) -> HttpResult<Json<ForumTopicReadState>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let state = ForumTopicReadStateService::new(runtime.db_clone())
        .get_topic_read_state(tenant.id, topic_id, forum_security(&auth))
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(state))
}

#[utoipa::path(
    put,
    path = "/api/forum/topics/{id}/read-state",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = MarkForumTopicReadInput,
    responses(
        (status = 200, description = "Monotonic topic read state", body = ForumTopicReadState),
        (status = 400, description = "Invalid read high-water mark"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Topic not found")
    )
)]
pub async fn mark_topic_read(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<MarkForumTopicReadInput>,
) -> HttpResult<Json<ForumTopicReadState>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let state = ForumTopicReadStateService::new(runtime.db_clone())
        .mark_topic_read(tenant.id, topic_id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(state))
}

#[utoipa::path(
    post,
    path = "/api/forum/categories/{id}/mark-read",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Root category ID")),
    request_body = MarkForumTopicsReadBatchInput,
    responses(
        (status = 200, description = "Bounded category-subtree read batch", body = MarkForumTopicsReadBatchResult),
        (status = 400, description = "Invalid batch cursor or limit"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found")
    )
)]
pub async fn mark_category_read(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(category_id): Path<Uuid>,
    Json(input): Json<MarkForumTopicsReadBatchInput>,
) -> HttpResult<Json<MarkForumTopicsReadBatchResult>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let result = ForumTopicReadStateService::new(runtime.db_clone())
        .mark_category_read(tenant.id, category_id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(result))
}

#[utoipa::path(
    post,
    path = "/api/forum/topics/mark-all-read",
    tag = "forum",
    request_body = MarkForumTopicsReadBatchInput,
    responses(
        (status = 200, description = "Bounded tenant-wide read batch", body = MarkForumTopicsReadBatchResult),
        (status = 400, description = "Invalid batch cursor or limit"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn mark_all_topics_read(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<MarkForumTopicsReadBatchInput>,
) -> HttpResult<Json<MarkForumTopicsReadBatchResult>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let result = ForumTopicReadStateService::new(runtime.db_clone())
        .mark_all_read(tenant.id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok(Json(result))
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
