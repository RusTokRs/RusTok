use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::MessageType;
use crate::error::MessengerError;
use crate::services::MAX_MESSAGE_CONTENT_SIZE;

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

impl SendMessageInput {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.conversation_id.is_nil() {
            return Err(MessengerError::Validation(
                "Conversation ID cannot be nil UUID".to_string(),
            ));
        }

        let content_len = self.content.len();
        if content_len > MAX_MESSAGE_CONTENT_SIZE {
            return Err(MessengerError::PayloadTooLarge {
                actual: content_len,
                max: MAX_MESSAGE_CONTENT_SIZE,
            });
        }

        if self.content.trim().is_empty() && self.blocks_json.is_none() {
            return Err(MessengerError::Validation(
                "Message content cannot be empty".to_string(),
            ));
        }

        if let Some(reply_to) = self.reply_to_id
            && reply_to <= 0
        {
            return Err(MessengerError::Validation(
                "reply_to_id must be a positive Snowflake ID".to_string(),
            ));
        }

        if let Some(algo) = &self.encryption_algo
            && algo != "m.olm.v1.curve25519-aes-sha2"
            && algo != "m.megolm.v1.aes-sha2"
        {
            return Err(MessengerError::Validation(format!(
                "Unsupported encryption algorithm: {algo}"
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditMessageInput {
    pub content: String,
}

impl EditMessageInput {
    pub fn validate(&self) -> Result<(), MessengerError> {
        let content_len = self.content.len();
        if content_len > MAX_MESSAGE_CONTENT_SIZE {
            return Err(MessengerError::PayloadTooLarge {
                actual: content_len,
                max: MAX_MESSAGE_CONTENT_SIZE,
            });
        }

        if self.content.trim().is_empty() {
            return Err(MessengerError::Validation(
                "Message content cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
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

impl ListMessagesQuery {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.conversation_id.is_nil() {
            return Err(MessengerError::Validation(
                "Conversation ID cannot be nil UUID".to_string(),
            ));
        }

        if self.limit == Some(0) {
            return Err(MessengerError::Validation(
                "Query limit must be greater than 0".to_string(),
            ));
        }

        if let Some(before) = self.before_id
            && before <= 0
        {
            return Err(MessengerError::Validation(
                "before_id must be positive".to_string(),
            ));
        }

        if let Some(after) = self.after_id
            && after < 0
        {
            return Err(MessengerError::Validation(
                "after_id cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMessagesQuery {
    pub query: String,
    pub conversation_id: Option<Uuid>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl SearchMessagesQuery {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.conversation_id == Some(Uuid::nil()) {
            return Err(MessengerError::Validation(
                "Conversation ID cannot be nil UUID".to_string(),
            ));
        }

        if self.limit == Some(0) {
            return Err(MessengerError::Validation(
                "Query limit must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}
