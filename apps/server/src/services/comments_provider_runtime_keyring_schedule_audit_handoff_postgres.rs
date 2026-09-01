use std::{fmt, time::Duration};

use rustok_api::AuthPrincipalKind;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
};
use uuid::Uuid;

use super::{
    keyring,
    keyring_schedule_audit_publication::{
        CommentsTcpDelegationScheduleAuditCanonicalPublication,
        CommentsTcpDelegationScheduleAuditCanonicalWriteError,
        SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
    },
    keyring_schedule_persistence_postgres as postgres,
    keyring_schedule_persistence_postgres_audit as postgres_audit,
    keyring_schedule_trigger as trigger,
};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS: u64 = 300;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationScheduleAuditHandoffError {
    Conflict,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditHandoffClaim {
    request_id: Uuid,
    claim_token: Uuid,
    attempt_count: i64,
}

impl CommentsTcpDelegationScheduleAuditHandoffClaim {
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn claim_token(&self) -> Uuid {
        self.claim_token
    }

    pub fn attempt_count(&self) -> i64 {
        self.attempt_count
    }
}

#[derive(Clone)]
pub struct PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff {
    database: DatabaseConnection,
    control_plane_tenant_id: Uuid,
    writer: SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
    claim_ttl_seconds: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredAuditHandoffRecord {
    audit_schema_version: i16,
    request_id: Uuid,
    state_key: String,
    event_type: String,
    occurred_at_unix_ms: i64,
    actor_id: Uuid,
    principal_kind: String,
    operation: String,
    source: String,
    previous_generation: i64,
    candidate_generation: i64,
    outcome: String,
    canonical_envelope_id: Option<Uuid>,
    published: bool,
    claim_token: Option<Uuid>,
    claim_attempt_count: i64,
    claim_active: bool,
}

impl PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff {
    pub fn new(
        database: DatabaseConnection,
        control_plane_tenant_id: Uuid,
        writer: SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
        claim_ttl: Duration,
    ) -> std::result::Result<Self, String> {
        if database.get_database_backend() != DbBackend::Postgres {
            return Err(
                "Comments schedule audit canonical handoff requires PostgreSQL".to_string(),
            );
        }
        if control_plane_tenant_id.is_nil() {
            return Err(
                "Comments schedule audit canonical handoff requires a non-nil control-plane tenant ID"
                    .to_string(),
            );
        }
        let claim_ttl_seconds = claim_ttl.as_secs();
        if claim_ttl_seconds == 0
            || claim_ttl_seconds > COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS
            || claim_ttl != Duration::from_secs(claim_ttl_seconds)
        {
            return Err(format!(
                "Comments schedule audit canonical handoff claim TTL must be a whole second in 1..={COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS}"
            ));
        }
        let claim_ttl_seconds = i64::try_from(claim_ttl_seconds).map_err(|_| {
            "Comments schedule audit canonical handoff claim TTL is out of range".to_string()
        })?;
        Ok(Self {
            database,
            control_plane_tenant_id,
            writer,
            claim_ttl_seconds,
        })
    }

    /// Claims one unpublished Blog audit row without waiting behind another
    /// claimant. Expired claims are eligible for bounded recovery.
    pub async fn claim_next(
        &self,
    ) -> std::result::Result<
        Option<CommentsTcpDelegationScheduleAuditHandoffClaim>,
        CommentsTcpDelegationScheduleAuditHandoffError,
    > {
        let claim_token = Uuid::new_v4();
        let transaction = self.database.begin().await.map_err(unavailable)?;
        let row = transaction
            .query_one_raw(claim_next_statement(claim_token, self.claim_ttl_seconds))
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(unavailable)?;
            return Ok(None);
        };
        let claim = decode_claim(&row, claim_token)?;
        match transaction.commit().await {
            Ok(()) => Ok(Some(claim)),
            Err(error) => {
                tracing::warn!(error = %error, claim_token = %claim_token, "Comments schedule audit claim commit acknowledgement was ambiguous");
                self.reconcile_claim(claim_token).await.map(Some)
            }
        }
    }

    /// Writes the canonical event and marks the exact Blog audit row published
    /// in one caller transaction. The source claim token and attempt count fence
    /// stale workers. Exact terminal replay returns the stored envelope UUID.
    pub async fn publish_claimed(
        &self,
        claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
    ) -> std::result::Result<Uuid, CommentsTcpDelegationScheduleAuditHandoffError> {
        validate_claim(claim)?;
        let transaction = self.database.begin().await.map_err(unavailable)?;
        let row = transaction
            .query_one_raw(read_for_publish_statement(claim.request_id))
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            let _ = transaction.rollback().await;
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Conflict);
        };
        let stored = StoredAuditHandoffRecord::from_row(&row)?;

        if stored.is_exact_terminal() {
            let _ = transaction.rollback().await;
            return Ok(claim.request_id);
        }
        if stored.has_invalid_terminal_pair() {
            let _ = transaction.rollback().await;
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
        }
        if !stored.matches_claim(claim) {
            let _ = transaction.rollback().await;
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Conflict);
        }

        let publication = stored.publication(self.control_plane_tenant_id)?;
        let canonical_envelope_id = match self
            .writer
            .write_once_in_transaction(&transaction, &publication)
            .await
        {
            Ok(envelope_id) if envelope_id == claim.request_id => envelope_id,
            Ok(_) => {
                let _ = transaction.rollback().await;
                return Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
            }
            Err(error) => {
                let mapped = map_writer_error(error);
                let _ = transaction.rollback().await;
                return Err(mapped);
            }
        };

        let updated = transaction
            .execute_raw(mark_published_statement(claim))
            .await
            .map_err(unavailable)?
            .rows_affected();
        if updated != 1 {
            let _ = transaction.rollback().await;
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Conflict);
        }

        match transaction.commit().await {
            Ok(()) => Ok(canonical_envelope_id),
            Err(error) => {
                tracing::warn!(error = %error, request_id = %claim.request_id, "Comments schedule audit publication commit acknowledgement was ambiguous");
                self.reconcile_publication(claim.request_id).await
            }
        }
    }

    pub async fn publish_next(
        &self,
    ) -> std::result::Result<Option<Uuid>, CommentsTcpDelegationScheduleAuditHandoffError> {
        let Some(claim) = self.claim_next().await? else {
            return Ok(None);
        };
        self.publish_claimed(claim).await.map(Some)
    }

    async fn reconcile_claim(
        &self,
        claim_token: Uuid,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditHandoffClaim,
        CommentsTcpDelegationScheduleAuditHandoffError,
    > {
        let row = self
            .database
            .query_one_raw(read_claim_statement(claim_token))
            .await
            .map_err(unavailable)?
            .ok_or(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable)?;
        let request_id: Uuid = row.try_get("", "request_id").map_err(unavailable)?;
        let attempt_count: i64 = row
            .try_get("", "handoff_attempt_count")
            .map_err(unavailable)?;
        let claim_active: bool = row.try_get("", "claim_active").map_err(unavailable)?;
        let claim = CommentsTcpDelegationScheduleAuditHandoffClaim {
            request_id,
            claim_token,
            attempt_count,
        };
        validate_claim(claim)?;
        if !claim_active {
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
        }
        Ok(claim)
    }

    async fn reconcile_publication(
        &self,
        request_id: Uuid,
    ) -> std::result::Result<Uuid, CommentsTcpDelegationScheduleAuditHandoffError> {
        let row = self
            .database
            .query_one_raw(read_publication_statement(request_id))
            .await
            .map_err(unavailable)?
            .ok_or(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable)?;
        let canonical_envelope_id: Option<Uuid> = row
            .try_get("", "canonical_envelope_id")
            .map_err(unavailable)?;
        let published: bool = row.try_get("", "published").map_err(unavailable)?;
        if published && canonical_envelope_id == Some(request_id) {
            Ok(request_id)
        } else {
            Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable)
        }
    }
}

impl fmt::Debug for PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff")
            .field("database", &"[CONFIGURED]")
            .field("control_plane_tenant_id", &self.control_plane_tenant_id)
            .field("writer", &"[CONFIGURED]")
            .field("claim_ttl_seconds", &self.claim_ttl_seconds)
            .finish()
    }
}

impl StoredAuditHandoffRecord {
    fn from_row(
        row: &QueryResult,
    ) -> std::result::Result<Self, CommentsTcpDelegationScheduleAuditHandoffError> {
        Ok(Self {
            audit_schema_version: row
                .try_get("", "audit_schema_version")
                .map_err(unavailable)?,
            request_id: row.try_get("", "request_id").map_err(unavailable)?,
            state_key: row.try_get("", "state_key").map_err(unavailable)?,
            event_type: row.try_get("", "event_type").map_err(unavailable)?,
            occurred_at_unix_ms: row
                .try_get("", "occurred_at_unix_ms")
                .map_err(unavailable)?,
            actor_id: row.try_get("", "actor_id").map_err(unavailable)?,
            principal_kind: row.try_get("", "principal_kind").map_err(unavailable)?,
            operation: row.try_get("", "operation").map_err(unavailable)?,
            source: row.try_get("", "source").map_err(unavailable)?,
            previous_generation: row
                .try_get("", "previous_generation")
                .map_err(unavailable)?,
            candidate_generation: row
                .try_get("", "candidate_generation")
                .map_err(unavailable)?,
            outcome: row.try_get("", "outcome").map_err(unavailable)?,
            canonical_envelope_id: row
                .try_get("", "canonical_envelope_id")
                .map_err(unavailable)?,
            published: row.try_get("", "published").map_err(unavailable)?,
            claim_token: row
                .try_get("", "handoff_claim_token")
                .map_err(unavailable)?,
            claim_attempt_count: row
                .try_get("", "handoff_attempt_count")
                .map_err(unavailable)?,
            claim_active: row.try_get("", "claim_active").map_err(unavailable)?,
        })
    }

    fn is_exact_terminal(&self) -> bool {
        self.published
            && self.canonical_envelope_id == Some(self.request_id)
            && self.claim_token.is_none()
    }

    fn has_invalid_terminal_pair(&self) -> bool {
        self.published != self.canonical_envelope_id.is_some()
            || self
                .canonical_envelope_id
                .is_some_and(|envelope_id| envelope_id != self.request_id)
    }

    fn matches_claim(&self, claim: CommentsTcpDelegationScheduleAuditHandoffClaim) -> bool {
        !self.published
            && self.canonical_envelope_id.is_none()
            && self.claim_token == Some(claim.claim_token)
            && self.claim_attempt_count == claim.attempt_count
            && self.claim_active
    }

    fn publication(
        &self,
        control_plane_tenant_id: Uuid,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditCanonicalPublication,
        CommentsTcpDelegationScheduleAuditHandoffError,
    > {
        if self.audit_schema_version
            != i16::try_from(
                postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_SCHEMA_VERSION,
            )
            .map_err(unavailable)?
            || self.request_id.is_nil()
            || self.actor_id.is_nil()
            || self.state_key != postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY
            || self.event_type
                != postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE
            || self.occurred_at_unix_ms <= 0
            || self.previous_generation <= 0
            || self.candidate_generation <= self.previous_generation
            || self.outcome != "replacement_succeeded"
        {
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
        }
        let principal_kind = principal_kind_from_text(self.principal_kind.as_str())
            .ok_or(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable)?;
        let operation = operation_from_text(self.operation.as_str())
            .ok_or(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable)?;
        let source = source_from_text(self.source.as_str())
            .ok_or(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable)?;
        CommentsTcpDelegationScheduleAuditCanonicalPublication::new(
            control_plane_tenant_id,
            self.request_id,
            self.actor_id,
            principal_kind,
            operation,
            source,
            u64::try_from(self.occurred_at_unix_ms).map_err(unavailable)?,
            u64::try_from(self.previous_generation).map_err(unavailable)?,
            u64::try_from(self.candidate_generation).map_err(unavailable)?,
        )
        .map_err(unavailable)
    }
}

fn validate_claim(
    claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
) -> std::result::Result<(), CommentsTcpDelegationScheduleAuditHandoffError> {
    if claim.request_id.is_nil() || claim.claim_token.is_nil() || claim.attempt_count <= 0 {
        return Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
    }
    Ok(())
}

fn decode_claim(
    row: &QueryResult,
    claim_token: Uuid,
) -> std::result::Result<
    CommentsTcpDelegationScheduleAuditHandoffClaim,
    CommentsTcpDelegationScheduleAuditHandoffError,
> {
    let claim = CommentsTcpDelegationScheduleAuditHandoffClaim {
        request_id: row.try_get("", "request_id").map_err(unavailable)?,
        claim_token,
        attempt_count: row
            .try_get("", "handoff_attempt_count")
            .map_err(unavailable)?,
    };
    validate_claim(claim)?;
    Ok(claim)
}

const fn map_writer_error(
    error: CommentsTcpDelegationScheduleAuditCanonicalWriteError,
) -> CommentsTcpDelegationScheduleAuditHandoffError {
    match error {
        CommentsTcpDelegationScheduleAuditCanonicalWriteError::Conflict => {
            CommentsTcpDelegationScheduleAuditHandoffError::Conflict
        }
        CommentsTcpDelegationScheduleAuditCanonicalWriteError::Unavailable => {
            CommentsTcpDelegationScheduleAuditHandoffError::Unavailable
        }
    }
}

fn unavailable(error: impl fmt::Display) -> CommentsTcpDelegationScheduleAuditHandoffError {
    tracing::error!(error = %error, "Comments schedule audit canonical handoff is unavailable");
    CommentsTcpDelegationScheduleAuditHandoffError::Unavailable
}

fn principal_kind_from_text(value: &str) -> Option<AuthPrincipalKind> {
    match value {
        "direct_user" => Some(AuthPrincipalKind::DirectUser),
        "service" => Some(AuthPrincipalKind::Service),
        _ => None,
    }
}

fn operation_from_text(
    value: &str,
) -> Option<trigger::CommentsTcpDelegationScheduleTriggerOperation> {
    match value {
        "reload_file" => Some(trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile),
        "replace_host_schedule" => {
            Some(trigger::CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule)
        }
        _ => None,
    }
}

fn source_from_text(value: &str) -> Option<keyring::CommentsTcpDelegationKeyringSource> {
    match value {
        "host_provided" => Some(keyring::CommentsTcpDelegationKeyringSource::HostProvided),
        "file" => Some(keyring::CommentsTcpDelegationKeyringSource::File),
        _ => None,
    }
}

fn claim_next_statement(claim_token: Uuid, claim_ttl_seconds: i64) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "WITH candidate AS ( \
                 SELECT request_id \
                 FROM {table} \
                 WHERE published_at IS NULL \
                   AND canonical_envelope_id IS NULL \
                   AND (handoff_claim_token IS NULL OR handoff_claim_expires_at <= NOW()) \
                 ORDER BY created_at ASC, request_id ASC \
                 FOR UPDATE SKIP LOCKED \
                 LIMIT 1 \
             ) \
             UPDATE {table} AS audit \
             SET handoff_claim_token = $1, \
                 handoff_claim_expires_at = NOW() + ($2::bigint * INTERVAL '1 second'), \
                 handoff_attempt_count = handoff_attempt_count + 1 \
             FROM candidate \
             WHERE audit.request_id = candidate.request_id \
             RETURNING audit.request_id, audit.handoff_attempt_count",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![claim_token.into(), claim_ttl_seconds.into()],
    )
}

fn read_claim_statement(claim_token: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT request_id, handoff_attempt_count, \
                    COALESCE(handoff_claim_expires_at > NOW(), FALSE) AS claim_active \
             FROM {table} \
             WHERE handoff_claim_token = $1 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![claim_token.into()],
    )
}

fn read_for_publish_statement(request_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT audit_schema_version, request_id, state_key, event_type, \
                    occurred_at_unix_ms, actor_id, principal_kind, operation, source, \
                    previous_generation, candidate_generation, outcome, \
                    canonical_envelope_id, published_at IS NOT NULL AS published, \
                    handoff_claim_token, handoff_attempt_count, \
                    COALESCE(handoff_claim_expires_at > NOW(), FALSE) AS claim_active \
             FROM {table} \
             WHERE request_id = $1 \
             FOR UPDATE",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![request_id.into()],
    )
}

fn mark_published_statement(claim: CommentsTcpDelegationScheduleAuditHandoffClaim) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "UPDATE {table} \
             SET canonical_envelope_id = request_id, \
                 published_at = NOW(), \
                 handoff_claim_token = NULL, \
                 handoff_claim_expires_at = NULL \
             WHERE request_id = $1 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL \
               AND handoff_claim_token = $2 \
               AND handoff_attempt_count = $3 \
               AND handoff_claim_expires_at > NOW()",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![
            claim.request_id.into(),
            claim.claim_token.into(),
            claim.attempt_count.into(),
        ],
    )
}

fn read_publication_statement(request_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "SELECT canonical_envelope_id, published_at IS NOT NULL AS published \
             FROM {table} WHERE request_id = $1",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![request_id.into()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_value_parsers_reject_unknown_contract_values() {
        assert_eq!(
            principal_kind_from_text("service"),
            Some(AuthPrincipalKind::Service)
        );
        assert!(principal_kind_from_text("delegated_user").is_none());
        assert!(operation_from_text("unknown").is_none());
        assert!(source_from_text("unknown").is_none());
    }

    #[test]
    fn claim_requires_non_nil_fencing_identity_and_positive_attempt() {
        assert!(
            validate_claim(CommentsTcpDelegationScheduleAuditHandoffClaim {
                request_id: Uuid::nil(),
                claim_token: Uuid::new_v4(),
                attempt_count: 1,
            })
            .is_err()
        );
        assert!(
            validate_claim(CommentsTcpDelegationScheduleAuditHandoffClaim {
                request_id: Uuid::new_v4(),
                claim_token: Uuid::new_v4(),
                attempt_count: 0,
            })
            .is_err()
        );
    }

    #[test]
    fn claim_sql_is_skip_locked_and_expiry_bounded() {
        let statement = claim_next_statement(Uuid::new_v4(), 60);
        let sql = statement.sql.as_str();
        assert!(sql.contains("FOR UPDATE SKIP LOCKED"));
        assert!(sql.contains("handoff_claim_expires_at <= NOW()"));
        assert!(sql.contains("handoff_attempt_count = handoff_attempt_count + 1"));
    }
}
