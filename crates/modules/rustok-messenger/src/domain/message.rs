use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Format of message payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum MessageType {
    /// Standard plain text or markdown
    Text = 1,
    /// Declarative UI blocks (Slack Block Kit / Discord components)
    BlockJson = 2,
    /// Media attachments (images, voice notes, documents)
    Media = 3,
    /// System generated audit notices (user joined, call started)
    System = 4,
}

impl TryFrom<i16> for MessageType {
    type Error = &'static str;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Text),
            2 => Ok(Self::BlockJson),
            3 => Ok(Self::Media),
            4 => Ok(Self::System),
            _ => Err("Invalid message type"),
        }
    }
}

/// Append-only message domain entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Monotonically increasing 64-bit sequence identifier (Snowflake)
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
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Message {
    pub fn new(
        id: i64,
        tenant_id: Uuid,
        conversation_id: Uuid,
        sender_id: Uuid,
        message_type: MessageType,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id,
            tenant_id,
            conversation_id,
            sender_id,
            message_type,
            content: content.into(),
            blocks_json: None,
            is_encrypted: false,
            encryption_algo: None,
            reply_to_id: None,
            created_at: Utc::now(),
            deleted_at: None,
        }
    }

    pub fn with_blocks(mut self, blocks: serde_json::Value) -> Self {
        self.message_type = MessageType::BlockJson;
        self.blocks_json = Some(blocks);
        self
    }

    pub fn with_reply(mut self, reply_to_id: i64) -> Self {
        self.reply_to_id = Some(reply_to_id);
        self
    }

    pub fn with_encryption(mut self, algo: impl Into<String>) -> Self {
        self.is_encrypted = true;
        self.encryption_algo = Some(algo.into());
        self
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
