use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{MessengerError, MessengerResult};

/// User read synchronization cursor (`pts` checkpoint).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCursor {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub last_read_message_id: i64,
    pub last_read_at: DateTime<Utc>,
}

impl SyncCursor {
    pub fn new(conversation_id: Uuid, user_id: Uuid, initial_seq: i64) -> Self {
        Self {
            id: Uuid::now_v7(),
            conversation_id,
            user_id,
            last_read_message_id: initial_seq,
            last_read_at: Utc::now(),
        }
    }

    /// Advance cursor monotonically. Regressions are strictly forbidden.
    pub fn advance(&mut self, target_seq: i64) -> MessengerResult<()> {
        if target_seq < self.last_read_message_id {
            return Err(MessengerError::CursorRegression {
                target_seq,
                current_seq: self.last_read_message_id,
            });
        }
        self.last_read_message_id = target_seq;
        self.last_read_at = Utc::now();
        Ok(())
    }
}
