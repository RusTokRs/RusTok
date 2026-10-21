use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::MemberRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberInput {
    pub user_id: Uuid,
    pub role: Option<MemberRole>,
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
