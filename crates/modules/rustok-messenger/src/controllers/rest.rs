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
    AddMemberInput, ChannelTranslationOutput, ConversationOutput, CreateConversationInput,
    CursorOutput, EditMessageInput, FastSyncRequest, FastSyncResponse, ListMessagesQuery,
    MemberOutput, MessageOutput, SearchMessagesQuery, SendMessageInput,
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

    let conversation_id = input.conversation_id;
    let output = runtime
        .service
        .send_message(tenant_id, actor_id, input)
        .await?;

    runtime.room_hub.broadcast(
        tenant_id,
        conversation_id,
        super::MessengerServerMessage::NewMessage(output.clone()),
    );

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

    runtime.room_hub.broadcast(
        tenant_id,
        conversation_id,
        super::MessengerServerMessage::CursorAdvanced(output.clone()),
    );

    Ok(Json(output))
}

pub async fn search_messages(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SearchMessagesQuery>,
) -> HttpResult<Json<Vec<MessageOutput>>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .search_messages(tenant_id, actor_id, query)
        .await?;

    Ok(Json(output))
}

pub async fn upload_device_keys(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<crate::e2ee::UploadDeviceKeysInput>,
) -> HttpResult<(StatusCode, Json<crate::e2ee::DeviceKey>)> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;
    let device_key = runtime
        .service
        .device_keys()
        .register_device_keys(actor_id, input)
        .map_err(|e| HttpError::bad_request("REGISTRATION_FAILED", e.to_string()))?;

    Ok((StatusCode::CREATED, Json(device_key)))
}

pub async fn list_user_devices(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> HttpResult<Json<Vec<crate::e2ee::DeviceKey>>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let devices = runtime.service.device_keys().list_user_devices(user_id);
    Ok(Json(devices))
}

pub async fn claim_prekey_bundle(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    _auth: AuthContext,
    Path((target_user_id, device_id)): Path<(Uuid, String)>,
) -> HttpResult<Json<crate::e2ee::PrekeyBundle>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let bundle = runtime
        .service
        .device_keys()
        .claim_prekey_bundle(target_user_id, &device_id)
        .map_err(|e| HttpError::not_found("DEVICE_NOT_FOUND", e.to_string()))?;

    Ok(Json(bundle))
}

pub async fn patch_message(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(message_id): Path<i64>,
    Json(input): Json<EditMessageInput>,
) -> HttpResult<Json<MessageOutput>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .edit_message(tenant_id, actor_id, message_id, input)
        .await?;

    runtime.room_hub.broadcast(
        tenant_id,
        output.conversation_id,
        super::MessengerServerMessage::MessageEdited(output.clone()),
    );

    Ok(Json(output))
}

pub async fn delete_message(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(message_id): Path<i64>,
) -> HttpResult<StatusCode> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let conv_id = runtime
        .service
        .delete_message(tenant_id, actor_id, message_id)
        .await?;

    runtime.room_hub.broadcast(
        tenant_id,
        conv_id,
        super::MessengerServerMessage::MessageDeleted { message_id },
    );

    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_member(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<AddMemberInput>,
) -> HttpResult<(StatusCode, Json<MemberOutput>)> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .add_member(tenant_id, actor_id, conversation_id, input)
        .await?;

    Ok((StatusCode::CREATED, Json(output)))
}

pub async fn remove_member(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path((conversation_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> HttpResult<StatusCode> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    runtime
        .service
        .remove_member(tenant_id, actor_id, conversation_id, target_user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_members(
    State(runtime): State<MessengerHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
) -> HttpResult<Json<Vec<MemberOutput>>> {
    let tenant_id = tenant.id;
    ensure_messenger_module_enabled(&runtime.db, tenant_id).await?;

    let actor_id = auth.user_id;

    let output = runtime
        .service
        .list_members(tenant_id, actor_id, conversation_id)
        .await?;

    Ok(Json(output))
}
