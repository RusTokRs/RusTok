use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::message::MessageOutput;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCursorInput {
    pub conversation_id: Uuid,
    pub last_read_message_id: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastSyncResponse {
    pub conversation_id: Uuid,
    pub messages: Vec<MessageOutput>,
    pub has_more: bool,
    pub latest_seq: i64,
}
