use chrono::{DateTime, Utc};
use rustok_core::RetentionPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::{ScriptId, deleted_evidence_retention, deletion::is_bounded_owner_audit_value};

/// A transition of the retention policy for evidence owned by a deleted draft.
///
/// Applying a legal hold clears the ordinary collection deadline. Releasing a
/// hold starts a new owner-selected `retain_until` window rather than exposing
/// evidence to an immediate collection race.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptEvidenceRetentionAction {
    ApplyLegalHold,
    ReleaseLegalHold,
}

impl ScriptEvidenceRetentionAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApplyLegalHold => "apply_legal_hold",
            Self::ReleaseLegalHold => "release_legal_hold",
        }
    }
}

/// Owner-authenticated, revision-guarded retention transition.
///
/// The transport derives `actor_id` from the authenticated principal. The
/// caller supplies a bounded reason and idempotency key, but cannot supply a
/// retention deadline or policy value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptEvidenceRetentionCommand {
    pub script_id: ScriptId,
    pub deletion_request_digest: String,
    pub expected_retention_revision: u32,
    pub action: ScriptEvidenceRetentionAction,
    pub actor_id: String,
    pub reason: String,
    pub idempotency_key: Uuid,
}

impl ScriptEvidenceRetentionCommand {
    pub fn validate(&self) -> Result<(), ScriptEvidenceRetentionError> {
        if self.script_id.is_nil()
            || self.expected_retention_revision == 0
            || self.idempotency_key.is_nil()
            || !is_canonical_sha256_digest(&self.deletion_request_digest)
            || !is_bounded_owner_audit_value(&self.actor_id, super::MAX_DELETION_ACTOR_ID_LENGTH)
            || !is_bounded_owner_audit_value(&self.reason, super::MAX_DELETION_REASON_LENGTH)
        {
            return Err(ScriptEvidenceRetentionError::InvalidCommand);
        }
        Ok(())
    }

    pub fn request_digest(&self) -> Result<String, ScriptEvidenceRetentionError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| ScriptEvidenceRetentionError::Serialize(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

/// The source-free retention state for one deleted draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptEvidenceRetentionState {
    pub script_id: ScriptId,
    pub tenant_id: Uuid,
    pub deletion_request_digest: String,
    pub policy: RetentionPolicy,
    pub retain_until: Option<DateTime<Utc>>,
    pub retention_revision: u32,
}

impl ScriptEvidenceRetentionState {
    pub fn new(
        script_id: ScriptId,
        tenant_id: Uuid,
        deletion_request_digest: String,
        policy: RetentionPolicy,
        retain_until: Option<DateTime<Utc>>,
        retention_revision: u32,
    ) -> Result<Self, ScriptEvidenceRetentionError> {
        let state = Self {
            script_id,
            tenant_id,
            deletion_request_digest,
            policy,
            retain_until,
            retention_revision,
        };
        state.validate_shape()?;
        Ok(state)
    }

    pub fn transition(
        &self,
        action: ScriptEvidenceRetentionAction,
        now: DateTime<Utc>,
    ) -> Result<Self, ScriptEvidenceRetentionError> {
        self.validate_shape()?;
        let (policy, retain_until) = match (action, self.policy) {
            (ScriptEvidenceRetentionAction::ApplyLegalHold, RetentionPolicy::RetainUntil) => {
                (RetentionPolicy::LegalHold, None)
            }
            (ScriptEvidenceRetentionAction::ReleaseLegalHold, RetentionPolicy::LegalHold) => {
                let (policy, retain_until) = deleted_evidence_retention(now);
                (policy, Some(retain_until))
            }
            _ => return Err(ScriptEvidenceRetentionError::InvalidTransition),
        };
        Self::new(
            self.script_id,
            self.tenant_id,
            self.deletion_request_digest.clone(),
            policy,
            retain_until,
            self.retention_revision
                .checked_add(1)
                .ok_or(ScriptEvidenceRetentionError::RevisionOverflow)?,
        )
    }

    pub fn validate_shape(&self) -> Result<(), ScriptEvidenceRetentionError> {
        if self.script_id.is_nil()
            || self.retention_revision == 0
            || !is_canonical_sha256_digest(&self.deletion_request_digest)
        {
            return Err(ScriptEvidenceRetentionError::InvalidStoredState);
        }
        match (self.policy, self.retain_until) {
            (RetentionPolicy::RetainUntil, Some(_))
            | (RetentionPolicy::OwnerLifecycle | RetentionPolicy::LegalHold, None) => Ok(()),
            _ => Err(ScriptEvidenceRetentionError::InvalidStoredState),
        }
    }
}

fn is_canonical_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'))
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ScriptEvidenceRetentionError {
    #[error("Alloy evidence retention command is invalid")]
    InvalidCommand,
    #[error("Alloy evidence retention idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Alloy evidence retention transition is invalid for the current policy")]
    InvalidTransition,
    #[error("Alloy evidence retention revision overflowed")]
    RevisionOverflow,
    #[error("Alloy evidence retention state is invalid")]
    InvalidStoredState,
    #[error("Alloy evidence retention command serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use rustok_core::RetentionPolicy;
    use uuid::Uuid;

    use super::{
        ScriptEvidenceRetentionAction, ScriptEvidenceRetentionCommand, ScriptEvidenceRetentionState,
    };

    #[test]
    fn a_legal_hold_clears_the_deadline_and_release_starts_a_new_window() {
        let now = Utc::now();
        let retained = ScriptEvidenceRetentionState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            RetentionPolicy::RetainUntil,
            Some(now + Duration::days(1)),
            1,
        )
        .expect("initial retention state should be valid");

        let held = retained
            .transition(ScriptEvidenceRetentionAction::ApplyLegalHold, now)
            .expect("retain-until evidence should accept a legal hold");
        assert_eq!(held.policy, RetentionPolicy::LegalHold);
        assert_eq!(held.retain_until, None);
        assert_eq!(held.retention_revision, 2);

        let released = held
            .transition(
                ScriptEvidenceRetentionAction::ReleaseLegalHold,
                now + Duration::hours(1),
            )
            .expect("a legal hold should release to a new retention window");
        assert_eq!(released.policy, RetentionPolicy::RetainUntil);
        assert!(released.retain_until > Some(now + Duration::days(30)));
        assert_eq!(released.retention_revision, 3);
    }

    #[test]
    fn retention_commands_bind_actor_reason_action_and_revision() {
        let command = ScriptEvidenceRetentionCommand {
            script_id: Uuid::new_v4(),
            deletion_request_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            expected_retention_revision: 1,
            action: ScriptEvidenceRetentionAction::ApplyLegalHold,
            actor_id: "operator:42".to_string(),
            reason: "A legal investigation requires preservation.".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(command.request_digest().is_ok());
    }
}
