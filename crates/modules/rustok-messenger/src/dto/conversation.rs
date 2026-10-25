use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::ConversationType;
use crate::error::MessengerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversationInput {
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub entity_type: Option<i16>,
    pub entity_id: Option<Uuid>,
    pub participant_ids: Vec<Uuid>,
    pub is_encrypted: Option<bool>,
    /// Optional initial localized translation for Channel type
    pub initial_locale: Option<String>,
    pub initial_channel_description: Option<String>,
}

impl CreateConversationInput {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.conversation_type == ConversationType::Direct && self.participant_ids.is_empty() {
            return Err(MessengerError::Validation(
                "Direct conversation requires at least one recipient".to_string(),
            ));
        }

        for participant_id in &self.participant_ids {
            if participant_id.is_nil() {
                return Err(MessengerError::Validation(
                    "Participant ID cannot be nil UUID".to_string(),
                ));
            }
        }

        if self.entity_id == Some(Uuid::nil()) {
            return Err(MessengerError::Validation(
                "Entity ID cannot be nil UUID".to_string(),
            ));
        }

        if let Some(title) = &self.title {
            let trimmed = title.trim();
            if trimmed.len() > 255 {
                return Err(MessengerError::Validation(
                    "Conversation title cannot exceed 255 characters".to_string(),
                ));
            }
        }

        if let Some(locale) = &self.initial_locale {
            let trimmed = locale.trim();
            if trimmed.is_empty() || trimmed.len() > 32 {
                return Err(MessengerError::InvalidLocale {
                    locale: locale.clone(),
                });
            }
            if rustok_api::normalize_locale_tag(trimmed).is_none() {
                return Err(MessengerError::InvalidLocale {
                    locale: locale.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelTranslationInput {
    pub channel_id: Uuid,
    pub locale: String,
    pub title: String,
    pub description: Option<String>,
}

impl UpdateChannelTranslationInput {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.channel_id.is_nil() {
            return Err(MessengerError::Validation(
                "Channel ID cannot be nil UUID".to_string(),
            ));
        }

        let trimmed_title = self.title.trim();
        if trimmed_title.is_empty() {
            return Err(MessengerError::Validation(
                "Translation title cannot be empty".to_string(),
            ));
        }
        if trimmed_title.len() > 255 {
            return Err(MessengerError::Validation(
                "Translation title cannot exceed 255 characters".to_string(),
            ));
        }

        let trimmed_locale = self.locale.trim();
        if trimmed_locale.is_empty() || trimmed_locale.len() > 32 {
            return Err(MessengerError::InvalidLocale {
                locale: self.locale.clone(),
            });
        }
        if rustok_api::normalize_locale_tag(trimmed_locale).is_none() {
            return Err(MessengerError::InvalidLocale {
                locale: self.locale.clone(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTranslationOutput {
    pub channel_id: Uuid,
    pub locale: String,
    pub title: String,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationOutput {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub conversation_type: ConversationType,
    pub title: Option<String>,
    pub entity_type: Option<i16>,
    pub entity_id: Option<Uuid>,
    pub is_encrypted: bool,
    pub created_by: Uuid,
    pub last_message_id: Option<i64>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub unread_count: i64,
}
