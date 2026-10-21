use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{AuthContext, TenantContext};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::runtime_data::MessengerGraphqlRuntimeData;
use super::types::{
    ConversationItemGql, MessageItemGql, MessengerChannelListItemGql, MessengerE2eeDeviceAuditGql,
    MessengerOverviewGql,
};
use crate::controllers::ensure_messenger_module_enabled;
use crate::domain::ConversationType;
use crate::services::MessengerService;

/// Root GraphQL query entrypoint for the Messenger module.
#[derive(Default)]
pub struct MessengerQuery;

#[Object]
impl MessengerQuery {
    /// Retrieve aggregated metrics and status for the Messenger overview tab.
    async fn messenger_overview(&self, ctx: &Context<'_>) -> Result<MessengerOverviewGql> {
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx.data::<AuthContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(FieldError::new("Permission denied: tenant mismatch"));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        ensure_messenger_module_enabled(db, tenant.id)
            .await
            .map_err(|e| FieldError::new(e.message))?;

        let runtime_data = ctx.data_opt::<MessengerGraphqlRuntimeData>();
        let device_registry = runtime_data.map(|d| d.device_keys.clone());

        let service = MessengerService::new(db.clone(), 1);
        let service = if let Some(reg) = device_registry {
            service.with_device_keys(reg)
        } else {
            service
        };

        let (active_conversations_count, registered_devices_count) = service
            .get_overview_counts(tenant.id)
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        Ok(MessengerOverviewGql {
            active_conversations_count,
            stream_topic: "rustok-messenger-events".to_string(),
            consumer_group: "rustok-messenger-workers".to_string(),
            partitions_count: 16,
            registered_devices_count,
            matrix_server_name: "matrix.rustok.local".to_string(),
            federated_rooms_count: 0,
        })
    }

    /// List active channels with membership count and localized title for current tenant.
    async fn messenger_channels(&self, ctx: &Context<'_>) -> Result<Vec<MessengerChannelListItemGql>> {
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx.data::<AuthContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(FieldError::new("Permission denied: tenant mismatch"));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        ensure_messenger_module_enabled(db, tenant.id)
            .await
            .map_err(|e| FieldError::new(e.message))?;

        let service = MessengerService::new(db.clone(), 1);
        let items = service
            .list_channels_summary(tenant.id, 50, 0)
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        let channels = items
            .into_iter()
            .map(|(conv, title, topic, member_count)| {
                let conv_type = ConversationType::try_from(conv.conversation_type)
                    .unwrap_or(ConversationType::Channel)
                    .as_str()
                    .to_string();
                MessengerChannelListItemGql {
                    id: conv.id,
                    conversation_type: conv_type,
                    title,
                    topic,
                    member_count,
                    created_at: conv.created_at,
                }
            })
            .collect();

        Ok(channels)
    }

    /// List registered E2EE devices and their OTK pool depth for security audit.
    #[graphql(name = "messengerE2eeDevices")]
    async fn messenger_e2ee_devices(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<MessengerE2eeDeviceAuditGql>> {
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx.data::<AuthContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(FieldError::new("Permission denied: tenant mismatch"));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        ensure_messenger_module_enabled(db, tenant.id)
            .await
            .map_err(|e| FieldError::new(e.message))?;

        let runtime_data = ctx.data_opt::<MessengerGraphqlRuntimeData>();
        let device_registry = runtime_data.map(|d| d.device_keys.clone());

        let service = MessengerService::new(db.clone(), 1);
        let service = if let Some(reg) = device_registry {
            service.with_device_keys(reg)
        } else {
            service
        };

        let audits = service
            .device_keys()
            .list_all_devices_audit()
            .into_iter()
            .map(|(dev, otk_count)| MessengerE2eeDeviceAuditGql {
                device_id: dev.device_id,
                user_id: dev.user_id,
                has_curve25519: !dev.curve25519_key.is_empty(),
                has_ed25519: !dev.identity_key.is_empty(),
                otk_count,
            })
            .collect();

        Ok(audits)
    }

    /// List conversations accessible to the current authenticated user.
    async fn messenger_conversations(&self, ctx: &Context<'_>) -> Result<Vec<ConversationItemGql>> {
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx.data::<AuthContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(FieldError::new("Permission denied: tenant mismatch"));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        ensure_messenger_module_enabled(db, tenant.id)
            .await
            .map_err(|e| FieldError::new(e.message))?;

        let service = MessengerService::new(db.clone(), 1);
        let convs = service
            .list_conversations(tenant.id, auth.user_id, 100, 0)
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        let last_msg_ids: Vec<i64> = convs
            .iter()
            .filter_map(|c| c.last_message_id)
            .collect();

        let message_map: std::collections::HashMap<i64, String> = if last_msg_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            crate::entities::Message::find()
                .filter(crate::entities::message::Column::Id.is_in(last_msg_ids))
                .filter(crate::entities::message::Column::DeletedAt.is_null())
                .filter(crate::entities::message::Column::IsEncrypted.eq(false))
                .all(db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|m| (m.id, m.content))
                .collect()
        };

        let items = convs
            .into_iter()
            .map(|c| {
                let last_content = c.last_message_id.and_then(|id| message_map.get(&id).cloned());
                ConversationItemGql {
                    id: c.id,
                    conversation_type: c.conversation_type.as_str().to_string(),
                    title: c.title,
                    topic: None,
                    last_message_content: last_content,
                    last_message_at: c.last_message_at,
                    unread_count: c.unread_count as usize,
                    is_encrypted: c.is_encrypted,
                    entity_type: c.entity_type,
                    entity_id: c.entity_id,
                }
            })
            .collect();

        Ok(items)
    }

    /// List messages in a conversation with sequence-based cursor pagination.
    async fn conversation_messages(
        &self,
        ctx: &Context<'_>,
        conversation_id: Uuid,
        since_seq: Option<i64>,
        limit: Option<i32>,
    ) -> Result<Vec<MessageItemGql>> {
        let tenant = ctx.data::<TenantContext>()?;
        let auth = ctx.data::<AuthContext>()?;
        if auth.tenant_id != tenant.id {
            return Err(FieldError::new("Permission denied: tenant mismatch"));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        ensure_messenger_module_enabled(db, tenant.id)
            .await
            .map_err(|e| FieldError::new(e.message))?;

        let service = MessengerService::new(db.clone(), 1);
        let msgs = if let Some(seq) = since_seq {
            let sync_res = service
                .fast_sync(
                    tenant.id,
                    auth.user_id,
                    crate::dto::FastSyncRequest {
                        conversation_id,
                        since_seq: seq,
                        limit: limit.map(|l| l.max(1) as u64),
                    },
                )
                .await
                .map_err(|e| FieldError::new(e.to_string()))?;
            sync_res.messages
        } else {
            service
                .list_messages(
                    tenant.id,
                    auth.user_id,
                    crate::dto::ListMessagesQuery {
                        conversation_id,
                        limit: limit.map(|l| l.max(1) as u64),
                        before_id: None,
                        after_id: None,
                    },
                )
                .await
                .map_err(|e| FieldError::new(e.to_string()))?
        };

        let result = msgs
            .into_iter()
            .map(|m| MessageItemGql {
                id: m.id,
                conversation_id: m.conversation_id,
                sender_id: m.sender_id,
                sender_name: None,
                message_type: m.message_type as i16,
                content: m.content,
                blocks_json: m.blocks_json.map(async_graphql::Json),
                reply_to_id: m.reply_to_id,
                is_encrypted: m.is_encrypted,
                created_at: m.created_at,
            })
            .collect();

        Ok(result)
    }
}
