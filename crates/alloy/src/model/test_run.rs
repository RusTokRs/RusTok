use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::{ScriptId, ScriptSourceRevision};

pub const MAX_TEST_PATH_LENGTH: usize = 160;
pub const MAX_TEST_ACTOR_ID_LENGTH: usize = 255;
pub const MAX_TEST_ERROR_LENGTH: usize = 4 * 1024;
pub const TEST_RUN_LEASE_SECONDS: i64 = 60;

/// Authenticated request to execute one declared test entrypoint from an exact
/// current source revision. The revision is a CAS precondition and the
/// idempotency fingerprint includes every caller-controlled command field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCommand {
    pub script_id: ScriptId,
    pub expected_revision: u32,
    pub test_path: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
}

impl TestCommand {
    pub fn validate(&self) -> Result<(), TestRunError> {
        if self.script_id.is_nil()
            || self.expected_revision == 0
            || self.idempotency_key.is_nil()
            || !is_bounded_value(&self.test_path, MAX_TEST_PATH_LENGTH)
            || !self.test_path.starts_with("tests/")
            || !self.test_path.ends_with(".rhai")
            || !is_bounded_value(&self.actor_id, MAX_TEST_ACTOR_ID_LENGTH)
        {
            return Err(TestRunError::InvalidCommand);
        }
        Ok(())
    }

    pub fn request_digest(&self) -> Result<String, TestRunError> {
        self.validate()?;
        let bytes =
            serde_json::to_vec(self).map_err(|error| TestRunError::Serialize(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestRunStatus {
    Pending,
    Passed,
    Failed,
}

impl TestRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

/// Durable test evidence bound to one immutable source snapshot. Pending rows
/// are private work leases; terminal rows are the idempotent caller response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    pub id: Uuid,
    pub script_id: ScriptId,
    pub tenant_id: Uuid,
    pub revision: u32,
    pub source_digest: String,
    pub test_path: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
    pub request_digest: String,
    pub status: TestRunStatus,
    pub passed: Option<bool>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunLease {
    pub run: TestRun,
    pub lease_token: Uuid,
    pub source: ScriptSourceRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestRunClaim {
    Claimed(TestRunLease),
    Replay(TestRun),
    InProgress(TestRun),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunCompletion {
    pub passed: bool,
    pub error: Option<String>,
}

impl TestRunCompletion {
    pub fn passed() -> Self {
        Self {
            passed: true,
            error: None,
        }
    }

    pub fn failed(error: Option<String>) -> Result<Self, TestRunError> {
        let completion = Self {
            passed: false,
            error,
        };
        completion.validate()?;
        Ok(completion)
    }

    pub fn validate(&self) -> Result<(), TestRunError> {
        if self.passed && self.error.is_some()
            || self.error.as_ref().is_some_and(|error| {
                error.trim() != error
                    || error.is_empty()
                    || error.len() > MAX_TEST_ERROR_LENGTH
                    || error.chars().any(char::is_control)
            })
        {
            return Err(TestRunError::InvalidCompletion);
        }
        Ok(())
    }
}

pub fn test_run_lease_expires_at(now: DateTime<Utc>) -> DateTime<Utc> {
    now + Duration::seconds(TEST_RUN_LEASE_SECONDS)
}

fn is_bounded_value(value: &str, limit: usize) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.len() <= limit
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TestRunError {
    #[error("Alloy test command is invalid")]
    InvalidCommand,
    #[error("Alloy test completion is invalid")]
    InvalidCompletion,
    #[error("Alloy test idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Alloy test lease is no longer owned by this execution")]
    LeaseLost,
    #[error("Alloy test command serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{TestCommand, TestRunCompletion, TestRunError};

    #[test]
    fn test_command_requires_a_bounded_declared_test_entrypoint() {
        let mut command = TestCommand {
            script_id: Uuid::new_v4(),
            expected_revision: 3,
            test_path: "tests/smoke.rhai".into(),
            actor_id: "user:42".into(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(command.validate().is_ok());
        command.test_path = "src/main.rhai".into();
        assert_eq!(command.validate(), Err(TestRunError::InvalidCommand));
    }

    #[test]
    fn terminal_test_completion_cannot_mix_success_and_an_error() {
        assert!(TestRunCompletion::passed().validate().is_ok());
        assert!(TestRunCompletion::failed(Some("assertion returned false".into())).is_ok());
        assert_eq!(
            TestRunCompletion {
                passed: true,
                error: Some("unexpected".into()),
            }
            .validate(),
            Err(TestRunError::InvalidCompletion)
        );
    }
}
