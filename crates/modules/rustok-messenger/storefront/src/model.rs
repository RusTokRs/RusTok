use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wire payload for a conversation in the storefront list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationItemPayload {
    pub id: Uuid,
    pub conversation_type: String,
    pub title: Option<String>,
    pub topic: Option<String>,
    pub last_message_content: Option<String>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub unread_count: usize,
    pub is_encrypted: bool,
    pub entity_type: Option<i16>,
    pub entity_id: Option<Uuid>,
}

/// Wire payload for an individual chat message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageItemPayload {
    pub id: i64,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub sender_name: Option<String>,
    pub message_type: i16,
    pub content: String,
    pub blocks_json: Option<serde_json::Value>,
    pub reply_to_id: Option<i64>,
    pub is_encrypted: bool,
    pub created_at: DateTime<Utc>,
}

/// Outbound payload for sending a chat message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendMessagePayload {
    pub conversation_id: Uuid,
    pub content: String,
    pub reply_to_id: Option<i64>,
    pub client_msg_id: Option<String>,
}

/// Outbound payload for updating the read horizon cursor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateCursorPayload {
    pub conversation_id: Uuid,
    pub last_read_message_id: i64,
}
