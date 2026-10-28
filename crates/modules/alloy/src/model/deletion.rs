use chrono::{DateTime, Duration, Utc};
use rustok_core::RetentionPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::ScriptId;

pub const MAX_DELETION_ACTOR_ID_LENGTH: usize = 255;
pub const MAX_DELETION_REASON_LENGTH: usize = 4 * 1024;
pub const DELETED_EVIDENCE_RETENTION_DAYS: i64 = 30;

/// The initial retention decision applied atomically with draft deletion.
///
/// The owner keeps source revisions, reviews, and test evidence hidden until
/// this deadline, then the retention reaper collects them and writes a
/// content-free purge receipt. Client deletion requests never choose the
/// policy. Owner authority to place evidence under a legal hold is a separate
/// lifecycle capability and is not implemented by draft deletion.
pub fn deleted_evidence_retention(deleted_at: DateTime<Utc>) -> (RetentionPolicy, DateTime<Utc>) {
    (
        RetentionPolicy::RetainUntil,
        deleted_at + Duration::days(DELETED_EVIDENCE_RETENTION_DAYS),
    )
}

/// Owner-authenticated deletion of one Alloy draft and its retained evidence.
///
/// The actor is injected by an authenticated owner transport. The requester
/// must provide a bounded retention reason but can never choose that actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptDeletionCommand {
    pub script_id: ScriptId,
    pub expected_revision: u32,
    pub actor_id: String,
    pub reason: String,
    pub idempotency_key: Uuid,
}

impl ScriptDeletionCommand {
    pub fn validate(&self) -> Result<(), ScriptDeletionError> {
        if self.script_id.is_nil()
            || self.expected_revision == 0
            || self.idempotency_key.is_nil()
            || !is_bounded_owner_audit_value(&self.actor_id, MAX_DELETION_ACTOR_ID_LENGTH)
            || !is_bounded_owner_audit_value(&self.reason, MAX_DELETION_REASON_LENGTH)
        {
            return Err(ScriptDeletionError::InvalidCommand);
        }
        Ok(())
    }

    pub fn request_digest(&self) -> Result<String, ScriptDeletionError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| ScriptDeletionError::Serialize(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

pub(super) fn is_bounded_owner_audit_value(value: &str, limit: usize) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.len() <= limit
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ScriptDeletionError {
    #[error("Alloy deletion command is invalid")]
    InvalidCommand,
    #[error("Alloy deletion idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Alloy deletion command serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::{
        DELETED_EVIDENCE_RETENTION_DAYS, ScriptDeletionCommand, ScriptDeletionError,
        deleted_evidence_retention,
    };

    #[test]
    fn deletion_requires_attributable_owner_identity_and_reason() {
        let mut command = ScriptDeletionCommand {
            script_id: Uuid::new_v4(),
            expected_revision: 2,
            actor_id: "user:42".to_string(),
            reason: "The draft was superseded.".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(command.validate().is_ok());
        command.reason = " ".to_string();
        assert_eq!(command.validate(), Err(ScriptDeletionError::InvalidCommand));
    }

    #[test]
    fn deletion_retention_is_a_fixed_owner_policy_not_a_client_field() {
        let deleted_at = Utc::now();
        let (policy, retain_until) = deleted_evidence_retention(deleted_at);
        assert_eq!(policy, rustok_core::RetentionPolicy::RetainUntil);
        assert_eq!(
            retain_until,
            deleted_at + chrono::Duration::days(DELETED_EVIDENCE_RETENTION_DAYS)
        );
    }
}
