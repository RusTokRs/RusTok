use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use super::require_auth_context;
use super::runtime_data::MessengerGraphqlRuntimeData;
use super::types::{
    CreateMessengerChannelGqlInput, DeviceKeyGql, MessageItemGql, MessengerChannelListItemGql,
    OneTimePrekeyGql, PrekeyBundleGql, SendMessageInputGql, UpdateCursorInputGql,
    UpdateCursorOutputGql, UploadDeviceKeysGqlInput,
};
use crate::domain::ConversationType;
use crate::dto::CreateConversationInput;
use crate::e2ee::{OneTimePrekeyInput, UploadDeviceKeysInput};
use crate::services::MessengerService;

/// Root GraphQL mutation entrypoint for the Messenger module.
#[derive(Default)]
pub struct MessengerMutation;

#[Object]
impl MessengerMutation {
    /// Create a new channel with localized metadata.
    async fn create_messenger_channel(
        &self,
        ctx: &Context<'_>,
        input: CreateMessengerChannelGqlInput,
    ) -> Result<MessengerChannelListItemGql> {
        let (tenant, auth, db) = require_auth_context(ctx).await?;

        let trimmed_title = input.title.trim();
        if trimmed_title.is_empty() {
            return Err(FieldError::new("Channel title cannot be empty"));
        }

        let conv_type = match input.conversation_type.trim().to_lowercase().as_str() {
            "channel" => ConversationType::Channel,
            "group" => ConversationType::Group,
            "direct" => ConversationType::Direct,
            "topic" | "thread" => ConversationType::EntityThread,
            other => {
                return Err(FieldError::new(format!(
                    "Unsupported conversation type: '{other}'"
                )));
            }
        };

        let service = MessengerService::new(db.clone(), 1);
        let output = service
            .create_conversation(
                tenant.id,
                auth.user_id,
                CreateConversationInput {
                    conversation_type: conv_type,
                    title: Some(trimmed_title.to_string()),
                    entity_type: None,
                    entity_id: None,
                    participant_ids: vec![],
                    is_encrypted: Some(false),
                    initial_locale: input.locale,
                    initial_channel_description: input.topic.clone(),
                },
            )
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        Ok(MessengerChannelListItemGql {
            id: output.id,
            conversation_type: output.conversation_type.as_str().to_string(),
            title: output.title,
            topic: input.topic,
            member_count: 1,
            created_at: output.created_at,
        })
    }

    /// Register/update user's device cryptographic identity and upload one-time prekeys.
    async fn upload_device_keys(
        &self,
        ctx: &Context<'_>,
        input: UploadDeviceKeysGqlInput,
    ) -> Result<DeviceKeyGql> {
        let (_tenant, auth, _db) = require_auth_context(ctx).await?;

        let runtime_data = ctx.data_opt::<MessengerGraphqlRuntimeData>();
        let device_registry = runtime_data
            .map(|d| d.device_keys.clone())
            .unwrap_or_default();

        let device_key = device_registry
            .register_device_keys(
                auth.user_id,
                UploadDeviceKeysInput {
                    device_id: input.device_id,
                    identity_key: input.identity_key,
                    curve25519_key: input.curve25519_key,
                    signature: input.signature,
                    one_time_prekeys: input
                        .one_time_prekeys
                        .into_iter()
                        .map(|otk| OneTimePrekeyInput {
                            key_id: otk.key_id,
                            key_material: otk.key_material,
                            signature: otk.signature,
                        })
                        .collect(),
                },
            )
            .map_err(|e| FieldError::new(e.to_string()))?;

        Ok(DeviceKeyGql {
            device_id: device_key.device_id,
            user_id: device_key.user_id,
            identity_key: device_key.identity_key,
            curve25519_key: device_key.curve25519_key,
            signature: device_key.signature,
            created_at: device_key.created_at,
        })
    }

    /// Atomically claim an unconsumed one-time prekey bundle for a target user device.
    async fn claim_prekey_bundle(
        &self,
        ctx: &Context<'_>,
        target_user_id: Uuid,
        device_id: String,
    ) -> Result<PrekeyBundleGql> {
        let (_tenant, _auth, _db) = require_auth_context(ctx).await?;

        if target_user_id.is_nil() {
            return Err(FieldError::new("target_user_id must not be nil"));
        }
        let trimmed_device = device_id.trim();
        if trimmed_device.is_empty() {
            return Err(FieldError::new("device_id must not be empty"));
        }

        let runtime_data = ctx.data_opt::<MessengerGraphqlRuntimeData>();
        let device_registry = runtime_data
            .map(|d| d.device_keys.clone())
            .unwrap_or_default();

        let bundle = device_registry
            .claim_prekey_bundle(target_user_id, trimmed_device)
            .map_err(|e| FieldError::new(e.to_string()))?;

        Ok(PrekeyBundleGql {
            user_id: bundle.user_id,
            device_id: bundle.device_id,
            identity_key: bundle.identity_key,
            curve25519_key: bundle.curve25519_key,
            one_time_prekey: bundle.one_time_prekey.map(|otk| OneTimePrekeyGql {
                key_id: otk.key_id,
                device_id: otk.device_id,
                key_material: otk.key_material,
                signature: otk.signature,
                consumed: otk.consumed,
            }),
        })
    }

    /// Send a message into an active conversation.
    async fn send_chat_message(
        &self,
        ctx: &Context<'_>,
        input: SendMessageInputGql,
    ) -> Result<MessageItemGql> {
        let (tenant, auth, db) = require_auth_context(ctx).await?;

        if input.conversation_id.is_nil() {
            return Err(FieldError::new("conversation_id cannot be nil UUID"));
        }

        let service = MessengerService::new(db.clone(), 1);
        let out = service
            .send_message(
                tenant.id,
                auth.user_id,
                crate::dto::SendMessageInput {
                    conversation_id: input.conversation_id,
                    message_type: None,
                    content: input.content,
                    blocks_json: None,
                    reply_to_id: input.reply_to_id,
                    is_encrypted: None,
                    encryption_algo: None,
                },
            )
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        if let Some(rd) = ctx.data_opt::<MessengerGraphqlRuntimeData>() {
            rd.room_hub.broadcast(
                tenant.id,
                out.conversation_id,
                crate::controllers::MessengerServerMessage::NewMessage(out.clone()),
            );
        }

        Ok(MessageItemGql {
            id: out.id,
            conversation_id: out.conversation_id,
            sender_id: out.sender_id,
            sender_name: None,
            message_type: out.message_type as i16,
            content: out.content,
            blocks_json: out.blocks_json.map(async_graphql::Json),
            reply_to_id: out.reply_to_id,
            is_encrypted: out.is_encrypted,
            created_at: out.created_at,
        })
    }

    /// Monotonically advance user read cursor.
    async fn update_read_cursor(
        &self,
        ctx: &Context<'_>,
        input: UpdateCursorInputGql,
    ) -> Result<UpdateCursorOutputGql> {
        let (tenant, auth, db) = require_auth_context(ctx).await?;

        if input.conversation_id.is_nil() {
            return Err(FieldError::new("conversation_id cannot be nil UUID"));
        }
        if input.last_read_message_id < 0 {
            return Err(FieldError::new("last_read_message_id cannot be negative"));
        }

        let service = MessengerService::new(db.clone(), 1);
        let out = service
            .update_cursor(
                tenant.id,
                auth.user_id,
                crate::dto::UpdateCursorInput {
                    conversation_id: input.conversation_id,
                    last_read_message_id: input.last_read_message_id,
                },
            )
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        if let Some(rd) = ctx.data_opt::<MessengerGraphqlRuntimeData>() {
            rd.room_hub.broadcast(
                tenant.id,
                out.conversation_id,
                crate::controllers::MessengerServerMessage::CursorAdvanced(out.clone()),
            );
        }

        Ok(UpdateCursorOutputGql {
            conversation_id: out.conversation_id,
            last_read_message_id: out.last_read_message_id,
        })
    }

    /// Edit an existing message content.
    async fn edit_chat_message(
        &self,
        ctx: &Context<'_>,
        message_id: i64,
        content: String,
    ) -> Result<MessageItemGql> {
        let (tenant, auth, db) = require_auth_context(ctx).await?;

        if message_id <= 0 {
            return Err(FieldError::new("message_id must be a positive Snowflake ID"));
        }
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(FieldError::new("content cannot be empty"));
        }

        let service = MessengerService::new(db.clone(), 1);
        let out = service
            .edit_message(
                tenant.id,
                auth.user_id,
                message_id,
                crate::dto::EditMessageInput { content },
            )
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        if let Some(rd) = ctx.data_opt::<MessengerGraphqlRuntimeData>() {
            rd.room_hub.broadcast(
                tenant.id,
                out.conversation_id,
                crate::controllers::MessengerServerMessage::MessageEdited(out.clone()),
            );
        }

        Ok(MessageItemGql {
            id: out.id,
            conversation_id: out.conversation_id,
            sender_id: out.sender_id,
            sender_name: None,
            message_type: out.message_type as i16,
            content: out.content,
            blocks_json: out.blocks_json.map(async_graphql::Json),
            reply_to_id: out.reply_to_id,
            is_encrypted: out.is_encrypted,
            created_at: out.created_at,
        })
    }

    /// Soft-delete a message, preserving sequence monotonicity.
    async fn delete_chat_message(
        &self,
        ctx: &Context<'_>,
        message_id: i64,
    ) -> Result<bool> {
        let (tenant, auth, db) = require_auth_context(ctx).await?;

        if message_id <= 0 {
            return Err(FieldError::new("message_id must be a positive Snowflake ID"));
        }

        let service = MessengerService::new(db.clone(), 1);
        let conv_id = service
            .delete_message(tenant.id, auth.user_id, message_id)
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        if let Some(rd) = ctx.data_opt::<MessengerGraphqlRuntimeData>() {
            rd.room_hub.broadcast(
                tenant.id,
                conv_id,
                crate::controllers::MessengerServerMessage::MessageDeleted { message_id },
            );
        }

        Ok(true)
    }
}
