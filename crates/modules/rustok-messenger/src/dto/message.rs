use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::MessageType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageInput {
    pub conversation_id: Uuid,
    pub message_type: Option<MessageType>,
    pub content: String,
    pub blocks_json: Option<serde_json::Value>,
    pub reply_to_id: Option<i64>,
    pub is_encrypted: Option<bool>,
    pub encryption_algo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageOutput {
    pub id: i64,
    pub tenant_id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub message_type: MessageType,
    pub content: String,
    pub blocks_json: Option<serde_json::Value>,
    pub is_encrypted: bool,
    pub encryption_algo: Option<String>,
    pub reply_to_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMessagesQuery {
    pub conversation_id: Uuid,
    pub limit: Option<u64>,
    pub before_id: Option<i64>,
    pub after_id: Option<i64>,
}
