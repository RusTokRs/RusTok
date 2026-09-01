use std::fmt;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

use super::{
    keyring_schedule_audit_source_retry_postgres::CommentsTcpDelegationScheduleAuditSourceFailureCode,
    keyring_schedule_persistence_postgres_audit as postgres_audit,
};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_ACTION: &str = "requeue";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_MAX_REASON_BYTES: usize = 512;
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_TABLE: &str =
    "blog_comments_tcp_delegation_schedule_audit_recovery_audits";

const DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED: &str = "attempt_budget_exhausted";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditRecoveryRequest {
    control_plane_tenant_id: Uuid,
    request_id: Uuid,
    actor_id: Uuid,
    expected_attempt_count: i64,
    expected_recovery_epoch: i64,
    reason: String,
}

impl CommentsTcpDelegationScheduleAuditRecoveryRequest {
    pub fn new(
        control_plane_tenant_id: Uuid,
        request_id: Uuid,
        actor_id: Uuid,
        expected_attempt_count: i64,
        expected_recovery_epoch: i64,
        reason: impl Into<String>,
    ) -> std::result::Result<Self, CommentsTcpDelegationScheduleAuditRecoveryError> {
        if control_plane_tenant_id.is_nil() {
            return Err(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                    "control-plane tenant ID must not be nil",
                ),
            );
        }
        if request_id.is_nil() {
            return Err(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                    "request ID must not be nil",
                ),
            );
        }
        if actor_id.is_nil() {
            return Err(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                    "actor ID must not be nil",
                ),
            );
        }
        if expected_attempt_count <= 0 {
            return Err(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                    "expected attempt count must be positive",
                ),
            );
        }
        if expected_recovery_epoch < 0 {
            return Err(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                    "expected recovery epoch must not be negative",
                ),
            );
        }
        let reason = reason.into();
        validate_recovery_reason(&reason)?;
        Ok(Self {
            control_plane_tenant_id,
            request_id,
            actor_id,
            expected_attempt_count,
            expected_recovery_epoch,
            reason,
        })
    }

    pub fn control_plane_tenant_id(&self) -> Uuid {
        self.control_plane_tenant_id
    }

    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    pub fn expected_attempt_count(&self) -> i64 {
        self.expected_attempt_count
    }

    pub fn expected_recovery_epoch(&self) -> i64 {
        self.expected_recovery_epoch
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditRecoveryInspection {
    request_id: Uuid,
    attempt_count: i64,
    recovery_epoch: i64,
    last_failure_code: Option<CommentsTcpDelegationScheduleAuditSourceFailureCode>,
}

impl CommentsTcpDelegationScheduleAuditRecoveryInspection {
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn attempt_count(&self) -> i64 {
        self.attempt_count
    }

    pub fn recovery_epoch(&self) -> i64 {
        self.recovery_epoch
    }

    pub fn last_failure_code(&self) -> Option<CommentsTcpDelegationScheduleAuditSourceFailureCode> {
        self.last_failure_code
    }

    pub const fn reason(&self) -> &'static str {
        DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditRecoveryOutcome {
    Requeued {
        audit_id: Uuid,
        request_id: Uuid,
        recovery_epoch: i64,
    },
    NotFound,
    NotDeadLetter,
    StaleInspection,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditRecoveryError {
    #[error("invalid Comments schedule audit recovery request: {0}")]
    InvalidRequest(&'static str),
    #[error("invalid stored Comments schedule audit recovery state")]
    InvalidStoredState,
    #[error("Comments schedule audit recovery storage is unavailable")]
    Unavailable,
}

#[derive(Clone)]
pub struct PostgresCommentsTcpDelegationScheduleAuditRecoveryStore {
    database: DatabaseConnection,
}

impl PostgresCommentsTcpDelegationScheduleAuditRecoveryStore {
    pub fn new(database: DatabaseConnection) -> std::result::Result<Self, String> {
        if database.get_database_backend() != DbBackend::Postgres {
            return Err("Comments schedule audit recovery requires PostgreSQL".to_string());
        }
        Ok(Self { database })
    }

    pub async fn inspect_dead_letter(
        &self,
        request_id: Uuid,
    ) -> std::result::Result<
        Option<CommentsTcpDelegationScheduleAuditRecoveryInspection>,
        CommentsTcpDelegationScheduleAuditRecoveryError,
    > {
        if request_id.is_nil() {
            return Err(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                    "request ID must not be nil",
                ),
            );
        }
        self.database
            .query_one_raw(inspect_dead_letter_statement(request_id))
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable)?
            .map(|row| decode_inspection(&row))
            .transpose()
    }

    pub async fn requeue_dead_letter(
        &self,
        request: CommentsTcpDelegationScheduleAuditRecoveryRequest,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditRecoveryOutcome,
        CommentsTcpDelegationScheduleAuditRecoveryError,
    > {
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable)?;
        let result = requeue_in_transaction(&transaction, &request).await;
        let (audit_id, recovery_epoch) = match result {
            Ok(Some(values)) => values,
            Ok(None) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable)?;
                return Ok(CommentsTcpDelegationScheduleAuditRecoveryOutcome::NotFound);
            }
            Err(RequeueTransactionError::NotDeadLetter) => {
                let _ = transaction.rollback().await;
                return Ok(CommentsTcpDelegationScheduleAuditRecoveryOutcome::NotDeadLetter);
            }
            Err(RequeueTransactionError::StaleInspection) => {
                let _ = transaction.rollback().await;
                return Ok(CommentsTcpDelegationScheduleAuditRecoveryOutcome::StaleInspection);
            }
            Err(RequeueTransactionError::Recovery(error)) => {
                let _ = transaction.rollback().await;
                return Err(error);
            }
        };

        match transaction.commit().await {
            Ok(()) => Ok(
                CommentsTcpDelegationScheduleAuditRecoveryOutcome::Requeued {
                    audit_id,
                    request_id: request.request_id,
                    recovery_epoch,
                },
            ),
            Err(_) => {
                self.reconcile_requeue(audit_id, &request, recovery_epoch)
                    .await
            }
        }
    }

    async fn reconcile_requeue(
        &self,
        audit_id: Uuid,
        request: &CommentsTcpDelegationScheduleAuditRecoveryRequest,
        recovery_epoch: i64,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditRecoveryOutcome,
        CommentsTcpDelegationScheduleAuditRecoveryError,
    > {
        let row = self
            .database
            .query_one_raw(reconcile_requeue_statement(audit_id))
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable)?
            .ok_or(CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable)?;
        let stored_tenant: Uuid = row
            .try_get("", "control_plane_tenant_id")
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?;
        let stored_request: Uuid = row
            .try_get("", "request_id")
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?;
        let stored_actor: Uuid = row
            .try_get("", "actor_id")
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?;
        let stored_reason: String = row
            .try_get("", "reason")
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?;
        let stored_prior_attempt: i64 = row
            .try_get("", "prior_attempt_count")
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?;
        let stored_epoch: i64 = row
            .try_get("", "recovery_epoch")
            .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?;
        if stored_tenant != request.control_plane_tenant_id
            || stored_request != request.request_id
            || stored_actor != request.actor_id
            || stored_reason != request.reason
            || stored_prior_attempt != request.expected_attempt_count
            || stored_epoch != recovery_epoch
        {
            return Err(CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState);
        }
        Ok(
            CommentsTcpDelegationScheduleAuditRecoveryOutcome::Requeued {
                audit_id,
                request_id: request.request_id,
                recovery_epoch,
            },
        )
    }
}

impl fmt::Debug for PostgresCommentsTcpDelegationScheduleAuditRecoveryStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCommentsTcpDelegationScheduleAuditRecoveryStore")
            .field("database", &"[CONFIGURED]")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredRecoveryRow {
    request_id: Uuid,
    attempt_count: i64,
    recovery_epoch: i64,
    published: bool,
    canonical: bool,
    claimed: bool,
    claim_expiry_present: bool,
    deferred: bool,
    dead_lettered: bool,
    dead_letter_reason: Option<String>,
    last_failure_code: Option<String>,
}

impl StoredRecoveryRow {
    fn from_row(
        row: &QueryResult,
    ) -> std::result::Result<Self, CommentsTcpDelegationScheduleAuditRecoveryError> {
        Ok(Self {
            request_id: row
                .try_get("", "request_id")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            attempt_count: row
                .try_get("", "handoff_attempt_count")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            recovery_epoch: row
                .try_get("", "handoff_recovery_epoch")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            published: row
                .try_get("", "published")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            canonical: row
                .try_get("", "canonical")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            claimed: row
                .try_get("", "claimed")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            claim_expiry_present: row
                .try_get("", "claim_expiry_present")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            deferred: row
                .try_get("", "deferred")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            dead_lettered: row
                .try_get("", "dead_lettered")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            dead_letter_reason: row
                .try_get("", "handoff_dead_letter_reason")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
            last_failure_code: row
                .try_get("", "handoff_last_failure_code")
                .map_err(|_| CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState)?,
        })
    }

    fn is_exact_dead_letter(&self) -> bool {
        !self.published
            && !self.canonical
            && !self.claimed
            && !self.claim_expiry_present
            && !self.deferred
            && self.dead_lettered
            && self.dead_letter_reason.as_deref()
                == Some(DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED)
            && self.attempt_count > 0
            && self.recovery_epoch >= 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequeueTransactionError {
    NotDeadLetter,
    StaleInspection,
    Recovery(CommentsTcpDelegationScheduleAuditRecoveryError),
}

async fn requeue_in_transaction(
    transaction: &DatabaseTransaction,
    request: &CommentsTcpDelegationScheduleAuditRecoveryRequest,
) -> std::result::Result<Option<(Uuid, i64)>, RequeueTransactionError> {
    let row = transaction
        .query_one_raw(read_recovery_row_for_update_statement(request.request_id))
        .await
        .map_err(|_| {
            RequeueTransactionError::Recovery(
                CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable,
            )
        })?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored = StoredRecoveryRow::from_row(&row).map_err(RequeueTransactionError::Recovery)?;
    if stored.request_id != request.request_id || !stored.is_exact_dead_letter() {
        return Err(RequeueTransactionError::NotDeadLetter);
    }
    if stored.attempt_count != request.expected_attempt_count
        || stored.recovery_epoch != request.expected_recovery_epoch
    {
        return Err(RequeueTransactionError::StaleInspection);
    }
    let recovery_epoch =
        stored
            .recovery_epoch
            .checked_add(1)
            .ok_or(RequeueTransactionError::Recovery(
                CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState,
            ))?;
    let audit_id = Uuid::new_v4();
    let updated = transaction
        .execute_raw(requeue_source_statement(request, recovery_epoch))
        .await
        .map_err(|_| {
            RequeueTransactionError::Recovery(
                CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable,
            )
        })?
        .rows_affected();
    if updated != 1 {
        return Err(RequeueTransactionError::StaleInspection);
    }
    transaction
        .execute_raw(insert_recovery_audit_statement(
            audit_id,
            request,
            recovery_epoch,
        ))
        .await
        .map_err(|_| {
            RequeueTransactionError::Recovery(
                CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable,
            )
        })?;
    Ok(Some((audit_id, recovery_epoch)))
}

fn decode_inspection(
    row: &QueryResult,
) -> std::result::Result<
    CommentsTcpDelegationScheduleAuditRecoveryInspection,
    CommentsTcpDelegationScheduleAuditRecoveryError,
> {
    let stored = StoredRecoveryRow::from_row(row)?;
    if stored.request_id.is_nil() || !stored.is_exact_dead_letter() {
        return Err(CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState);
    }
    let last_failure_code = match stored.last_failure_code.as_deref() {
        None => None,
        Some("conflict") => Some(CommentsTcpDelegationScheduleAuditSourceFailureCode::Conflict),
        Some("unavailable") => {
            Some(CommentsTcpDelegationScheduleAuditSourceFailureCode::Unavailable)
        }
        Some(_) => {
            return Err(CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState);
        }
    };
    Ok(CommentsTcpDelegationScheduleAuditRecoveryInspection {
        request_id: stored.request_id,
        attempt_count: stored.attempt_count,
        recovery_epoch: stored.recovery_epoch,
        last_failure_code,
    })
}

fn validate_recovery_reason(
    reason: &str,
) -> std::result::Result<(), CommentsTcpDelegationScheduleAuditRecoveryError> {
    if reason.is_empty()
        || reason.trim() != reason
        || reason.len() > COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_MAX_REASON_BYTES
        || reason.chars().any(char::is_control)
    {
        return Err(
            CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(
                "reason must be trimmed, non-empty, control-free, and at most 512 UTF-8 bytes",
            ),
        );
    }
    Ok(())
}

fn recovery_row_projection() -> &'static str {
    "request_id, handoff_attempt_count, handoff_recovery_epoch, \
     published_at IS NOT NULL AS published, \
     canonical_envelope_id IS NOT NULL AS canonical, \
     handoff_claim_token IS NOT NULL AS claimed, \
     handoff_claim_expires_at IS NOT NULL AS claim_expiry_present, \
     handoff_next_attempt_at IS NOT NULL AS deferred, \
     handoff_dead_lettered_at IS NOT NULL AS dead_lettered, \
     handoff_dead_letter_reason, handoff_last_failure_code"
}

fn inspect_dead_letter_statement(request_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT {projection} FROM {table} \
             WHERE request_id = $1 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL \
               AND handoff_dead_lettered_at IS NOT NULL \
               AND handoff_dead_letter_reason = '{dead_letter_reason}' \
             LIMIT 1",
            projection = recovery_row_projection(),
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
            dead_letter_reason = DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED,
        ),
        vec![request_id.into()],
    )
}

fn read_recovery_row_for_update_statement(request_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT {projection} FROM {table} WHERE request_id = $1 FOR UPDATE",
            projection = recovery_row_projection(),
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![request_id.into()],
    )
}

fn requeue_source_statement(
    request: &CommentsTcpDelegationScheduleAuditRecoveryRequest,
    recovery_epoch: i64,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "UPDATE {table} \
             SET handoff_attempt_count = 0, \
                 handoff_recovery_epoch = $4, \
                 handoff_claim_token = NULL, \
                 handoff_claim_expires_at = NULL, \
                 handoff_next_attempt_at = NULL, \
                 handoff_last_failure_at = NULL, \
                 handoff_last_failure_code = NULL, \
                 handoff_dead_lettered_at = NULL, \
                 handoff_dead_letter_reason = NULL \
             WHERE request_id = $1 \
               AND handoff_attempt_count = $2 \
               AND handoff_recovery_epoch = $3 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL \
               AND handoff_claim_token IS NULL \
               AND handoff_claim_expires_at IS NULL \
               AND handoff_next_attempt_at IS NULL \
               AND handoff_dead_lettered_at IS NOT NULL \
               AND handoff_dead_letter_reason = '{dead_letter_reason}'",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
            dead_letter_reason = DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED,
        ),
        vec![
            request.request_id.into(),
            request.expected_attempt_count.into(),
            request.expected_recovery_epoch.into(),
            recovery_epoch.into(),
        ],
    )
}

fn insert_recovery_audit_statement(
    audit_id: Uuid,
    request: &CommentsTcpDelegationScheduleAuditRecoveryRequest,
    recovery_epoch: i64,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "INSERT INTO {table} ( \
                 audit_id, control_plane_tenant_id, request_id, actor_id, action, reason, \
                 prior_attempt_count, recovery_epoch \
             ) VALUES ($1, $2, $3, $4, '{action}', $5, $6, $7)",
            table = COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_TABLE,
            action = COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_ACTION,
        ),
        vec![
            audit_id.into(),
            request.control_plane_tenant_id.into(),
            request.request_id.into(),
            request.actor_id.into(),
            request.reason.clone().into(),
            request.expected_attempt_count.into(),
            recovery_epoch.into(),
        ],
    )
}

fn reconcile_requeue_statement(audit_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT control_plane_tenant_id, request_id, actor_id, reason, \
                    prior_attempt_count, recovery_epoch \
             FROM {table} WHERE audit_id = $1 AND action = '{action}'",
            table = COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_TABLE,
            action = COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_ACTION,
        ),
        vec![audit_id.into()],
    )
}
