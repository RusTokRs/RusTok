use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::HostRuntimeContext;
use rustok_notifications::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest, NotificationAudienceCandidate,
    NotificationAudienceCursor, NotificationAudiencePage, NotificationOpenAuthorization,
    NotificationPriority, NotificationProviderError, NotificationProviderResult,
    NotificationSemanticDescriptor, NotificationSourceEventRef, NotificationSourceProvider,
    NotificationSourceProviderFactory, NotificationSourceSlug, NotificationTargetKind,
    NotificationTargetRef, NotificationTargetRoute, NotificationTemplateData,
    NotificationTemplateKey, NotificationTypeKey, ResolveNotificationAudienceRequest,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use crate::entities::{conversation, member, message};

pub const MESSENGER_SOURCE: &str = "messenger";
pub const MESSAGE_DISPATCHED_TYPE: &str = "messenger.message.dispatched";
pub const MESSENGER_CONVERSATION_TARGET: &str = "messenger.conversation";

#[derive(Clone, Default)]
pub struct MessengerNotificationSourceProviderFactory;

impl NotificationSourceProviderFactory for MessengerNotificationSourceProviderFactory {
    fn slug(&self) -> NotificationSourceSlug {
        messenger_source_slug()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> NotificationProviderResult<Arc<dyn NotificationSourceProvider>> {
        Ok(Arc::new(MessengerNotificationSourceProvider::new(
            host.db_clone(),
        )))
    }
}

#[derive(Clone)]
pub struct MessengerNotificationSourceProvider {
    db: DatabaseConnection,
}

impl MessengerNotificationSourceProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn load_message_and_conversation(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationProviderResult<Option<(message::Model, conversation::Model)>> {
        if event.source() != &messenger_source_slug()
            || event.event_type() != &message_dispatched_type()
        {
            return Err(NotificationProviderError::InvalidEvent);
        }

        let message_id = i64::try_from(event.source_revision())
            .map_err(|_| NotificationProviderError::InvalidEvent)?;
        let conversation_id = event.event_id();

        let message = message::Entity::find()
            .filter(message::Column::TenantId.eq(event.tenant_id()))
            .filter(message::Column::ConversationId.eq(conversation_id))
            .filter(message::Column::Id.eq(message_id))
            .filter(message::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?;

        let Some(msg) = message else {
            return Ok(None);
        };

        let conv = conversation::Entity::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(event.tenant_id()))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?;

        let Some(c) = conv else {
            return Ok(None);
        };

        Ok(Some((msg, c)))
    }

    fn validate_descriptor(
        &self,
        event: &NotificationSourceEventRef,
        descriptor: &NotificationSemanticDescriptor,
    ) -> NotificationProviderResult<()> {
        if descriptor.notification_type != message_dispatched_type()
            || descriptor.target.owner != messenger_source_slug()
            || descriptor.target.kind != messenger_conversation_target_kind()
            || descriptor.target.id != event.event_id()
        {
            return Err(NotificationProviderError::Rejected);
        }
        Ok(())
    }
}

#[async_trait]
impl NotificationSourceProvider for MessengerNotificationSourceProvider {
    fn slug(&self) -> NotificationSourceSlug {
        messenger_source_slug()
    }

    fn display_name(&self) -> &'static str {
        "Messenger"
    }

    fn supported_types(&self) -> Vec<NotificationTypeKey> {
        vec![message_dispatched_type()]
    }

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
        let Some((message, conversation)) =
            self.load_message_and_conversation(&request.event).await?
        else {
            return Ok(None);
        };

        // Privacy Invariant: NEVER leak plaintext for encrypted chats or encrypted messages
        let preview = if message.is_encrypted
            || conversation.is_encrypted
            || message.encryption_algo.is_some()
        {
            "New encrypted message".to_string()
        } else {
            let trimmed = message.content.trim();
            if trimmed.is_empty() {
                if message.blocks_json.is_some() {
                    "Interactive message received".to_string()
                } else {
                    "Attachment received".to_string()
                }
            } else if trimmed.chars().count() > 120 {
                let truncated: String = trimmed.chars().take(120).collect();
                format!("{truncated}...")
            } else {
                trimmed.to_string()
            }
        };

        let mut data = BTreeMap::new();
        data.insert("conversation_id".to_string(), conversation.id.to_string());
        data.insert("message_id".to_string(), message.id.to_string());
        data.insert("sender_id".to_string(), message.sender_id.to_string());
        data.insert("message_preview".to_string(), preview);
        if let Some(title) = conversation.title.as_deref().filter(|t| !t.trim().is_empty()) {
            data.insert("conversation_title".to_string(), title.trim().to_string());
        }

        let template_data = NotificationTemplateData::try_new(data)
            .map_err(|_| NotificationProviderError::InvalidEvent)?;

        Ok(Some(NotificationSemanticDescriptor {
            notification_type: message_dispatched_type(),
            template_key: NotificationTemplateKey::new(MESSAGE_DISPATCHED_TYPE)
                .expect("messenger message dispatched template key must stay valid"),
            target: NotificationTargetRef {
                owner: messenger_source_slug(),
                kind: messenger_conversation_target_kind(),
                id: conversation.id,
            },
            actor_id: Some(message.sender_id),
            priority: NotificationPriority::Normal,
            template_data,
        }))
    }

    async fn resolve_audience(
        &self,
        request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage> {
        if request.event.source() != &messenger_source_slug()
            || request.event.event_type() != &message_dispatched_type()
        {
            return Err(NotificationProviderError::InvalidEvent);
        }
        self.validate_descriptor(&request.event, &request.descriptor)?;

        let limit = request.bounded_limit();
        if limit == 0 {
            return Err(NotificationProviderError::Rejected);
        }

        let conversation_id = request.descriptor.target.id;

        // Tenant isolation: verify conversation belongs to event tenant
        let conv_exists = conversation::Entity::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(request.event.tenant_id()))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
            .is_some();

        if !conv_exists {
            return Ok(NotificationAudiencePage::empty());
        }

        let actor_id = request.descriptor.actor_id;
        let now = Utc::now();

        let mut query = member::Entity::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(
                Condition::any()
                    .add(member::Column::MutedUntil.is_null())
                    .add(member::Column::MutedUntil.lt(now)),
            );

        if let Some(sender_id) = actor_id {
            query = query.filter(member::Column::UserId.ne(sender_id));
        }

        let after_user_id = match &request.cursor {
            Some(cursor) => match Uuid::parse_str(cursor.as_str()) {
                Ok(uuid) => Some(uuid),
                Err(_) => return Err(NotificationProviderError::InvalidEvent),
            },
            None => None,
        };

        if let Some(cursor_id) = after_user_id {
            query = query.filter(member::Column::UserId.gt(cursor_id));
        }

        query = query
            .order_by_asc(member::Column::UserId)
            .limit((limit as u64) + 1);

        let members = query
            .all(&self.db)
            .await
            .map_err(retryable_database_error)?;

        let has_more = members.len() > limit;
        let batch = if has_more {
            &members[..limit]
        } else {
            &members[..]
        };

        let next_cursor = if has_more {
            let last_user_id = batch
                .last()
                .map(|m| m.user_id)
                .expect("batch cannot be empty if has_more is true");
            Some(
                NotificationAudienceCursor::new(last_user_id.to_string())
                    .map_err(|_| NotificationProviderError::Internal { retryable: false })?,
            )
        } else {
            None
        };

        let candidates = batch
            .iter()
            .map(|m| NotificationAudienceCandidate {
                recipient_id: m.user_id,
            })
            .collect();

        NotificationAudiencePage::try_new(candidates, next_cursor)
            .map_err(|_| NotificationProviderError::Internal { retryable: false })
    }

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization> {
        if request.validate().is_err() {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }

        if request.target.owner != messenger_source_slug()
            || request.target.kind != messenger_conversation_target_kind()
        {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }

        let conversation_id = request.target.id;

        // Tenant isolation: verify conversation belongs to request tenant
        let conv_exists = conversation::Entity::find_by_id(conversation_id)
            .filter(conversation::Column::TenantId.eq(request.tenant_id))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
            .is_some();

        if !conv_exists {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }

        let is_member = member::Entity::find()
            .filter(member::Column::ConversationId.eq(conversation_id))
            .filter(member::Column::UserId.eq(request.recipient_id))
            .one(&self.db)
            .await
            .map_err(retryable_database_error)?
            .is_some();

        if !is_member {
            return Ok(NotificationOpenAuthorization::Unavailable);
        }

        let route = NotificationTargetRoute::new(format!("/messages?c={conversation_id}"))
            .map_err(|_| NotificationProviderError::Internal { retryable: false })?;

        Ok(NotificationOpenAuthorization::Allowed { route })
    }
}

fn retryable_database_error(_error: sea_orm::DbErr) -> NotificationProviderError {
    NotificationProviderError::Internal { retryable: true }
}

pub fn messenger_source_slug() -> NotificationSourceSlug {
    NotificationSourceSlug::new(MESSENGER_SOURCE)
        .expect("messenger notification source slug must stay valid")
}

pub fn message_dispatched_type() -> NotificationTypeKey {
    NotificationTypeKey::new(MESSAGE_DISPATCHED_TYPE)
        .expect("messenger message dispatched type must stay valid")
}

pub fn messenger_conversation_target_kind() -> NotificationTargetKind {
    NotificationTargetKind::new(MESSENGER_CONVERSATION_TARGET)
        .expect("messenger conversation target kind must stay valid")
}
