use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::ConversationType;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelTranslationInput {
    pub channel_id: Uuid,
    pub locale: String,
    pub title: String,
    pub description: Option<String>,
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
