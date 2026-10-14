use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{AuthContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::Deserialize;
use uuid::Uuid;

use super::{MessengerHttpRuntime, ensure_messenger_module_enabled};
use crate::dto::{
    ChannelTranslationOutput, ConversationOutput, CreateConversationInput, CursorOutput,
    FastSyncRequest, FastSyncResponse, ListMessagesQuery, MessageOutput, SendMessageInput,
    UpdateChannelTranslationInput, UpdateCursorInput,
};

#[derive(Debug, Clone, Deserialize)]
pub struct ListConversationsQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FastSyncQueryParams {
    pub since_seq: i64,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCursorBody {
    pub last_read_message_id: i64,
}

pub async fn create_conversation(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateConversationInput>,
) -> HttpResult<(StatusCode, Json<ConversationOutput>)> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .create_conversation(tenant_id, actor_id, input)
        .await?;

    Ok((StatusCode::CREATED, Json(output)))
}

pub async fn list_conversations(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<ListConversationsQuery>,
) -> HttpResult<Json<Vec<ConversationOutput>>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let output = runtime
        .service
        .list_conversations(tenant_id, actor_id, limit, offset)
        .await?;

    Ok(Json(output))
}

pub async fn get_conversation(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ConversationOutput>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .get_conversation(tenant_id, actor_id, id, None)
        .await?;

    Ok(Json(output))
}

pub async fn update_channel_translation(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateChannelTranslationInput>,
) -> HttpResult<Json<ChannelTranslationOutput>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    if input.channel_id != id {
        return Err(HttpError::bad_request("MISMATCHED_ID", "Path ID and body channel_id do not match"));
    }

    let output = runtime
        .service
        .update_channel_translation(tenant_id, actor_id, input)
        .await?;

    Ok(Json(output))
}

pub async fn send_message(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<SendMessageInput>,
) -> HttpResult<(StatusCode, Json<MessageOutput>)> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .send_message(tenant_id, actor_id, input)
        .await?;

    Ok((StatusCode::CREATED, Json(output)))
}

pub async fn list_messages(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<ListMessagesQuery>,
) -> HttpResult<Json<Vec<MessageOutput>>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let mut q = query;
    q.conversation_id = conversation_id;

    let output = runtime
        .service
        .list_messages(tenant_id, actor_id, q)
        .await?;

    Ok(Json(output))
}

pub async fn fast_sync(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<FastSyncQueryParams>,
) -> HttpResult<Json<FastSyncResponse>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .fast_sync(
            tenant_id,
            actor_id,
            FastSyncRequest {
                conversation_id,
                since_seq: query.since_seq,
                limit: query.limit,
            },
        )
        .await?;

    Ok(Json(output))
}

pub async fn update_cursor(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<UpdateCursorBody>,
) -> HttpResult<Json<CursorOutput>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .update_cursor(
            tenant_id,
            actor_id,
            UpdateCursorInput {
                conversation_id,
                last_read_message_id: body.last_read_message_id,
            },
        )
        .await?;

    Ok(Json(output))
}
