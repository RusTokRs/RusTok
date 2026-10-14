use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum ConversationType {
    /// 1-on-1 private dialogue between two users
    Direct = 1,
    /// Multi-user group chat
    Group = 2,
    /// Public or team broadcast channel with hierarchical topics
    Channel = 3,
    /// Contextual thread attached to an external business entity (CRM deal, order, task)
    EntityThread = 4,
}

impl TryFrom<i16> for ConversationType {
    type Error = &'static str;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Direct),
            2 => Ok(Self::Group),
            3 => Ok(Self::Channel),
            4 => Ok(Self::EntityThread),
            _ => Err("Invalid conversation type"),
        }
    }
}

/// Sovereign conversation domain aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
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
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new(
        tenant_id: Uuid,
        conversation_type: ConversationType,
        created_by: Uuid,
        title: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            conversation_type,
            title,
            entity_type: None,
            entity_id: None,
            is_encrypted: false,
            created_by,
            last_message_id: None,
            last_message_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_entity(mut self, entity_type: i16, entity_id: Uuid) -> Self {
        self.entity_type = Some(entity_type);
        self.entity_id = Some(entity_id);
        self
    }

    pub fn with_encryption(mut self, encrypted: bool) -> Self {
        self.is_encrypted = encrypted;
        self
    }
}
