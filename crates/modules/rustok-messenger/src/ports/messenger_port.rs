use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::domain::{ConversationType, MessageType};
use crate::dto::{CreateConversationInput, SendMessageInput};
use crate::services::MessengerService;

/// Cross-module port trait for messaging capabilities.
#[async_trait]
pub trait MessengerPort: Send + Sync {
    /// Send an automated system message into an existing conversation.
    async fn send_system_message(
        &self,
        ctx: &PortContext,
        conversation_id: Uuid,
        content: String,
    ) -> Result<i64, PortError>;

    /// Send a direct message to a user, creating the direct dialogue if absent.
    async fn send_direct_message(
        &self,
        ctx: &PortContext,
        recipient_id: Uuid,
        content: String,
    ) -> Result<i64, PortError>;

    /// Create or link a contextual conversation thread to an external entity (CRM deal, order, task).
    async fn create_entity_thread(
        &self,
        ctx: &PortContext,
        entity_type: i16,
        entity_id: Uuid,
        title: Option<String>,
        participant_ids: Vec<Uuid>,
    ) -> Result<Uuid, PortError>;
}

/// Port adapter delegating to MessengerService.
pub struct MessengerPortAdapter {
    service: MessengerService,
}

impl MessengerPortAdapter {
    pub fn new(service: MessengerService) -> Self {
        Self { service }
    }

    fn parse_tenant_id(ctx: &PortContext) -> Result<Uuid, PortError> {
        Uuid::parse_str(&ctx.tenant_id)
            .map_err(|_| PortError::validation("invalid_tenant_id", "Invalid tenant UUID in PortContext"))
    }

    fn parse_actor_id(ctx: &PortContext) -> Result<Uuid, PortError> {
        Uuid::parse_str(&ctx.actor.id)
            .map_err(|_| PortError::forbidden("invalid_actor_id", "Valid actor UUID required in PortContext"))
    }
}

#[async_trait]
impl MessengerPort for MessengerPortAdapter {
    async fn send_system_message(
        &self,
        ctx: &PortContext,
        conversation_id: Uuid,
        content: String,
    ) -> Result<i64, PortError> {
        let tenant_id = Self::parse_tenant_id(ctx)?;
        let actor_id = Self::parse_actor_id(ctx).unwrap_or(Uuid::nil());

        let out = self
            .service
            .send_message(
                tenant_id,
                actor_id,
                SendMessageInput {
                    conversation_id,
                    message_type: Some(MessageType::System),
                    content,
                    blocks_json: None,
                    reply_to_id: None,
                    is_encrypted: Some(false),
                    encryption_algo: None,
                },
            )
            .await?;

        Ok(out.id)
    }

    async fn send_direct_message(
        &self,
        ctx: &PortContext,
        recipient_id: Uuid,
        content: String,
    ) -> Result<i64, PortError> {
        let tenant_id = Self::parse_tenant_id(ctx)?;
        let actor_id = Self::parse_actor_id(ctx)?;

        // 1. Create 1-on-1 direct dialogue
        let conv = self
            .service
            .create_conversation(
                tenant_id,
                actor_id,
                CreateConversationInput {
                    conversation_type: ConversationType::Direct,
                    title: None,
                    entity_type: None,
                    entity_id: None,
                    participant_ids: vec![recipient_id],
                    is_encrypted: Some(false),
                    initial_locale: None,
                    initial_channel_description: None,
                },
            )
            .await?;

        // 2. Dispatch message
        let out = self
            .service
            .send_message(
                tenant_id,
                actor_id,
                SendMessageInput {
                    conversation_id: conv.id,
                    message_type: Some(MessageType::Text),
                    content,
                    blocks_json: None,
                    reply_to_id: None,
                    is_encrypted: Some(false),
                    encryption_algo: None,
                },
            )
            .await?;

        Ok(out.id)
    }

    async fn create_entity_thread(
        &self,
        ctx: &PortContext,
        entity_type: i16,
        entity_id: Uuid,
        title: Option<String>,
        participant_ids: Vec<Uuid>,
    ) -> Result<Uuid, PortError> {
        let tenant_id = Self::parse_tenant_id(ctx)?;
        let actor_id = Self::parse_actor_id(ctx)?;

        let conv = self
            .service
            .create_conversation(
                tenant_id,
                actor_id,
                CreateConversationInput {
                    conversation_type: ConversationType::EntityThread,
                    title,
                    entity_type: Some(entity_type),
                    entity_id: Some(entity_id),
                    participant_ids,
                    is_encrypted: Some(false),
                    initial_locale: None,
                    initial_channel_description: None,
                },
            )
            .await?;

        Ok(conv.id)
    }
}
