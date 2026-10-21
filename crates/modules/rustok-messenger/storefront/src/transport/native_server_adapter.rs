#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use crate::model::*;

#[cfg(feature = "ssr")]
pub async fn fetch_conversations(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<Vec<ConversationItemPayload>, String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1);

    let convs = service
        .list_conversations(tenant.id, auth.user_id, 100, 0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(convs
        .into_iter()
        .map(|c| ConversationItemPayload {
            id: c.id,
            conversation_type: format!("{:?}", c.conversation_type).to_lowercase(),
            title: c.title,
            topic: None,
            last_message_content: None,
            last_message_at: c.last_message_at,
            unread_count: c.unread_count as usize,
            is_encrypted: c.is_encrypted,
            entity_type: c.entity_type,
            entity_id: c.entity_id,
        })
        .collect())
}

#[cfg(feature = "ssr")]
pub async fn fetch_messages(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    conversation_id: Uuid,
    since_seq: Option<i64>,
    limit: Option<i32>,
) -> Result<Vec<MessageItemPayload>, String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::dto::ListMessagesQuery;
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1);

    let messages = service
        .list_messages(
            tenant.id,
            auth.user_id,
            ListMessagesQuery {
                conversation_id,
                limit: limit.map(|l| l as u64),
                before_id: None,
                after_id: since_seq,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(messages
        .into_iter()
        .map(|m| MessageItemPayload {
            id: m.id,
            conversation_id: m.conversation_id,
            sender_id: m.sender_id,
            sender_name: None,
            message_type: m.message_type as i16,
            content: m.content,
            blocks_json: m.blocks_json,
            reply_to_id: m.reply_to_id,
            is_encrypted: m.is_encrypted,
            created_at: m.created_at,
        })
        .collect())
}

#[cfg(feature = "ssr")]
pub async fn send_message(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    input: SendMessagePayload,
) -> Result<MessageItemPayload, String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::dto::SendMessageInput;
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1);

    let out = service
        .send_message(
            tenant.id,
            auth.user_id,
            SendMessageInput {
                conversation_id: input.conversation_id,
                message_type: None,
                content: input.content,
                blocks_json: None,
                reply_to_id: input.reply_to_id,
                is_encrypted: Some(false),
                encryption_algo: None,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(MessageItemPayload {
        id: out.id,
        conversation_id: out.conversation_id,
        sender_id: out.sender_id,
        sender_name: Some("You".into()),
        message_type: out.message_type as i16,
        content: out.content,
        blocks_json: out.blocks_json,
        reply_to_id: out.reply_to_id,
        is_encrypted: out.is_encrypted,
        created_at: out.created_at,
    })
}

#[cfg(feature = "ssr")]
pub async fn update_cursor(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    input: UpdateCursorPayload,
) -> Result<(), String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::dto::UpdateCursorInput;
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1);

    service
        .update_cursor(
            tenant.id,
            auth.user_id,
            UpdateCursorInput {
                conversation_id: input.conversation_id,
                last_read_message_id: input.last_read_message_id,
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
