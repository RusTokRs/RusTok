//! Durable release-admission intent journal for atomic staging and CAS publication.
//!
//! Enforces that artifact admission records an immutable intent before CAS mutation
//! and coordinates with reconcilers so staging, CAS publication, commit, and orphan
//! cleanup operate idempotently without orphaned staging residue.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value as SqlValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactAdmissionStage, ModuleCommandContext, ModuleInstallationScope,
};

#[derive(Debug, Error)]
pub enum ReleaseAdmissionJournalError {
    #[error("Database store error: {0}")]
    Store(String),
    #[error("Intent already committed for operation `{0}`")]
    AlreadyCommitted(Uuid),
    #[error("Intent conflict for idempotency key `{0}`: {1}")]
    Conflict(Uuid, String),
}

/// Durable record of an in-flight or completed release admission intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAdmissionIntentRecord {
    pub scope_kind: String,
    pub scope_tenant_key: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
    pub trace_id: String,
    pub correlation_id: Uuid,
    pub request_digest: String,
    pub installation_id: Option<Uuid>,
    pub stage: ArtifactAdmissionStage,
    pub committed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct ReleaseAdmissionIntentJournal;

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(value)),
        _ => value.to_string().into(),
    }
}

fn datetime_value(backend: DbBackend, value: &DateTime<Utc>) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(value.to_owned())),
        _ => value.to_rfc3339().into(),
    }
}

fn parse_uuid_column(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ReleaseAdmissionJournalError> {
    match backend {
        DbBackend::Postgres => row
            .try_get::<Uuid>("", column)
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string())),
        _ => row
            .try_get::<String>("", column)
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?
            .parse()
            .map_err(|e: uuid::Error| ReleaseAdmissionJournalError::Store(e.to_string())),
    }
}

fn parse_datetime_column(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<DateTime<Utc>, ReleaseAdmissionJournalError> {
    match backend {
        DbBackend::Postgres => row
            .try_get::<DateTime<Utc>>("", column)
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string())),
        _ => DateTime::parse_from_rfc3339(
            &row.try_get::<String>("", column)
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?,
        )
        .map_err(|_| ReleaseAdmissionJournalError::Store("Invalid timestamp in column".into()))
        .map(|dt| dt.with_timezone(&Utc)),
    }
}

impl ReleaseAdmissionIntentJournal {
    /// Records an admission intent reservation before staging or CAS publication.
    ///
    /// If a reservation already exists for the same actor and idempotency key:
    /// - If `request_digest` matches, returns the existing intent record.
    /// - If `request_digest` differs, returns a `Conflict` error.
    pub async fn record_staging_intent<C: ConnectionTrait>(
        db: &C,
        scope: &ModuleInstallationScope,
        context: &ModuleCommandContext,
        request_digest: &str,
    ) -> Result<ReleaseAdmissionIntentRecord, ReleaseAdmissionJournalError> {
        let backend = db.get_database_backend();
        let (scope_kind, scope_tenant_key) = match scope {
            ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => {
                ("tenant", tenant_id.to_string())
            }
        };
        let now = Utc::now();

        // 1. Check existing reservation
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4"),
            _ => ("?1", "?2", "?3", "?4"),
        };
        let existing = db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request_digest, installation_id, committed_at \
                     FROM module_artifact_admission_commands \
                     WHERE scope_kind = {} AND scope_tenant_key = {} \
                       AND actor_id = {} AND idempotency_key = {}",
                    placeholders.0, placeholders.1, placeholders.2, placeholders.3
                ),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    uuid_value(context.actor_id, backend),
                    uuid_value(context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        if let Some(row) = existing {
            let stored_digest: String = row
                .try_get("", "request_digest")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            let installation_id: Option<Uuid> = match backend {
                DbBackend::Postgres => row
                    .try_get("", "installation_id")
                    .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?,
                _ => row
                    .try_get::<Option<String>>("", "installation_id")
                    .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?
                    .map(|s| s.parse().map_err(|e: uuid::Error| ReleaseAdmissionJournalError::Store(e.to_string())))
                    .transpose()?,
            };
            let committed_at = parse_datetime_column(&row, "committed_at", backend)?;

            if stored_digest != request_digest {
                return Err(ReleaseAdmissionJournalError::Conflict(
                    context.idempotency_key,
                    "Idempotency key was already used for a different admission request".to_string(),
                ));
            }

            let stage = if installation_id.is_some() {
                ArtifactAdmissionStage::DbCommitted
            } else {
                ArtifactAdmissionStage::Staged
            };

            return Ok(ReleaseAdmissionIntentRecord {
                scope_kind: scope_kind.to_string(),
                scope_tenant_key,
                actor_id: context.actor_id,
                idempotency_key: context.idempotency_key,
                trace_id: context.trace_id.clone(),
                correlation_id: context.correlation_id,
                request_digest: stored_digest,
                installation_id,
                stage,
                committed_at,
            });
        }

        // 2. Insert new reservation
        let insert_placeholders = match backend {
            DbBackend::Postgres => (1..=8).map(|i| format!("${i}")).collect::<Vec<_>>().join(", "),
            _ => (1..=8).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", "),
        };
        db.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_admission_commands (\
                    scope_kind, scope_tenant_key, actor_id, idempotency_key, trace_id, correlation_id, request_digest, committed_at\
                 ) VALUES ({insert_placeholders}) ON CONFLICT DO NOTHING"
            ),
            vec![
                scope_kind.into(),
                scope_tenant_key.clone().into(),
                uuid_value(context.actor_id, backend),
                uuid_value(context.idempotency_key, backend),
                context.trace_id.clone().into(),
                uuid_value(context.correlation_id, backend),
                request_digest.into(),
                datetime_value(backend, &now),
            ],
        ))
        .await
        .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        Ok(ReleaseAdmissionIntentRecord {
            scope_kind: scope_kind.to_string(),
            scope_tenant_key,
            actor_id: context.actor_id,
            idempotency_key: context.idempotency_key,
            trace_id: context.trace_id.clone(),
            correlation_id: context.correlation_id,
            request_digest: request_digest.to_string(),
            installation_id: None,
            stage: ArtifactAdmissionStage::Staged,
            committed_at: now,
        })
    }

    /// Marks the admission intent as committed to an installed artifact row.
    pub async fn bind_committed_installation<C: ConnectionTrait>(
        db: &C,
        idempotency_key: Uuid,
        installation_id: Uuid,
    ) -> Result<bool, ReleaseAdmissionJournalError> {
        let backend = db.get_database_backend();
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2"),
            _ => ("?1", "?2"),
        };
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_admission_commands \
                     SET installation_id = {} \
                     WHERE idempotency_key = {} AND installation_id IS NULL",
                    placeholders.0, placeholders.1
                ),
                vec![
                    uuid_value(installation_id, backend),
                    uuid_value(idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// Queries all unfinished admission reservations older than the grace period.
    pub async fn scan_stale_unfinished_intents<C: ConnectionTrait>(
        db: &C,
        grace_period: Duration,
    ) -> Result<Vec<ReleaseAdmissionIntentRecord>, ReleaseAdmissionJournalError> {
        let backend = db.get_database_backend();
        let threshold = Utc::now() - grace_period;
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT scope_kind, scope_tenant_key, actor_id, idempotency_key, \
                            trace_id, correlation_id, request_digest, committed_at \
                     FROM module_artifact_admission_commands \
                     WHERE installation_id IS NULL AND committed_at <= {}",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![datetime_value(backend, &threshold)],
            ))
            .await
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let scope_kind: String = row
                .try_get("", "scope_kind")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            let scope_tenant_key: String = row
                .try_get("", "scope_tenant_key")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            let actor_id = parse_uuid_column(&row, "actor_id", backend)?;
            let idempotency_key = parse_uuid_column(&row, "idempotency_key", backend)?;
            let trace_id: String = row
                .try_get("", "trace_id")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            let correlation_id = parse_uuid_column(&row, "correlation_id", backend)?;
            let request_digest: String = row
                .try_get("", "request_digest")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            let committed_at = parse_datetime_column(&row, "committed_at", backend)?;

            records.push(ReleaseAdmissionIntentRecord {
                scope_kind,
                scope_tenant_key,
                actor_id,
                idempotency_key,
                trace_id,
                correlation_id,
                request_digest,
                installation_id: None,
                stage: ArtifactAdmissionStage::Staged,
                committed_at,
            });
        }

        Ok(records)
    }
}
