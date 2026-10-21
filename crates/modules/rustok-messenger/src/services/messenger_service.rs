use std::sync::Arc;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::domain::{ConversationType, MemberRole, MessageType, SnowflakeGenerator};
use crate::dto::{
    AddMemberInput, ChannelTranslationOutput, ConversationOutput, CreateConversationInput,
    EditMessageInput, FastSyncRequest, FastSyncResponse, ListMessagesQuery, MemberOutput,
    MessageOutput, SendMessageInput, UpdateChannelTranslationInput, UpdateCursorInput,
};
use crate::entities::{
    channel_translation, conversation, cursor, member, message,
    ChannelTranslation, Conversation, Cursor, Member, Message,
};
use crate::error::{MessengerError, MessengerResult};

/// Maximum message payload size (64 KiB).
pub const MAX_MESSAGE_CONTENT_SIZE: usize = 65_536;

/// Default page limit for message queries.
pub const DEFAULT_MESSAGE_PAGE_LIMIT: u64 = 50;

/// Application service owning messaging business transactions.
#[derive(Clone)]
pub struct MessengerService {
    db: DatabaseConnection,
    snowflake: Arc<SnowflakeGenerator>,
    device_keys: Arc<crate::e2ee::DeviceKeyRegistry>,
}

impl MessengerService {
    pub fn new(db: DatabaseConnection, worker_id: i64) -> Self {
        Self {
            db,
            snowflake: Arc::new(SnowflakeGenerator::new(worker_id)),
            device_keys: Arc::new(crate::e2ee::DeviceKeyRegistry::new()),
        }
    }

    pub fn with_device_keys(mut self, device_keys: Arc<crate::e2ee::DeviceKeyRegistry>) -> Self {
        self.device_keys = device_keys;
        self
    }

    pub fn device_keys(&self) -> &Arc<crate::e2ee::DeviceKeyRegistry> {
        &self.device_keys
    }

    /// Retrieve active conversation count and registered E2EE device count for a tenant.
    pub async fn get_overview_counts(&self, tenant_id: Uuid) -> MessengerResult<(usize, usize)> {
        let conv_count = Conversation::find()
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .count(&self.db)
            .await? as usize;
        let device_count = self.device_keys.total_devices_count();
        Ok((conv_count, device_count))
    }

    /// Lists channels for administrative inspection including title fallback and member counts.
    /// Excludes private 1-on-1 direct dialogues to protect user privacy.
    pub async fn list_channels_summary(
        &self,
        tenant_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> MessengerResult<Vec<(conversation::Model, Option<String>, Option<String>, usize)>> {
        let convs = Conversation::find()
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .filter(conversation::Column::ConversationType.ne(ConversationType::Direct as i16))
            .order_by_desc(conversation::Column::CreatedAt)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;

        if convs.is_empty() {
            return Ok(Vec::new());
        }

        let conv_ids: Vec<Uuid> = convs.iter().map(|c| c.id).collect();

        // Batch load translations and member counts in 2 targeted queries (no N+1)
        let all_translations: Vec<channel_translation::Model> = ChannelTranslation::find()
            .filter(channel_translation::Column::ChannelId.is_in(conv_ids.clone()))
            .all(&self.db)
            .await?;

        let mut translations_by_conv: std::collections::HashMap<Uuid, Vec<channel_translation::Model>> =
            std::collections::HashMap::new();
        for t in all_translations {
            translations_by_conv.entry(t.channel_id).or_default().push(t);
        }

        let all_members: Vec<member::Model> = Member::find()
            .filter(member::Column::ConversationId.is_in(conv_ids))
            .all(&self.db)
            .await?;

        let mut member_counts: std::collections::HashMap<Uuid, usize> =
            std::collections::HashMap::new();
        for m in all_members {
            *member_counts.entry(m.conversation_id).or_default() += 1;
        }

        let mut results = Vec::with_capacity(convs.len());
        for c in convs {
            let translation = translations_by_conv
                .get(&c.id)
                .and_then(|list| list.first());

            let title = translation
                .as_ref()
                .map(|t| t.title.clone())
                .or_else(|| c.title.clone());
            let topic = translation.and_then(|t| t.description.clone());
            let member_count = member_counts.get(&c.id).copied().unwrap_or(0);

            results.push((c, title, topic, member_count));
        }

        Ok(results)
    }

    /// Create a new conversation, channel, or direct message dialogue atomically.
    pub async fn create_conversation(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateConversationInput,
    ) -> MessengerResult<ConversationOutput> {
        let conv_id = Uuid::now_v7();
        let now = Utc::now();

        let conv_type = input.conversation_type as i16;
        let is_encrypted = input.is_encrypted.unwrap_or(false);

        if let Some(locale) = input.initial_locale.as_deref().filter(|l| l.len() > 32) {
            return Err(MessengerError::InvalidLocale {
                locale: locale.to_string(),
            });
        }

        let txn = self.db.begin().await?;

        let conv_model = conversation::ActiveModel {
            id: Set(conv_id),
            tenant_id: Set(tenant_id),
            conversation_type: Set(conv_type),
            title: Set(input.title.clone()),
            entity_type: Set(input.entity_type),
            entity_id: Set(input.entity_id),
            is_encrypted: Set(is_encrypted),
            created_by: Set(actor_id),
            last_message_id: Set(None),
            last_message_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        conv_model.insert(&txn).await?;

        // 1. If localized channel translation provided, persist it
        if let Some(locale) = input.initial_locale {
            let title = input.title.clone().unwrap_or_else(|| "Channel".to_string());
            let trans_model = channel_translation::ActiveModel {
                id: Set(Uuid::now_v7()),
                channel_id: Set(conv_id),
                locale: Set(locale),
                title: Set(title),
                description: Set(input.initial_channel_description),
                created_at: Set(now),
                updated_at: Set(now),
            };
            trans_model.insert(&txn).await?;
        }

        // 2. Add creator as Owner
        let creator_member = member::ActiveModel {
            id: Set(Uuid::now_v7()),
            conversation_id: Set(conv_id),
            user_id: Set(actor_id),
            role: Set(MemberRole::Owner as i16),
            joined_at: Set(now),
            muted_until: Set(None),
        };
        creator_member.insert(&txn).await?;

        // 3. Add other invited participants
        let mut seen = std::collections::HashSet::new();
        seen.insert(actor_id);
        for participant_id in input.participant_ids {
            if seen.insert(participant_id) {
                let p_member = member::ActiveModel {
                    id: Set(Uuid::now_v7()),
                    conversation_id: Set(conv_id),
                    user_id: Set(participant_id),
                    role: Set(MemberRole::Member as i16),
                    joined_at: Set(now),
                    muted_until: Set(None),
                };
                p_member.insert(&txn).await?;
            }
        }

        // 4. Initialize read cursor for creator
        let initial_cursor = cursor::ActiveModel {
            id: Set(Uuid::now_v7()),
            conversation_id: Set(conv_id),
            user_id: Set(actor_id),
            last_read_message_id: Set(0),
            last_read_at: Set(now),
        };
        initial_cursor.insert(&txn).await?;

        txn.commit().await?;

        Ok(ConversationOutput {
            id: conv_id,
            tenant_id,
            conversation_type: input.conversation_type,
            title: input.title,
            entity_type: input.entity_type,
            entity_id: input.entity_id,
            is_encrypted,
            created_by: actor_id,
            last_message_id: None,
            last_message_at: None,
            created_at: now,
            unread_count: 0,
        })
    }

    /// Retrieve conversation metadata by ID with effective localized title.
    pub async fn get_conversation(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        conversation_id: Uuid,
        locale: Option<&str>,
    ) -> MessengerResult<ConversationOutput> {
        let conv = Conversation::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationNotFound(conversation_id))?;

        self.ensure_membership(conversation_id, actor_id).await?;

        // Resolve title with localization fallback
        let mut title = conv.title.clone();
        if let Some(req_locale) = locale {
            let trans = ChannelTranslation::find()
                .filter(channel_translation::Column::ChannelId.eq(conversation_id))
                .filter(channel_translation::Column::Locale.eq(req_locale))
                .one(&self.db)
                .await?;
            if let Some(trans) = trans {
                title = Some(trans.title);
            }
        }

        let unread_count = self.calculate_unread_count(conversation_id, actor_id).await?;

        let conv_type = ConversationType::try_from(conv.conversation_type)
            .map_err(|_| MessengerError::Internal("Corrupted conversation_type in DB".into()))?;

        Ok(ConversationOutput {
            id: conv.id,
            tenant_id: conv.tenant_id,
            conversation_type: conv_type,
            title,
            entity_type: conv.entity_type,
            entity_id: conv.entity_id,
            is_encrypted: conv.is_encrypted,
            created_by: conv.created_by,
            last_message_id: conv.last_message_id,
            last_message_at: conv.last_message_at,
            created_at: conv.created_at,
            unread_count,
        })
    }

    /// List conversations where actor is an active participant.
    pub async fn list_conversations(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> MessengerResult<Vec<ConversationOutput>> {
        let member_conv_ids: Vec<Uuid> = Member::find()
            .filter(member::Column::UserId.eq(actor_id))
            .select_only()
            .column(member::Column::ConversationId)
            .into_tuple()
            .all(&self.db)
            .await?;

        if member_conv_ids.is_empty() {
            return Ok(vec![]);
        }

        let convs = Conversation::find()
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .filter(conversation::Column::Id.is_in(member_conv_ids))
            .order_by_desc(conversation::Column::UpdatedAt)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;

        let mut outputs = Vec::with_capacity(convs.len());
        for c in convs {
            let conv_type = ConversationType::try_from(c.conversation_type).unwrap_or(ConversationType::Group);
            let unread_count = self.calculate_unread_count(c.id, actor_id).await.unwrap_or(0);
            outputs.push(ConversationOutput {
                id: c.id,
                tenant_id: c.tenant_id,
                conversation_type: conv_type,
                title: c.title,
                entity_type: c.entity_type,
                entity_id: c.entity_id,
                is_encrypted: c.is_encrypted,
                created_by: c.created_by,
                last_message_id: c.last_message_id,
                last_message_at: c.last_message_at,
                created_at: c.created_at,
                unread_count,
            });
        }

        Ok(outputs)
    }

    /// Update or create localized channel translation (title, description).
    pub async fn update_channel_translation(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: UpdateChannelTranslationInput,
    ) -> MessengerResult<ChannelTranslationOutput> {
        let conv = Conversation::find_by_id(input.channel_id)
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationNotFound(input.channel_id))?;

        let member_record = Member::find()
            .filter(member::Column::ConversationId.eq(input.channel_id))
            .filter(member::Column::UserId.eq(actor_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationAccessDenied(input.channel_id))?;

        if member_record.role != MemberRole::Owner as i16 && member_record.role != MemberRole::Admin as i16 {
            return Err(MessengerError::ConversationAccessDenied(input.channel_id));
        }

        if input.locale.len() > 32 {
            return Err(MessengerError::InvalidLocale {
                locale: input.locale,
            });
        }

        let now = Utc::now();
        let existing = ChannelTranslation::find()
            .filter(channel_translation::Column::ChannelId.eq(input.channel_id))
            .filter(channel_translation::Column::Locale.eq(&input.locale))
            .one(&self.db)
            .await?;

        if let Some(model) = existing {
            let mut active: channel_translation::ActiveModel = model.into();
            active.title = Set(input.title.clone());
            active.description = Set(input.description.clone());
            active.updated_at = Set(now);
            active.update(&self.db).await?;
        } else {
            let new_model = channel_translation::ActiveModel {
                id: Set(Uuid::now_v7()),
                channel_id: Set(conv.id),
                locale: Set(input.locale.clone()),
                title: Set(input.title.clone()),
                description: Set(input.description.clone()),
                created_at: Set(now),
                updated_at: Set(now),
            };
            new_model.insert(&self.db).await?;
        }

        Ok(ChannelTranslationOutput {
            channel_id: input.channel_id,
            locale: input.locale,
            title: input.title,
            description: input.description,
            updated_at: now,
        })
    }

    /// Send a message with append-only monotonicity.
    pub async fn send_message(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: SendMessageInput,
    ) -> MessengerResult<MessageOutput> {
        let content_len = input.content.len();
        if content_len > MAX_MESSAGE_CONTENT_SIZE {
            return Err(MessengerError::PayloadTooLarge {
                actual: content_len,
                max: MAX_MESSAGE_CONTENT_SIZE,
            });
        }
        if input.content.trim().is_empty() && input.blocks_json.is_none() {
            return Err(MessengerError::Validation(
                "Message content cannot be empty".to_string(),
            ));
        }

        let conv = Conversation::find_by_id(input.conversation_id)
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationNotFound(input.conversation_id))?;

        self.ensure_active_member(conv.id, actor_id).await?;

        // Enforce E2EE invariants: encrypted conversation requires encrypted messages
        let is_encrypted = input.is_encrypted.unwrap_or(conv.is_encrypted);
        if conv.is_encrypted && !is_encrypted {
            return Err(MessengerError::Validation(
                "Encrypted conversation requires encrypted messages".to_string(),
            ));
        }

        let encryption_algo = if is_encrypted {
            let algo = input.encryption_algo.unwrap_or_else(|| {
                if conv.conversation_type == ConversationType::Direct as i16 {
                    "m.olm.v1.curve25519-aes-sha2".to_string()
                } else {
                    "m.megolm.v1.aes-sha2".to_string()
                }
            });
            if algo != "m.olm.v1.curve25519-aes-sha2" && algo != "m.megolm.v1.aes-sha2" {
                return Err(MessengerError::Validation(format!(
                    "Unsupported encryption algorithm: {algo}"
                )));
            }
            Some(algo)
        } else {
            None
        };

        // Generate 64-bit strictly monotonic Snowflake sequence ID
        let message_id = self.snowflake.next_id();
        let now = Utc::now();

        let msg_type = input.message_type.unwrap_or(MessageType::Text) as i16;

        let txn = self.db.begin().await?;

        let msg_model = message::ActiveModel {
            id: Set(message_id),
            tenant_id: Set(tenant_id),
            conversation_id: Set(conv.id),
            sender_id: Set(actor_id),
            message_type: Set(msg_type),
            content: Set(input.content.clone()),
            blocks_json: Set(input.blocks_json.clone()),
            is_encrypted: Set(is_encrypted),
            encryption_algo: Set(encryption_algo.clone()),
            reply_to_id: Set(input.reply_to_id),
            created_at: Set(now),
            deleted_at: Set(None),
        };

        msg_model.insert(&txn).await?;

        // Update conversation's latest message pointer
        let mut conv_active: conversation::ActiveModel = conv.into();
        conv_active.last_message_id = Set(Some(message_id));
        conv_active.last_message_at = Set(Some(now));
        conv_active.updated_at = Set(now);
        conv_active.update(&txn).await?;

        // Monotonically advance sender's cursor automatically within the same transaction
        Self::update_cursor_in_tx(&txn, input.conversation_id, actor_id, message_id).await?;

        // Emit transactional outbox event for notifications & streaming fanout
        rustok_outbox::TransactionalEventBus::publish_root_in_tx(
            &txn,
            tenant_id,
            Some(actor_id),
            rustok_events::DomainEvent::MessengerMessageDispatched {
                conversation_id: input.conversation_id,
                message_id,
                sender_id: actor_id,
            },
        )
        .await
        .map_err(|e| MessengerError::Internal(format!("Failed to emit outbox event: {e}")))?;

        txn.commit().await?;

        let res_type = MessageType::try_from(msg_type).unwrap_or(MessageType::Text);

        Ok(MessageOutput {
            id: message_id,
            tenant_id,
            conversation_id: input.conversation_id,
            sender_id: actor_id,
            message_type: res_type,
            content: input.content,
            blocks_json: input.blocks_json,
            is_encrypted,
            encryption_algo,
            reply_to_id: input.reply_to_id,
            created_at: now,
            is_deleted: false,
        })
    }

    /// List messages in a conversation with sequence-based cursor pagination.
    pub async fn list_messages(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        query: ListMessagesQuery,
    ) -> MessengerResult<Vec<MessageOutput>> {
        self.ensure_membership(query.conversation_id, actor_id).await?;

        let limit = query.limit.unwrap_or(DEFAULT_MESSAGE_PAGE_LIMIT).min(100);

        let mut select = Message::find()
            .filter(message::Column::TenantId.eq(tenant_id))
            .filter(message::Column::ConversationId.eq(query.conversation_id));

        if let Some(before) = query.before_id {
            select = select.filter(message::Column::Id.lt(before));
        }
        if let Some(after) = query.after_id {
            select = select.filter(message::Column::Id.gt(after));
        }

        let msgs = select
            .order_by_desc(message::Column::Id)
            .limit(limit)
            .all(&self.db)
            .await?;

        Ok(msgs.into_iter().map(Self::to_message_output).collect())
    }

    fn to_message_output(m: message::Model) -> MessageOutput {
        let m_type = MessageType::try_from(m.message_type).unwrap_or(MessageType::Text);
        MessageOutput {
            id: m.id,
            tenant_id: m.tenant_id,
            conversation_id: m.conversation_id,
            sender_id: m.sender_id,
            message_type: m_type,
            content: m.content,
            blocks_json: m.blocks_json,
            is_encrypted: m.is_encrypted,
            encryption_algo: m.encryption_algo,
            reply_to_id: m.reply_to_id,
            created_at: m.created_at,
            is_deleted: m.deleted_at.is_some(),
        }
    }

    /// FastSync delta replay for offline or reconnecting clients.
    pub async fn fast_sync(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        request: FastSyncRequest,
    ) -> MessengerResult<FastSyncResponse> {
        self.ensure_membership(request.conversation_id, actor_id).await?;

        let limit = request.limit.unwrap_or(100).min(500);

        // Fetch messages strictly greater than client's checkpoint cursor in ascending sequence order
        let msgs = Message::find()
            .filter(message::Column::TenantId.eq(tenant_id))
            .filter(message::Column::ConversationId.eq(request.conversation_id))
            .filter(message::Column::Id.gt(request.since_seq))
            .order_by_asc(message::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;

        let has_more = msgs.len() > limit as usize;
        let result_msgs: Vec<_> = msgs.into_iter().take(limit as usize).collect();

        let latest_seq = result_msgs
            .last()
            .map(|m| m.id)
            .unwrap_or(request.since_seq);

        let outputs: Vec<MessageOutput> = result_msgs
            .into_iter()
            .map(|m| {
                let m_type = MessageType::try_from(m.message_type).unwrap_or(MessageType::Text);
                MessageOutput {
                    id: m.id,
                    tenant_id: m.tenant_id,
                    conversation_id: m.conversation_id,
                    sender_id: m.sender_id,
                    message_type: m_type,
                    content: m.content,
                    blocks_json: m.blocks_json,
                    is_encrypted: m.is_encrypted,
                    encryption_algo: m.encryption_algo,
                    reply_to_id: m.reply_to_id,
                    created_at: m.created_at,
                    is_deleted: m.deleted_at.is_some(),
                }
            })
            .collect();

        Ok(FastSyncResponse {
            conversation_id: request.conversation_id,
            messages: outputs,
            has_more,
            latest_seq,
        })
    }

    /// Edit an existing message's text content.
    /// In accordance with Zero-Knowledge E2EE, encrypted messages cannot be edited into plaintext.
    pub async fn edit_message(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        message_id: i64,
        input: EditMessageInput,
    ) -> MessengerResult<MessageOutput> {
        let content_len = input.content.len();
        if content_len > MAX_MESSAGE_CONTENT_SIZE {
            return Err(MessengerError::PayloadTooLarge {
                actual: content_len,
                max: MAX_MESSAGE_CONTENT_SIZE,
            });
        }
        if input.content.trim().is_empty() {
            return Err(MessengerError::Validation(
                "Message content cannot be empty".to_string(),
            ));
        }

        let msg = Message::find_by_id(message_id)
            .filter(message::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::MessageNotFound(message_id))?;

        if msg.deleted_at.is_some() {
            return Err(MessengerError::Validation(
                "Cannot edit deleted message".to_string(),
            ));
        }

        self.ensure_active_member(msg.conversation_id, actor_id).await?;

        if msg.sender_id != actor_id {
            return Err(MessengerError::ConversationAccessDenied(msg.conversation_id));
        }

        if msg.is_encrypted {
            return Err(MessengerError::Validation(
                "Plaintext editing of end-to-end encrypted message is not permitted".to_string(),
            ));
        }

        let mut active: message::ActiveModel = msg.into();
        active.content = Set(input.content);
        let updated = active.update(&self.db).await?;

        Ok(Self::to_message_output(updated))
    }

    /// Soft-delete a message, preserving sequence monotonicity.
    /// Returns the conversation_id of the deleted message.
    pub async fn delete_message(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        message_id: i64,
    ) -> MessengerResult<Uuid> {
        let msg = Message::find_by_id(message_id)
            .filter(message::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::MessageNotFound(message_id))?;

        if msg.deleted_at.is_some() {
            return Ok(msg.conversation_id); // Idempotent
        }

        self.ensure_membership(msg.conversation_id, actor_id).await?;

        // Author can delete their own message. Channel owner/admin can also delete messages.
        if msg.sender_id != actor_id {
            let conv = Conversation::find_by_id(msg.conversation_id)
                .filter(conversation::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await?
                .ok_or(MessengerError::ConversationNotFound(msg.conversation_id))?;

            // In 1-on-1 direct dialogues, counterparties cannot delete the other party's messages
            if conv.conversation_type == ConversationType::Direct as i16 {
                return Err(MessengerError::ConversationAccessDenied(msg.conversation_id));
            }

            let member_record = Member::find()
                .filter(member::Column::ConversationId.eq(msg.conversation_id))
                .filter(member::Column::UserId.eq(actor_id))
                .one(&self.db)
                .await?
                .ok_or(MessengerError::ConversationAccessDenied(msg.conversation_id))?;

            if member_record.role != MemberRole::Owner as i16 && member_record.role != MemberRole::Admin as i16 {
                return Err(MessengerError::ConversationAccessDenied(msg.conversation_id));
            }
        }

        let mut active: message::ActiveModel = msg.into();
        active.deleted_at = Set(Some(Utc::now()));
        let updated = active.update(&self.db).await?;

        Ok(updated.conversation_id)
    }

    /// Add a member to a conversation (group or channel).
    pub async fn add_member(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        conversation_id: Uuid,
        input: AddMemberInput,
    ) -> MessengerResult<MemberOutput> {
        let conv = Conversation::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationNotFound(conversation_id))?;

        if conv.conversation_type == ConversationType::Direct as i16 {
            return Err(MessengerError::Validation(
                "Cannot add members to a 1-on-1 direct dialogue".to_string(),
            ));
        }

        let actor_member = Member::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(member::Column::UserId.eq(actor_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationAccessDenied(conversation_id))?;

        if actor_member.role != MemberRole::Owner as i16 && actor_member.role != MemberRole::Admin as i16 {
            return Err(MessengerError::ConversationAccessDenied(conversation_id));
        }

        let role = input.role.unwrap_or(MemberRole::Member);
        if role == MemberRole::Owner && actor_member.role != MemberRole::Owner as i16 {
            return Err(MessengerError::ConversationAccessDenied(conversation_id));
        }

        let existing = Member::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(member::Column::UserId.eq(input.user_id))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(MessengerError::MemberAlreadyExists {
                conversation_id,
                user_id: input.user_id,
            });
        }
        let now = Utc::now();
        let new_id = Uuid::now_v7();
        let new_member = member::ActiveModel {
            id: Set(new_id),
            conversation_id: Set(conversation_id),
            user_id: Set(input.user_id),
            role: Set(role as i16),
            joined_at: Set(now),
            muted_until: Set(None),
        };
        new_member.insert(&self.db).await?;

        Ok(MemberOutput {
            id: new_id,
            conversation_id,
            user_id: input.user_id,
            role,
            joined_at: now,
            muted_until: None,
        })
    }

    /// Remove a member from a conversation or leave the conversation.
    pub async fn remove_member(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        conversation_id: Uuid,
        target_user_id: Uuid,
    ) -> MessengerResult<()> {
        let conv = Conversation::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationNotFound(conversation_id))?;

        if conv.conversation_type == ConversationType::Direct as i16 {
            return Err(MessengerError::Validation(
                "Cannot remove members from a 1-on-1 direct dialogue".to_string(),
            ));
        }

        let target_member = Member::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(member::Column::UserId.eq(target_user_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::MemberNotFound {
                conversation_id,
                user_id: target_user_id,
            })?;

        // If removing someone else, actor must be owner or admin
        if actor_id != target_user_id {
            let actor_member = Member::find()
                .filter(member::Column::ConversationId.eq(conversation_id))
                .filter(member::Column::UserId.eq(actor_id))
                .one(&self.db)
                .await?
                .ok_or(MessengerError::ConversationAccessDenied(conversation_id))?;

            if actor_member.role != MemberRole::Owner as i16 && actor_member.role != MemberRole::Admin as i16 {
                return Err(MessengerError::ConversationAccessDenied(conversation_id));
            }
            if target_member.role == MemberRole::Owner as i16 && actor_member.role != MemberRole::Owner as i16 {
                return Err(MessengerError::ConversationAccessDenied(conversation_id));
            }
        } else if target_member.role == MemberRole::Owner as i16 {
            // Owner leaving must ensure another owner exists or cannot leave as sole owner
            let other_owners = Member::find()
                .filter(member::Column::ConversationId.eq(conversation_id))
                .filter(member::Column::Role.eq(MemberRole::Owner as i16))
                .filter(member::Column::UserId.ne(actor_id))
                .count(&self.db)
                .await?;
            if other_owners == 0 {
                return Err(MessengerError::Validation(
                    "Conversation owner cannot leave without transferring ownership".to_string(),
                ));
            }
        }

        Member::delete_by_id(target_member.id).exec(&self.db).await?;
        Ok(())
    }

    /// List members of a conversation.
    pub async fn list_members(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        conversation_id: Uuid,
    ) -> MessengerResult<Vec<MemberOutput>> {
        let _conv = Conversation::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationNotFound(conversation_id))?;

        self.ensure_membership(conversation_id, actor_id).await?;

        let members = Member::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .order_by_asc(member::Column::JoinedAt)
            .all(&self.db)
            .await?;

        let outputs = members
            .into_iter()
            .map(|m| {
                let role = MemberRole::try_from(m.role).unwrap_or(MemberRole::Member);
                MemberOutput {
                    id: m.id,
                    conversation_id: m.conversation_id,
                    user_id: m.user_id,
                    role,
                    joined_at: m.joined_at,
                    muted_until: m.muted_until,
                }
            })
            .collect();

        Ok(outputs)
    }

    /// Internal transactional helper for monotonic cursor progression.
    pub async fn update_cursor_in_tx<C>(
        txn: &C,
        conversation_id: Uuid,
        actor_id: Uuid,
        last_read_message_id: i64,
    ) -> MessengerResult<crate::dto::CursorOutput>
    where
        C: sea_orm::ConnectionTrait,
    {
        let existing = Cursor::find()
            .filter(cursor::Column::ConversationId.eq(conversation_id))
            .filter(cursor::Column::UserId.eq(actor_id))
            .one(txn)
            .await?;

        let now = Utc::now();

        if let Some(c) = existing {
            if last_read_message_id < c.last_read_message_id {
                return Err(MessengerError::CursorRegression {
                    target_seq: last_read_message_id,
                    current_seq: c.last_read_message_id,
                });
            }
            let mut active: cursor::ActiveModel = c.into();
            active.last_read_message_id = Set(last_read_message_id);
            active.last_read_at = Set(now);
            active.update(txn).await?;
        } else {
            let new_cursor = cursor::ActiveModel {
                id: Set(Uuid::now_v7()),
                conversation_id: Set(conversation_id),
                user_id: Set(actor_id),
                last_read_message_id: Set(last_read_message_id),
                last_read_at: Set(now),
            };
            new_cursor.insert(txn).await?;
        }

        Ok(crate::dto::CursorOutput {
            conversation_id,
            user_id: actor_id,
            last_read_message_id,
        })
    }

    /// Monotonically advance user read cursor.
    pub async fn update_cursor(
        &self,
        _tenant_id: Uuid,
        actor_id: Uuid,
        input: UpdateCursorInput,
    ) -> MessengerResult<crate::dto::CursorOutput> {
        self.ensure_membership(input.conversation_id, actor_id).await?;
        Self::update_cursor_in_tx(&self.db, input.conversation_id, actor_id, input.last_read_message_id).await
    }

    pub async fn ensure_membership(&self, conversation_id: Uuid, user_id: Uuid) -> MessengerResult<()> {
        let is_member = Member::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(member::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .is_some();

        if !is_member {
            return Err(MessengerError::ConversationAccessDenied(conversation_id));
        }
        Ok(())
    }

    pub async fn ensure_active_member(&self, conversation_id: Uuid, user_id: Uuid) -> MessengerResult<member::Model> {
        let m = Member::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(member::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .ok_or(MessengerError::ConversationAccessDenied(conversation_id))?;

        if m.muted_until.is_some_and(|until| until > Utc::now()) {
            return Err(MessengerError::Validation(
                "User is currently muted in this conversation".to_string(),
            ));
        }
        Ok(m)
    }

    async fn calculate_unread_count(&self, conversation_id: Uuid, user_id: Uuid) -> MessengerResult<i64> {
        let user_cursor = Cursor::find()
            .filter(cursor::Column::ConversationId.eq(conversation_id))
            .filter(cursor::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?;

        let last_read = user_cursor.map(|c| c.last_read_message_id).unwrap_or(0);

        let count = Message::find()
            .filter(message::Column::ConversationId.eq(conversation_id))
            .filter(message::Column::Id.gt(last_read))
            .count(&self.db)
            .await?;

        Ok(count as i64)
    }

    /// Searches unencrypted conversation transcripts accessible to the actor.
    /// In accordance with Zero-Knowledge E2EE, encrypted conversations are strictly excluded
    /// from server-side plaintext indexing and search.
    pub async fn search_messages(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        query: crate::dto::SearchMessagesQuery,
    ) -> MessengerResult<Vec<MessageOutput>> {
        let trimmed = query.query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut select = Message::find()
            .filter(message::Column::TenantId.eq(tenant_id))
            .filter(message::Column::DeletedAt.is_null())
            .filter(message::Column::IsEncrypted.eq(false))
            .filter(message::Column::Content.contains(trimmed));

        if let Some(conv_id) = query.conversation_id {
            self.ensure_membership(conv_id, actor_id).await?;
            select = select.filter(message::Column::ConversationId.eq(conv_id));
        } else {
            let memberships = Member::find()
                .filter(member::Column::UserId.eq(actor_id))
                .all(&self.db)
                .await?;
            let conv_ids: Vec<Uuid> = memberships.into_iter().map(|m| m.conversation_id).collect();
            if conv_ids.is_empty() {
                return Ok(Vec::new());
            }
            select = select.filter(message::Column::ConversationId.is_in(conv_ids));
        }

        let limit = query.limit.unwrap_or(20).min(100);
        let offset = query.offset.unwrap_or(0);

        let messages = select
            .order_by_desc(message::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;

        Ok(messages.into_iter().map(Self::to_message_output).collect())
    }
}
