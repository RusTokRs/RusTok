use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext, has_any_effective_permission,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::reply_create_transport::{
    ForumReplyCreateTransport, reply_create_audience_port_context,
};
use crate::topic_create_transport::{
    ForumTopicCreateTransport, topic_create_audience_port_context,
};
use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ReplyResponse, TopicResponse,
    UpdateReplyCommandInput, UpdateTopicCommandInput,
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
    request_context: RequestContext,
    Json(input): Json<CreateTopicCommandInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if input.category_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_category_id",
            "Category ID must not be nil",
        ));
    }
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %auth.user_id,
        category_id = %input.category_id,
        title = %input.title,
        "create_topic"
    );

    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_CREATE,
        "Permission denied: forum_topics:create required",
    )?;
    let audience_context = topic_create_audience_port_context(
        ForumTopicCreateTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(command_error)?;
    let topic = runtime
        .topic_service()
        .create_command_with_audience_context(
            tenant.id,
            forum_security(&auth),
            audience_context,
            input,
        )
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
        (status = 404, description = "Topic not found"),
        (status = 409, description = "Relation revision changed concurrently")
    )
)]
pub async fn update_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<UpdateTopicCommandInput>,
) -> HttpResult<Json<TopicResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if topic_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_topic_id",
            "Topic ID must not be nil",
        ));
    }
    tracing::debug!(tenant_id = %tenant.id, topic_id = %topic_id, user_id = %auth.user_id, "update_topic");

    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_UPDATE,
        "Permission denied: forum_topics:update required",
    )?;
    let topic = runtime
        .topic_service()
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
    request_context: RequestContext,
    Path(topic_id): Path<Uuid>,
    Json(input): Json<CreateReplyCommandInput>,
) -> HttpResult<(StatusCode, Json<ReplyResponse>)> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if topic_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_topic_id",
            "Topic ID must not be nil",
        ));
    }
    tracing::debug!(tenant_id = %tenant.id, topic_id = %topic_id, user_id = %auth.user_id, "create_reply");

    ensure_permission(
        &auth,
        Permission::FORUM_REPLIES_CREATE,
        "Permission denied: forum_replies:create required",
    )?;
    let audience_context = reply_create_audience_port_context(
        ForumReplyCreateTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(command_error)?;
    let reply = runtime
        .reply_service()
        .create_command_with_audience_context(
            tenant.id,
            forum_security(&auth),
            topic_id,
            audience_context,
            input,
        )
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
        (status = 404, description = "Reply not found"),
        (status = 409, description = "Relation revision changed concurrently")
    )
)]
pub async fn update_reply(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(reply_id): Path<Uuid>,
    Json(input): Json<UpdateReplyCommandInput>,
) -> HttpResult<Json<ReplyResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if reply_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_reply_id",
            "Reply ID must not be nil",
        ));
    }
    tracing::debug!(tenant_id = %tenant.id, reply_id = %reply_id, user_id = %auth.user_id, "update_reply");

    ensure_permission(
        &auth,
        Permission::FORUM_REPLIES_UPDATE,
        "Permission denied: forum_replies:update required",
    )?;
    let reply = runtime
        .reply_service()
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
    if auth.tenant_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Auth tenant ID must not be nil",
        ));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_user_id",
            "Auth user ID must not be nil",
        ));
    }
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
    crate::controllers::map_forum_error(error)
}
