use std::{fmt, time::Duration};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use uuid::Uuid;

use super::{
    keyring_schedule_audit_handoff_postgres::{
        CommentsTcpDelegationScheduleAuditHandoffClaim,
        CommentsTcpDelegationScheduleAuditHandoffError,
    },
    keyring_schedule_persistence_postgres_audit as postgres_audit,
};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS: u32 = 100;
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS: u64 = 86_400;

const DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED: &str = "attempt_budget_exhausted";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditSourceFailureCode {
    Conflict,
    Unavailable,
}

impl CommentsTcpDelegationScheduleAuditSourceFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Unavailable => "unavailable",
        }
    }

    fn from_stored(value: &str) -> Option<Self> {
        match value {
            "conflict" => Some(Self::Conflict),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}

impl From<CommentsTcpDelegationScheduleAuditHandoffError>
    for CommentsTcpDelegationScheduleAuditSourceFailureCode
{
    fn from(value: CommentsTcpDelegationScheduleAuditHandoffError) -> Self {
        match value {
            CommentsTcpDelegationScheduleAuditHandoffError::Conflict => Self::Conflict,
            CommentsTcpDelegationScheduleAuditHandoffError::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditSourceFailureTransition {
    RetryScheduled {
        request_id: Uuid,
        attempt_count: i64,
    },
    DeadLettered {
        request_id: Uuid,
        attempt_count: i64,
    },
    StaleClaim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection {
    request_id: Uuid,
    attempt_count: i64,
    last_failure_code: Option<CommentsTcpDelegationScheduleAuditSourceFailureCode>,
}

impl CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection {
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn attempt_count(&self) -> i64 {
        self.attempt_count
    }

    pub fn last_failure_code(&self) -> Option<CommentsTcpDelegationScheduleAuditSourceFailureCode> {
        self.last_failure_code
    }

    pub const fn reason(&self) -> &'static str {
        DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditSourceRetryPolicyError {
    InvalidStoredState,
    Unavailable,
}

/// PostgreSQL owner for Blog source-row retry and exhaustion state.
///
/// This owner is intentionally explicit and task-free. It records one failed
/// claimed attempt behind the exact request/token/attempt fence, moves ordinary
/// failures to a bounded retry timestamp, terminalizes exhausted rows in the
/// Blog source dead letter, and exposes only a bounded storage inspection.
/// Canonical `sys_events` delivery remains owned by `rustok-outbox`.
#[derive(Clone)]
pub struct PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy {
    database: DatabaseConnection,
    max_attempts: i64,
    retry_delay_seconds: i64,
}

impl PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy {
    pub fn new(
        database: DatabaseConnection,
        max_attempts: u32,
        retry_delay: Duration,
    ) -> std::result::Result<Self, String> {
        if database.get_database_backend() != DbBackend::Postgres {
            return Err(
                "Comments schedule audit source retry policy requires PostgreSQL".to_string(),
            );
        }
        if max_attempts == 0
            || max_attempts > COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS
        {
            return Err(format!(
                "Comments schedule audit source max attempts must be in 1..={COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS}"
            ));
        }
        let retry_delay_seconds = retry_delay.as_secs();
        if retry_delay_seconds == 0
            || retry_delay_seconds
                > COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS
            || retry_delay != Duration::from_secs(retry_delay_seconds)
        {
            return Err(format!(
                "Comments schedule audit source retry delay must be a whole second in 1..={COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS}"
            ));
        }

        Ok(Self {
            database,
            max_attempts: i64::from(max_attempts),
            retry_delay_seconds: i64::try_from(retry_delay_seconds).map_err(|_| {
                "Comments schedule audit source retry delay is out of range".to_string()
            })?,
        })
    }

    pub fn max_attempts(&self) -> u32 {
        u32::try_from(self.max_attempts)
            .expect("validated Comments source max attempts must fit u32")
    }

    pub fn retry_delay(&self) -> Duration {
        Duration::from_secs(
            u64::try_from(self.retry_delay_seconds)
                .expect("validated Comments source retry delay must fit u64"),
        )
    }

    /// Records a failed publication attempt only while the exact source claim
    /// is still authoritative. A replaced or terminal claim is a closed no-op.
    pub async fn record_failure(
        &self,
        claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
        failure: CommentsTcpDelegationScheduleAuditHandoffError,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditSourceFailureTransition,
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    > {
        validate_claim(claim)?;
        let row = self
            .database
            .query_one_raw(record_failure_statement(
                claim,
                failure.into(),
                self.max_attempts,
                self.retry_delay_seconds,
            ))
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable)?;
        let Some(row) = row else {
            return Ok(CommentsTcpDelegationScheduleAuditSourceFailureTransition::StaleClaim);
        };
        decode_failure_transition(&row)
    }

    /// Closes the crash gap where the final bounded claim expired before the
    /// worker could record its failure. At most one oldest exhausted row is
    /// transitioned per explicit call using `FOR UPDATE SKIP LOCKED`.
    pub async fn dead_letter_next_expired_exhausted(
        &self,
    ) -> std::result::Result<
        Option<CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection>,
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    > {
        self.database
            .query_one_raw(dead_letter_expired_exhausted_statement(self.max_attempts))
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable)?
            .map(|row| decode_dead_letter(&row))
            .transpose()
    }

    /// Reads one exact terminal source dead letter without returning actor,
    /// tenant, payload, claim token, timestamps, SQL, or raw database details.
    /// Authorization remains a future server-owned wrapper responsibility.
    pub async fn inspect_dead_letter(
        &self,
        request_id: Uuid,
    ) -> std::result::Result<
        Option<CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection>,
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    > {
        if request_id.is_nil() {
            return Err(
                CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState,
            );
        }
        self.database
            .query_one_raw(inspect_dead_letter_statement(request_id))
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable)?
            .map(|row| decode_dead_letter(&row))
            .transpose()
    }
}

impl fmt::Debug for PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy")
            .field("database", &"[CONFIGURED]")
            .field("max_attempts", &self.max_attempts)
            .field("retry_delay_seconds", &self.retry_delay_seconds)
            .finish()
    }
}

fn validate_claim(
    claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
) -> std::result::Result<(), CommentsTcpDelegationScheduleAuditSourceRetryPolicyError> {
    if claim.request_id().is_nil() || claim.claim_token().is_nil() || claim.attempt_count() <= 0 {
        return Err(CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState);
    }
    Ok(())
}

fn decode_failure_transition(
    row: &QueryResult,
) -> std::result::Result<
    CommentsTcpDelegationScheduleAuditSourceFailureTransition,
    CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
> {
    let request_id: Uuid = row.try_get("", "request_id").map_err(|_| {
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
    })?;
    let attempt_count: i64 = row.try_get("", "handoff_attempt_count").map_err(|_| {
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
    })?;
    let dead_lettered: bool = row.try_get("", "dead_lettered").map_err(|_| {
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
    })?;
    if request_id.is_nil() || attempt_count <= 0 {
        return Err(CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState);
    }
    if dead_lettered {
        Ok(
            CommentsTcpDelegationScheduleAuditSourceFailureTransition::DeadLettered {
                request_id,
                attempt_count,
            },
        )
    } else {
        Ok(
            CommentsTcpDelegationScheduleAuditSourceFailureTransition::RetryScheduled {
                request_id,
                attempt_count,
            },
        )
    }
}

fn decode_dead_letter(
    row: &QueryResult,
) -> std::result::Result<
    CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection,
    CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
> {
    let request_id: Uuid = row.try_get("", "request_id").map_err(|_| {
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
    })?;
    let attempt_count: i64 = row.try_get("", "handoff_attempt_count").map_err(|_| {
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
    })?;
    let last_failure_code: Option<String> =
        row.try_get("", "handoff_last_failure_code").map_err(|_| {
            CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
        })?;
    let reason: String = row.try_get("", "handoff_dead_letter_reason").map_err(|_| {
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState
    })?;
    if request_id.is_nil()
        || attempt_count <= 0
        || reason != DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED
    {
        return Err(CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState);
    }
    let last_failure_code = last_failure_code
        .as_deref()
        .map(CommentsTcpDelegationScheduleAuditSourceFailureCode::from_stored)
        .transpose_option()
        .ok_or(CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState)?;

    Ok(
        CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection {
            request_id,
            attempt_count,
            last_failure_code,
        },
    )
}

trait TransposeOption<T> {
    fn transpose_option(self) -> Option<Option<T>>;
}

impl<T> TransposeOption<T> for Option<Option<T>> {
    fn transpose_option(self) -> Option<Option<T>> {
        match self {
            None => Some(None),
            Some(Some(value)) => Some(Some(value)),
            Some(None) => None,
        }
    }
}

fn record_failure_statement(
    claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
    failure_code: CommentsTcpDelegationScheduleAuditSourceFailureCode,
    max_attempts: i64,
    retry_delay_seconds: i64,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "UPDATE {table} \
             SET handoff_last_failure_at = NOW(), \
                 handoff_last_failure_code = $4, \
                 handoff_claim_token = NULL, \
                 handoff_claim_expires_at = NULL, \
                 handoff_next_attempt_at = CASE \
                     WHEN handoff_attempt_count >= $5 THEN NULL \
                     ELSE NOW() + ($6::bigint * INTERVAL '1 second') \
                 END, \
                 handoff_dead_lettered_at = CASE \
                     WHEN handoff_attempt_count >= $5 THEN NOW() \
                     ELSE NULL \
                 END, \
                 handoff_dead_letter_reason = CASE \
                     WHEN handoff_attempt_count >= $5 THEN '{dead_letter_reason}' \
                     ELSE NULL \
                 END \
             WHERE request_id = $1 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL \
               AND handoff_dead_lettered_at IS NULL \
               AND handoff_claim_token = $2 \
               AND handoff_attempt_count = $3 \
             RETURNING request_id, handoff_attempt_count, \
                       handoff_dead_lettered_at IS NOT NULL AS dead_lettered",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
            dead_letter_reason = DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED,
        ),
        vec![
            claim.request_id().into(),
            claim.claim_token().into(),
            claim.attempt_count().into(),
            failure_code.as_str().into(),
            max_attempts.into(),
            retry_delay_seconds.into(),
        ],
    )
}

fn dead_letter_expired_exhausted_statement(max_attempts: i64) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "WITH candidate AS ( \
                 SELECT request_id \
                 FROM {table} \
                 WHERE published_at IS NULL \
                   AND canonical_envelope_id IS NULL \
                   AND handoff_dead_lettered_at IS NULL \
                   AND handoff_attempt_count >= $1 \
                   AND (handoff_claim_token IS NULL OR handoff_claim_expires_at <= NOW()) \
                 ORDER BY created_at ASC, request_id ASC \
                 FOR UPDATE SKIP LOCKED \
                 LIMIT 1 \
             ) \
             UPDATE {table} AS audit \
             SET handoff_claim_token = NULL, \
                 handoff_claim_expires_at = NULL, \
                 handoff_next_attempt_at = NULL, \
                 handoff_dead_lettered_at = NOW(), \
                 handoff_dead_letter_reason = '{dead_letter_reason}' \
             FROM candidate \
             WHERE audit.request_id = candidate.request_id \
             RETURNING audit.request_id, audit.handoff_attempt_count, \
                       audit.handoff_last_failure_code, audit.handoff_dead_letter_reason",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
            dead_letter_reason = DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED,
        ),
        vec![max_attempts.into()],
    )
}

fn inspect_dead_letter_statement(request_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT request_id, handoff_attempt_count, handoff_last_failure_code, \
                    handoff_dead_letter_reason \
             FROM {table} \
             WHERE request_id = $1 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL \
               AND handoff_dead_lettered_at IS NOT NULL \
               AND handoff_dead_letter_reason = '{dead_letter_reason}' \
             LIMIT 1",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
            dead_letter_reason = DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED,
        ),
        vec![request_id.into()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_codes_are_closed_machine_values() {
        assert_eq!(
            CommentsTcpDelegationScheduleAuditSourceFailureCode::Conflict.as_str(),
            "conflict"
        );
        assert_eq!(
            CommentsTcpDelegationScheduleAuditSourceFailureCode::Unavailable.as_str(),
            "unavailable"
        );
        assert!(
            CommentsTcpDelegationScheduleAuditSourceFailureCode::from_stored("database_error")
                .is_none()
        );
    }

    #[test]
    fn failure_recording_is_exactly_fenced_and_bounded() {
        let claim = CommentsTcpDelegationScheduleAuditHandoffClaim::from_parts_for_test(
            Uuid::new_v4(),
            Uuid::new_v4(),
            3,
        );
        let statement = record_failure_statement(
            claim,
            CommentsTcpDelegationScheduleAuditSourceFailureCode::Unavailable,
            8,
            30,
        );
        let sql = statement.sql.as_str();
        assert!(sql.contains("handoff_claim_token = $2"));
        assert!(sql.contains("handoff_attempt_count = $3"));
        assert!(sql.contains("handoff_attempt_count >= $5"));
        assert!(sql.contains("handoff_next_attempt_at"));
        assert!(sql.contains("attempt_budget_exhausted"));
    }

    #[test]
    fn crash_gap_sweep_is_oldest_first_skip_locked() {
        let statement = dead_letter_expired_exhausted_statement(8);
        let sql = statement.sql.as_str();
        assert!(sql.contains("ORDER BY created_at ASC, request_id ASC"));
        assert!(sql.contains("FOR UPDATE SKIP LOCKED"));
        assert!(sql.contains("handoff_claim_expires_at <= NOW()"));
        assert!(sql.contains("LIMIT 1"));
    }
}
