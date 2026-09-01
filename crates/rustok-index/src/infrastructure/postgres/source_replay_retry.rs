use std::time::Duration;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;

use super::IndexReplayJobLease;

const MAX_REPLAY_ATTEMPTS: u32 = 100;
const MAX_BACKOFF_SECONDS: u64 = 86_400;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const RETRY_DETAILS_CONTRACT: &str = "index_replay_retry_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayRetryFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayRetryFailure {
    kind: IndexReplayRetryFailureKind,
    code: String,
}

impl IndexReplayRetryFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexReplayRetryError> {
        Self::new(IndexReplayRetryFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexReplayRetryError> {
        Self::new(IndexReplayRetryFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexReplayRetryFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexReplayRetryError> {
        let code = code.into();
        if !valid_failure_code(&code) {
            return Err(IndexReplayRetryError::InvalidFailureCode);
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexReplayRetryFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexReplayRetryPolicy {
    max_attempts: u32,
    base_backoff_seconds: u64,
    max_backoff_seconds: u64,
}

impl IndexReplayRetryPolicy {
    pub fn new(
        max_attempts: u32,
        base_backoff: Duration,
        max_backoff: Duration,
    ) -> Result<Self, IndexReplayRetryError> {
        if !(1..=MAX_REPLAY_ATTEMPTS).contains(&max_attempts) {
            return Err(IndexReplayRetryError::InvalidPolicy(
                "max attempts must be between 1 and 100",
            ));
        }
        let base_backoff_seconds = validate_backoff(base_backoff)?;
        let max_backoff_seconds = validate_backoff(max_backoff)?;
        if base_backoff_seconds > max_backoff_seconds {
            return Err(IndexReplayRetryError::InvalidPolicy(
                "base backoff must not exceed max backoff",
            ));
        }
        Ok(Self {
            max_attempts,
            base_backoff_seconds,
            max_backoff_seconds,
        })
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    pub fn base_backoff(&self) -> Duration {
        Duration::from_secs(self.base_backoff_seconds)
    }

    pub fn max_backoff(&self) -> Duration {
        Duration::from_secs(self.max_backoff_seconds)
    }

    pub fn evaluate(
        &self,
        attempt_count: u32,
        failure_kind: IndexReplayRetryFailureKind,
    ) -> Result<IndexReplayRetryDisposition, IndexReplayRetryError> {
        if attempt_count == 0 {
            return Err(IndexReplayRetryError::InvalidAttemptCount);
        }
        if failure_kind == IndexReplayRetryFailureKind::Permanent {
            return Ok(IndexReplayRetryDisposition::TerminalPermanent {
                attempts: attempt_count,
            });
        }
        if attempt_count >= self.max_attempts {
            return Ok(IndexReplayRetryDisposition::TerminalExhausted {
                attempts: attempt_count,
            });
        }
        let shift = attempt_count.saturating_sub(1).min(63);
        let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
        let retry_after_seconds = self
            .base_backoff_seconds
            .saturating_mul(multiplier)
            .min(self.max_backoff_seconds);
        Ok(IndexReplayRetryDisposition::RetryScheduled {
            retry_after: Duration::from_secs(retry_after_seconds),
            next_attempt: attempt_count
                .checked_add(1)
                .ok_or(IndexReplayRetryError::InvalidAttemptCount)?,
        })
    }
}

impl Default for IndexReplayRetryPolicy {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(5), Duration::from_secs(300))
            .expect("default replay retry policy must be valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayRetryDisposition {
    RetryScheduled {
        retry_after: Duration,
        next_attempt: u32,
    },
    TerminalPermanent {
        attempts: u32,
    },
    TerminalExhausted {
        attempts: u32,
    },
}

#[derive(Debug, Error)]
pub enum IndexReplayRetryError {
    #[error("invalid Index replay retry policy: {0}")]
    InvalidPolicy(&'static str),
    #[error("Index replay retry attempt count must be greater than zero")]
    InvalidAttemptCount,
    #[error("Index replay retry failure code is invalid")]
    InvalidFailureCode,
    #[error("Index replay retry transition lost lease ownership")]
    LeaseLost,
    #[error("Index replay retry storage operation failed")]
    Storage,
}

#[derive(Clone)]
pub struct PostgresIndexReplayRetryStore {
    db: DatabaseConnection,
    policy: IndexReplayRetryPolicy,
}

impl PostgresIndexReplayRetryStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            policy: IndexReplayRetryPolicy::default(),
        }
    }

    pub fn with_policy(db: DatabaseConnection, policy: IndexReplayRetryPolicy) -> Self {
        Self { db, policy }
    }

    pub fn policy(&self) -> IndexReplayRetryPolicy {
        self.policy
    }

    /// Applies one lease-fenced retry or terminal failure transition.
    ///
    /// The caller must pass only a bounded machine-readable dependency code. Raw source,
    /// database, transport, request, or tenant details are intentionally not accepted.
    /// A retryable failure keeps the same job identity and moves it to `pending` with a
    /// deterministic `available_at` delay. Permanent or exhausted failures terminalize the
    /// current job as `failed`. Runner integration, automatic scheduling, and scope-level
    /// dead-letter blocking/requeue remain separate ownership boundaries.
    pub async fn record_failure(
        &self,
        lease: &IndexReplayJobLease,
        failure: &IndexReplayRetryFailure,
    ) -> Result<IndexReplayRetryDisposition, IndexReplayRetryError> {
        let disposition = self
            .policy
            .evaluate(lease.attempt_count(), failure.kind())?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;

        let rows_affected = match disposition {
            IndexReplayRetryDisposition::RetryScheduled {
                retry_after,
                next_attempt,
            } => {
                let retry_after_seconds = i64::try_from(retry_after.as_secs()).map_err(|_| {
                    IndexReplayRetryError::InvalidPolicy("retry delay exceeds the SQL range")
                })?;
                let details = retry_details(
                    lease,
                    failure,
                    "retry_scheduled",
                    Some(retry_after.as_secs()),
                    Some(next_attempt),
                    self.policy,
                );
                self.db
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        schedule_retry_sql(backend),
                        vec![
                            uuid_value(lease.tenant_id(), backend),
                            uuid_value(lease.job_id(), backend),
                            lease.worker_id().to_owned().into(),
                            i64::from(lease.attempt_count()).into(),
                            retry_after_seconds.into(),
                            failure.code().to_owned().into(),
                            SqlValue::Json(Some(Box::new(details))),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?
                    .rows_affected()
            }
            IndexReplayRetryDisposition::TerminalPermanent { .. } => {
                let details = retry_details(
                    lease,
                    failure,
                    "terminal_permanent",
                    None,
                    None,
                    self.policy,
                );
                terminalize_failure(&self.db, backend, lease, failure, details).await?
            }
            IndexReplayRetryDisposition::TerminalExhausted { .. } => {
                let details = retry_details(
                    lease,
                    failure,
                    "terminal_exhausted",
                    None,
                    None,
                    self.policy,
                );
                terminalize_failure(&self.db, backend, lease, failure, details).await?
            }
        };

        if rows_affected != 1 {
            return Err(IndexReplayRetryError::LeaseLost);
        }
        Ok(disposition)
    }
}

async fn terminalize_failure(
    db: &DatabaseConnection,
    backend: DbBackend,
    lease: &IndexReplayJobLease,
    failure: &IndexReplayRetryFailure,
    details: JsonValue,
) -> Result<u64, IndexReplayRetryError> {
    db.execute_raw(Statement::from_sql_and_values(
        backend,
        terminal_failure_sql(backend),
        vec![
            uuid_value(lease.tenant_id(), backend),
            uuid_value(lease.job_id(), backend),
            lease.worker_id().to_owned().into(),
            i64::from(lease.attempt_count()).into(),
            failure.code().to_owned().into(),
            SqlValue::Json(Some(Box::new(details))),
        ],
    ))
    .await
    .map_err(storage_error)
    .map(|result| result.rows_affected())
}

fn retry_details(
    lease: &IndexReplayJobLease,
    failure: &IndexReplayRetryFailure,
    disposition: &'static str,
    retry_after_seconds: Option<u64>,
    next_attempt: Option<u32>,
    policy: IndexReplayRetryPolicy,
) -> JsonValue {
    json!({
        "contract": RETRY_DETAILS_CONTRACT,
        "dependency_code": failure.code(),
        "failure_kind": match failure.kind() {
            IndexReplayRetryFailureKind::Retryable => "retryable",
            IndexReplayRetryFailureKind::Permanent => "permanent",
        },
        "attempt_count": lease.attempt_count(),
        "max_attempts": policy.max_attempts(),
        "disposition": disposition,
        "retry_after_seconds": retry_after_seconds,
        "next_attempt": next_attempt,
    })
}

fn validate_backoff(value: Duration) -> Result<u64, IndexReplayRetryError> {
    if value.subsec_nanos() != 0 {
        return Err(IndexReplayRetryError::InvalidPolicy(
            "backoff must use whole seconds",
        ));
    }
    let seconds = value.as_secs();
    if seconds == 0 || seconds > MAX_BACKOFF_SECONDS {
        return Err(IndexReplayRetryError::InvalidPolicy(
            "backoff must be between 1 and 86400 seconds",
        ));
    }
    Ok(seconds)
}

fn valid_failure_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReplayRetryError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexReplayRetryError::Storage),
    }
}

fn storage_error(_error: impl std::fmt::Display) -> IndexReplayRetryError {
    IndexReplayRetryError::Storage
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: uuid::Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn available_at_expression(backend: DbBackend, parameter: usize) -> String {
    let prefix = placeholder_prefix(backend);
    match backend {
        DbBackend::Postgres => {
            format!("CURRENT_TIMESTAMP + ({prefix}{parameter} * INTERVAL '1 second')")
        }
        DbBackend::Sqlite => {
            format!("datetime('now', '+' || {prefix}{parameter} || ' seconds')")
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn schedule_retry_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let available_at = available_at_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET state = 'pending', available_at = {available_at}, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = {prefix}6, last_error_details = {prefix}7, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn terminal_failure_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'failed', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, last_error_code = {prefix}5, last_error_details = {prefix}6, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_uses_bounded_exponential_backoff() {
        let policy = IndexReplayRetryPolicy::default();
        for (attempt, seconds, next_attempt) in [(1, 5, 2), (2, 10, 3), (3, 20, 4), (4, 40, 5)] {
            assert_eq!(
                policy
                    .evaluate(attempt, IndexReplayRetryFailureKind::Retryable)
                    .unwrap(),
                IndexReplayRetryDisposition::RetryScheduled {
                    retry_after: Duration::from_secs(seconds),
                    next_attempt,
                }
            );
        }
        assert_eq!(
            policy
                .evaluate(5, IndexReplayRetryFailureKind::Retryable)
                .unwrap(),
            IndexReplayRetryDisposition::TerminalExhausted { attempts: 5 }
        );
    }

    #[test]
    fn permanent_failure_is_terminal_on_the_current_attempt() {
        assert_eq!(
            IndexReplayRetryPolicy::default()
                .evaluate(1, IndexReplayRetryFailureKind::Permanent)
                .unwrap(),
            IndexReplayRetryDisposition::TerminalPermanent { attempts: 1 }
        );
    }

    #[test]
    fn policy_and_failure_code_validation_fail_closed() {
        assert!(
            IndexReplayRetryPolicy::new(0, Duration::from_secs(1), Duration::from_secs(2)).is_err()
        );
        assert!(
            IndexReplayRetryPolicy::new(2, Duration::from_secs(3), Duration::from_secs(2)).is_err()
        );
        for code in ["", "UPPER", " leading", "contains/slash"] {
            assert!(IndexReplayRetryFailure::retryable(code).is_err());
        }
    }

    #[test]
    fn retry_sql_is_lease_fenced_and_preserves_existing_job_identity() {
        let postgres = schedule_retry_sql(DbBackend::Postgres);
        assert!(postgres.contains("state = 'pending'"));
        assert!(postgres.contains("available_at = CURRENT_TIMESTAMP + ($5 * INTERVAL '1 second')"));
        assert!(postgres.contains("lease_owner = $3"));
        assert!(postgres.contains("attempt_count = $4"));
        assert!(postgres.contains("lease_expires_at > CURRENT_TIMESTAMP"));
        assert!(postgres.contains("cancel_requested = FALSE"));
        assert!(!postgres.contains("INSERT INTO index_jobs"));

        let terminal = terminal_failure_sql(DbBackend::Postgres);
        assert!(terminal.contains("state = 'failed'"));
        assert!(terminal.contains("completed_at = CURRENT_TIMESTAMP"));
        assert!(terminal.contains("lease_owner = $3"));
        assert!(terminal.contains("attempt_count = $4"));
    }
}
