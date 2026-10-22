use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::MemberRole;
use crate::error::MessengerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberInput {
    pub user_id: Uuid,
    pub role: Option<MemberRole>,
}

impl AddMemberInput {
    pub fn validate(&self) -> Result<(), MessengerError> {
        if self.user_id.is_nil() {
            return Err(MessengerError::Validation(
                "user_id cannot be nil UUID".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberOutput {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
    pub muted_until: Option<DateTime<Utc>>,
}
