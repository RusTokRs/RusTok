use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Role of a member in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(i16)]
pub enum MemberRole {
    Owner = 1,
    Admin = 2,
    Member = 3,
    Guest = 4,
}

impl TryFrom<i16> for MemberRole {
    type Error = &'static str;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Owner),
            2 => Ok(Self::Admin),
            3 => Ok(Self::Member),
            4 => Ok(Self::Guest),
            _ => Err("Invalid member role"),
        }
    }
}

/// Participant domain entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
    pub muted_until: Option<DateTime<Utc>>,
}

impl Member {
    pub fn new(conversation_id: Uuid, user_id: Uuid, role: MemberRole) -> Self {
        Self {
            id: Uuid::now_v7(),
            conversation_id,
            user_id,
            role,
            joined_at: Utc::now(),
            muted_until: None,
        }
    }

    pub fn is_muted(&self) -> bool {
        self.muted_until
            .map(|until| until > Utc::now())
            .unwrap_or(false)
    }

    pub fn can_manage(&self) -> bool {
        matches!(self.role, MemberRole::Owner | MemberRole::Admin)
    }
}
