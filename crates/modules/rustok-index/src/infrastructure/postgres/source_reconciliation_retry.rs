use std::time::Duration;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use uuid::Uuid;

const MAX_RECONCILIATION_ATTEMPTS: u32 = 100;
const MAX_BACKOFF_SECONDS: u64 = 86_400;
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_WORKER_ID_BYTES: usize = 191;
const RECONCILIATION_FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";
const RECONCILIATION_PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReconciliationRetryLease {
    tenant_id: Uuid,
    job_id: Uuid,
    worker_id: String,
    attempt_count: u32,
}

impl IndexReconciliationRetryLease {
    pub fn new(
        tenant_id: Uuid,
        job_id: Uuid,
        worker_id: impl Into<String>,
        attempt_count: u32,
    ) -> Result<Self, IndexReconciliationRetryError> {
        if tenant_id.is_nil() {
            return Err(IndexReconciliationRetryError::NilTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexReconciliationRetryError::NilJobId);
        }
        if attempt_count == 0 {
            return Err(IndexReconciliationRetryError::InvalidAttemptCount);
        }
        let worker_id = worker_id.into();
        validate_worker_id(&worker_id)?;
        Ok(Self {
            tenant_id,
            job_id,
            worker_id,
            attempt_count,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReconciliationRetryFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReconciliationRetryFailure {
    kind: IndexReconciliationRetryFailureKind,
    code: String,
}

impl IndexReconciliationRetryFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexReconciliationRetryError> {
        Self::new(IndexReconciliationRetryFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexReconciliationRetryError> {
        Self::new(IndexReconciliationRetryFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexReconciliationRetryFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexReconciliationRetryError> {
        let code = code.into();
        if !valid_failure_code(&code) {
            return Err(IndexReconciliationRetryError::InvalidFailureCode);
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexReconciliationRetryFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexReconciliationRetryPolicy {
    max_attempts: u32,
    base_backoff_seconds: u64,
    max_backoff_seconds: u64,
}

impl IndexReconciliationRetryPolicy {
    pub fn new(
        max_attempts: u32,
        base_backoff: Duration,
        max_backoff: Duration,
    ) -> Result<Self, IndexReconciliationRetryError> {
        if !(1..=MAX_RECONCILIATION_ATTEMPTS).contains(&max_attempts) {
            return Err(IndexReconciliationRetryError::InvalidPolicy(
                "max attempts must be between 1 and 100",
            ));
        }
        let base_backoff_seconds = validate_backoff(base_backoff)?;
        let max_backoff_seconds = validate_backoff(max_backoff)?;
        if base_backoff_seconds > max_backoff_seconds {
            return Err(IndexReconciliationRetryError::InvalidPolicy(
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
        failure_kind: IndexReconciliationRetryFailureKind,
    ) -> Result<IndexReconciliationRetryDisposition, IndexReconciliationRetryError> {
        if attempt_count == 0 {
            return Err(IndexReconciliationRetryError::InvalidAttemptCount);
        }
        if failure_kind == IndexReconciliationRetryFailureKind::Permanent {
            return Ok(IndexReconciliationRetryDisposition::TerminalPermanent {
                attempts: attempt_count,
            });
        }
        if attempt_count >= self.max_attempts {
            return Ok(IndexReconciliationRetryDisposition::TerminalExhausted {
                attempts: attempt_count,
            });
        }
        let shift = attempt_count.saturating_sub(1).min(63);
        let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
        let retry_after_seconds = self
            .base_backoff_seconds
            .saturating_mul(multiplier)
            .min(self.max_backoff_seconds);
        Ok(IndexReconciliationRetryDisposition::RetryScheduled {
            retry_after: Duration::from_secs(retry_after_seconds),
            next_attempt: attempt_count
                .checked_add(1)
                .ok_or(IndexReconciliationRetryError::InvalidAttemptCount)?,
        })
    }
}

impl Default for IndexReconciliationRetryPolicy {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(5), Duration::from_secs(300))
            .expect("default reconciliation retry policy must be valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReconciliationRetryDisposition {
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexReconciliationRetryError {
    #[error("Index reconciliation retry tenant id must not be nil")]
    NilTenantId,
    #[error("Index reconciliation retry job id must not be nil")]
    NilJobId,
    #[error("Index reconciliation retry worker id is invalid")]
    InvalidWorkerId,
    #[error("Index reconciliation retry attempt count must be greater than zero")]
    InvalidAttemptCount,
    #[error("Index reconciliation retry failure code is invalid")]
    InvalidFailureCode,
    #[error("invalid Index reconciliation retry policy: {0}")]
    InvalidPolicy(&'static str),
    #[error("Index reconciliation retry transition lost lease ownership")]
    LeaseLost,
    #[error("Index reconciliation retry storage operation failed")]
    Storage,
}

#[derive(Clone)]
pub struct PostgresIndexReconciliationRetryStore {
    db: DatabaseConnection,
    policy: IndexReconciliationRetryPolicy,
}

impl PostgresIndexReconciliationRetryStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            policy: IndexReconciliationRetryPolicy::default(),
        }
    }

    pub fn with_policy(db: DatabaseConnection, policy: IndexReconciliationRetryPolicy) -> Self {
        Self { db, policy }
    }

    pub fn policy(&self) -> IndexReconciliationRetryPolicy {
        self.policy
    }

    /// Applies one lease-fenced retry or terminal failure transition to an existing
    /// reconciliation job.
    ///
    /// Retryable failures below the policy limit keep the same job UUID and move
    /// `running -> pending` with deterministic `available_at`. Permanent or exhausted
    /// failures move `running -> failed`. The caller supplies only exact lease identity
    /// and a bounded machine-readable dependency code; raw source, database, request,
    /// transport, payload, or stack details are not accepted.
    pub async fn record_failure(
        &self,
        lease: &IndexReconciliationRetryLease,
        failure: &IndexReconciliationRetryFailure,
    ) -> Result<IndexReconciliationRetryDisposition, IndexReconciliationRetryError> {
        let disposition = self
            .policy
            .evaluate(lease.attempt_count(), failure.kind())?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let details = failure_details(failure);

        let rows_affected = match disposition {
            IndexReconciliationRetryDisposition::RetryScheduled { retry_after, .. } => {
                let retry_after_seconds = i64::try_from(retry_after.as_secs()).map_err(|_| {
                    IndexReconciliationRetryError::InvalidPolicy(
                        "retry delay exceeds the SQL range",
                    )
                })?;
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
                            RECONCILIATION_PAGE_FAILURE_CODE.to_owned().into(),
                            SqlValue::Json(Some(Box::new(details))),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?
                    .rows_affected()
            }
            IndexReconciliationRetryDisposition::TerminalPermanent { .. }
            | IndexReconciliationRetryDisposition::TerminalExhausted { .. } => {
                terminalize_failure(&self.db, backend, lease, details).await?
            }
        };

        if rows_affected != 1 {
            return Err(IndexReconciliationRetryError::LeaseLost);
        }
        Ok(disposition)
    }
}

async fn terminalize_failure(
    db: &DatabaseConnection,
    backend: DbBackend,
    lease: &IndexReconciliationRetryLease,
    details: JsonValue,
) -> Result<u64, IndexReconciliationRetryError> {
    db.execute_raw(Statement::from_sql_and_values(
        backend,
        terminal_failure_sql(backend),
        vec![
            uuid_value(lease.tenant_id(), backend),
            uuid_value(lease.job_id(), backend),
            lease.worker_id().to_owned().into(),
            i64::from(lease.attempt_count()).into(),
            RECONCILIATION_PAGE_FAILURE_CODE.to_owned().into(),
            SqlValue::Json(Some(Box::new(details))),
        ],
    ))
    .await
    .map_err(storage_error)
    .map(|result| result.rows_affected())
}

fn failure_details(failure: &IndexReconciliationRetryFailure) -> JsonValue {
    json!({
        "contract": RECONCILIATION_FAILURE_CONTRACT,
        "dependency_code": failure.code(),
        "retryable": failure.kind() == IndexReconciliationRetryFailureKind::Retryable,
    })
}

fn validate_backoff(value: Duration) -> Result<u64, IndexReconciliationRetryError> {
    if value.subsec_nanos() != 0 {
        return Err(IndexReconciliationRetryError::InvalidPolicy(
            "backoff must use whole seconds",
        ));
    }
    let seconds = value.as_secs();
    if seconds == 0 || seconds > MAX_BACKOFF_SECONDS {
        return Err(IndexReconciliationRetryError::InvalidPolicy(
            "backoff must be between 1 and 86400 seconds",
        ));
    }
    Ok(seconds)
}

fn validate_worker_id(value: &str) -> Result<(), IndexReconciliationRetryError> {
    if value.is_empty()
        || value.len() > MAX_WORKER_ID_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(IndexReconciliationRetryError::InvalidWorkerId);
    }
    Ok(())
}

fn valid_failure_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReconciliationRetryError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexReconciliationRetryError::Storage),
    }
}

fn storage_error(_error: impl std::fmt::Display) -> IndexReconciliationRetryError {
    IndexReconciliationRetryError::Storage
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
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
        "UPDATE index_jobs SET state = 'pending', available_at = {available_at}, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = {prefix}6, last_error_details = {prefix}7, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn terminal_failure_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'failed', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, last_error_code = {prefix}5, last_error_details = {prefix}6, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_uses_bounded_exponential_backoff() {
        let policy = IndexReconciliationRetryPolicy::default();
        for (attempt, seconds, next_attempt) in [(1, 5, 2), (2, 10, 3), (3, 20, 4), (4, 40, 5)] {
            assert_eq!(
                policy
                    .evaluate(attempt, IndexReconciliationRetryFailureKind::Retryable)
                    .unwrap(),
                IndexReconciliationRetryDisposition::RetryScheduled {
                    retry_after: Duration::from_secs(seconds),
                    next_attempt,
                }
            );
        }
        assert_eq!(
            policy
                .evaluate(5, IndexReconciliationRetryFailureKind::Retryable)
                .unwrap(),
            IndexReconciliationRetryDisposition::TerminalExhausted { attempts: 5 }
        );
    }

    #[test]
    fn permanent_failure_terminalizes_immediately() {
        assert_eq!(
            IndexReconciliationRetryPolicy::default()
                .evaluate(1, IndexReconciliationRetryFailureKind::Permanent)
                .unwrap(),
            IndexReconciliationRetryDisposition::TerminalPermanent { attempts: 1 }
        );
    }

    #[test]
    fn lease_policy_and_failure_validation_fail_closed() {
        assert!(matches!(
            IndexReconciliationRetryLease::new(Uuid::nil(), Uuid::new_v4(), "worker", 1),
            Err(IndexReconciliationRetryError::NilTenantId)
        ));
        assert!(matches!(
            IndexReconciliationRetryLease::new(Uuid::new_v4(), Uuid::nil(), "worker", 1),
            Err(IndexReconciliationRetryError::NilJobId)
        ));
        assert!(matches!(
            IndexReconciliationRetryLease::new(Uuid::new_v4(), Uuid::new_v4(), "worker", 0),
            Err(IndexReconciliationRetryError::InvalidAttemptCount)
        ));
        assert!(
            IndexReconciliationRetryPolicy::new(0, Duration::from_secs(1), Duration::from_secs(2),)
                .is_err()
        );
        assert!(
            IndexReconciliationRetryPolicy::new(2, Duration::from_secs(3), Duration::from_secs(2),)
                .is_err()
        );
        for code in ["", "UPPER", " leading", "contains/slash"] {
            assert!(IndexReconciliationRetryFailure::retryable(code).is_err());
        }
    }

    #[test]
    fn retry_sql_is_lease_fenced_and_preserves_job_identity() {
        let retry = schedule_retry_sql(DbBackend::Postgres);
        assert!(retry.contains("state = 'pending'"));
        assert!(retry.contains("available_at = CURRENT_TIMESTAMP + ($5 * INTERVAL '1 second')"));
        assert!(retry.contains("kind = 'reconcile'"));
        assert!(retry.contains("lease_owner = $3"));
        assert!(retry.contains("attempt_count = $4"));
        assert!(retry.contains("lease_expires_at > CURRENT_TIMESTAMP"));
        assert!(retry.contains("cancel_requested = FALSE"));
        assert!(!retry.contains("INSERT INTO index_jobs"));

        let terminal = terminal_failure_sql(DbBackend::Postgres);
        assert!(terminal.contains("state = 'failed'"));
        assert!(terminal.contains("completed_at = CURRENT_TIMESTAMP"));
        assert!(terminal.contains("kind = 'reconcile'"));
        assert!(terminal.contains("lease_owner = $3"));
        assert!(terminal.contains("attempt_count = $4"));
    }

    #[test]
    fn diagnostics_keep_the_existing_inspection_contract() {
        let details = failure_details(
            &IndexReconciliationRetryFailure::retryable("owner_source_retryable").unwrap(),
        );
        assert_eq!(details.as_object().unwrap().len(), 3);
        assert_eq!(
            details.get("contract").and_then(JsonValue::as_str),
            Some(RECONCILIATION_FAILURE_CONTRACT)
        );
        assert_eq!(
            details.get("dependency_code").and_then(JsonValue::as_str),
            Some("owner_source_retryable")
        );
        assert_eq!(
            details.get("retryable").and_then(JsonValue::as_bool),
            Some(true)
        );
    }
}
