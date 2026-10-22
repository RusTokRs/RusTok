use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::message::MessageOutput;
use crate::error::MessengerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCursorInput {
    pub conversation_id: Uuid,
    pub last_read_message_id: i64,
}

impl UpdateCursorInput {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.conversation_id.is_nil() {
            return Err(MessengerError::Validation(
                "Conversation ID cannot be nil UUID".to_string(),
            ));
        }
        if self.last_read_message_id < 0 {
            return Err(MessengerError::Validation(
                "last_read_message_id cannot be negative".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorOutput {
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub last_read_message_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastSyncRequest {
    pub conversation_id: Uuid,
    /// Latest sequence cursor seen by the client. Replays all events after this sequence.
    pub since_seq: i64,
    pub limit: Option<u64>,
}

impl FastSyncRequest {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.conversation_id.is_nil() {
            return Err(MessengerError::Validation(
                "Conversation ID cannot be nil UUID".to_string(),
            ));
        }
        if self.since_seq < 0 {
            return Err(MessengerError::Validation(
                "since_seq cannot be negative".to_string(),
            ));
        }
        if self.limit == Some(0) {
            return Err(MessengerError::Validation(
                "limit must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastSyncResponse {
    pub conversation_id: Uuid,
    pub messages: Vec<MessageOutput>,
    pub has_more: bool,
    pub latest_seq: i64,
}
