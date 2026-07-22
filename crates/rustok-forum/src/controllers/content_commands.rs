use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ReplyResponse, ReplyService, TopicResponse,
    TopicService, UpdateReplyCommandInput, UpdateTopicCommandInput,
};

#[utoipa::path(
    post,
    path = "/api/forum/topics",
    tag = "forum",
    request_body = CreateTopicCommandInput,
    responses(
        (status = 201, description = "Topic created with inline quote relations", body = TopicResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateTopicCommandInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_CREATE,
        "Permission denied: forum_topics:create required",
    )?;
    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .create_command(tenant.id, forum_security(&auth), input)
        .await
        .map_err(command_error)?;
    Ok((StatusCode::CREATED, Json(topic)))
}

#[utoipa::path(
    put,
    path = "/api/forum/topics/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = UpdateTopicCommandInput,
    responses(
        (status = 200, description = "Topic and inline quote relations updated atomically", body = TopicResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Topic not found")
    )
)]
pub async fn update_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<UpdateTopicCommandInput>,
) -> HttpResult<Json<TopicResponse>> {
    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_UPDATE,
        "Permission denied: forum_topics:update required",
    )?;
    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .update_command(tenant.id, topic_id, forum_security(&auth), input)
        .await
        .map_err(command_error)?;
    Ok(Json(topic))
}

#[utoipa::path(
    post,
    path = "/api/forum/topics/{id}/replies",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Topic ID")),
    request_body = CreateReplyCommandInput,
    responses(
        (status = 201, description = "Reply created with inline quote relations", body = ReplyResponse),
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
    Json(input): Json<CreateReplyCommandInput>,
) -> HttpResult<(StatusCode, Json<ReplyResponse>)> {
    ensure_permission(
        &auth,
        Permission::FORUM_REPLIES_CREATE,
        "Permission denied: forum_replies:create required",
    )?;
    let reply = ReplyService::new(runtime.db_clone(), runtime.event_bus())
        .create_command(tenant.id, forum_security(&auth), topic_id, input)
        .await
        .map_err(command_error)?;
    Ok((StatusCode::CREATED, Json(reply)))
}

#[utoipa::path(
    put,
    path = "/api/forum/replies/{id}",
    tag = "forum",
    params(("id" = Uuid, Path, description = "Reply ID")),
    request_body = UpdateReplyCommandInput,
    responses(
        (status = 200, description = "Reply and inline quote relations updated atomically", body = ReplyResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Reply not found")
    )
)]
pub async fn update_reply(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(reply_id): Path<Uuid>,
    Json(input): Json<UpdateReplyCommandInput>,
) -> HttpResult<Json<ReplyResponse>> {
    ensure_permission(
        &auth,
        Permission::FORUM_REPLIES_UPDATE,
        "Permission denied: forum_replies:update required",
    )?;
    let reply = ReplyService::new(runtime.db_clone(), runtime.event_bus())
        .update_command(tenant.id, reply_id, forum_security(&auth), input)
        .await
        .map_err(command_error)?;
    Ok(Json(reply))
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

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn command_error(error: crate::ForumError) -> HttpError {
    match error {
        crate::ForumError::Database(error) => HttpError::internal(error.to_string()),
        crate::ForumError::Content(error) => HttpError::internal(error.to_string()),
        crate::ForumError::Internal(error) => HttpError::internal(error.to_string()),
        crate::ForumError::Forbidden(message) => {
            HttpError::forbidden("forum_permission_denied", message)
        }
        crate::ForumError::TopicNotFound(id) => HttpError::not_found(
            "forum_topic_not_found",
            format!("Topic not found: {id}"),
        ),
        crate::ForumError::ReplyNotFound(id) => HttpError::not_found(
            "forum_reply_not_found",
            format!("Reply not found: {id}"),
        ),
        error => HttpError::bad_request(error.stable_code(), error.to_string()),
    }
}
