use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::ScriptId;

pub const MAX_REVIEW_POLICY_REVISION_LENGTH: usize = 128;
pub const MAX_REVIEW_ACTOR_ID_LENGTH: usize = 255;
pub const MAX_REVIEW_REASON_LENGTH: usize = 4 * 1024;

/// Terminal or actionable review state for one immutable Alloy source revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    ChangesRequested,
    Approved,
    Rejected,
    Archived,
}

impl ReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChangesRequested => "changes_requested",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "changes_requested" => Some(Self::ChangesRequested),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

/// Authenticated request to append a review decision for one admitted source
/// revision. The revision is both the CAS precondition and the immutable review
/// subject; a later workspace revision cannot reuse this command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewCommand {
    pub script_id: ScriptId,
    pub expected_revision: u32,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub actor_id: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
}

impl ReviewCommand {
    pub fn validate(&self) -> Result<(), ReviewError> {
        if self.script_id.is_nil()
            || self.expected_revision == 0
            || self.idempotency_key.is_nil()
            || !is_bounded_value(&self.policy_revision, MAX_REVIEW_POLICY_REVISION_LENGTH)
            || !is_bounded_value(&self.actor_id, MAX_REVIEW_ACTOR_ID_LENGTH)
            || self.reason.as_ref().is_some_and(|reason| {
                reason.trim() != reason
                    || reason.is_empty()
                    || reason.len() > MAX_REVIEW_REASON_LENGTH
                    || reason.chars().any(char::is_control)
            })
        {
            return Err(ReviewError::InvalidCommand);
        }
        Ok(())
    }

    pub fn request_digest(&self) -> Result<String, ReviewError> {
        self.validate()?;
        let bytes =
            serde_json::to_vec(self).map_err(|error| ReviewError::Serialize(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

/// Immutable evidence that an authorized reviewer decided the state of one
/// exact source revision under one policy revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDecision {
    pub id: Uuid,
    pub script_id: ScriptId,
    pub tenant_id: Uuid,
    pub revision: u32,
    pub source_digest: String,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub actor_id: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
    pub request_digest: String,
    pub created_at: DateTime<Utc>,
}

pub fn validate_transition(
    current: Option<ReviewStatus>,
    next: ReviewStatus,
) -> Result<(), ReviewError> {
    let allowed = match current {
        None => matches!(
            next,
            ReviewStatus::ChangesRequested
                | ReviewStatus::Approved
                | ReviewStatus::Rejected
                | ReviewStatus::Archived
        ),
        Some(ReviewStatus::ChangesRequested) => matches!(
            next,
            ReviewStatus::Approved | ReviewStatus::Rejected | ReviewStatus::Archived
        ),
        Some(ReviewStatus::Approved | ReviewStatus::Rejected) => next == ReviewStatus::Archived,
        Some(ReviewStatus::Archived) => false,
    };
    allowed
        .then_some(())
        .ok_or(ReviewError::InvalidTransition { current, next })
}

fn is_bounded_value(value: &str, limit: usize) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.len() <= limit
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReviewError {
    #[error("Alloy review command is invalid")]
    InvalidCommand,
    #[error("Alloy review transition from {current:?} to {next:?} is invalid")]
    InvalidTransition {
        current: Option<ReviewStatus>,
        next: ReviewStatus,
    },
    #[error("Alloy review idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Alloy review serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use super::{ReviewStatus, validate_transition};

    #[test]
    fn review_transitions_are_terminal_for_one_source_revision() {
        assert!(validate_transition(None, ReviewStatus::ChangesRequested).is_ok());
        assert!(
            validate_transition(Some(ReviewStatus::ChangesRequested), ReviewStatus::Approved)
                .is_ok()
        );
        assert!(validate_transition(Some(ReviewStatus::Approved), ReviewStatus::Archived).is_ok());
        assert!(validate_transition(Some(ReviewStatus::Approved), ReviewStatus::Rejected).is_err());
        assert!(validate_transition(Some(ReviewStatus::Archived), ReviewStatus::Approved).is_err());
    }
}
