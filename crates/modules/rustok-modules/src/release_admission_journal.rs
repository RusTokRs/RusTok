//! Durable release-admission intent journal for atomic staging and CAS publication.
//!
//! Enforces that artifact admission records an immutable intent before CAS mutation
//! and coordinates with reconcilers so staging, CAS publication, commit, and orphan
//! cleanup operate idempotently without orphaned staging residue.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, Statement, Value as SqlValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::{ArtifactAdmissionStage, ModuleCommandContext, ModuleInstallationScope};

/// Maximum number of stale unfinished intents returned by one scan;
/// reconcilers page by re-scanning after handling a batch.
pub const MAX_STALE_UNFINISHED_INTENTS_PER_SCAN: u32 = 500;

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

fn datetime_value(backend: SupportedBackend, value: &DateTime<Utc>) -> SqlValue {
    match backend {
        SupportedBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(value.to_owned())),
        SupportedBackend::Sqlite => value.to_rfc3339().into(),
    }
}

fn parse_uuid_column(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: SupportedBackend,
) -> Result<Uuid, ReleaseAdmissionJournalError> {
    match backend {
        SupportedBackend::Postgres => row
            .try_get::<Uuid>("", column)
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string())),
        SupportedBackend::Sqlite => row
            .try_get::<String>("", column)
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?
            .parse()
            .map_err(|e: uuid::Error| ReleaseAdmissionJournalError::Store(e.to_string())),
    }
}

fn parse_datetime_column(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: SupportedBackend,
) -> Result<DateTime<Utc>, ReleaseAdmissionJournalError> {
    match backend {
        SupportedBackend::Postgres => row
            .try_get::<DateTime<Utc>>("", column)
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string())),
        SupportedBackend::Sqlite => DateTime::parse_from_rfc3339(
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
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
        let (scope_kind, scope_tenant_key) = match scope {
            ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
        };
        let now = Utc::now();

        // 1. Check existing reservation
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2", "$3", "$4"),
            SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4"),
        };
        let existing = db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
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
                    backend.uuid_value(context.actor_id),
                    backend.uuid_value(context.idempotency_key),
                ],
            ))
            .await
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        if let Some(row) = existing {
            let stored_digest: String = row
                .try_get("", "request_digest")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            let installation_id: Option<Uuid> = match backend {
                SupportedBackend::Postgres => row
                    .try_get("", "installation_id")
                    .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?,
                SupportedBackend::Sqlite => row
                    .try_get::<Option<String>>("", "installation_id")
                    .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?
                    .map(|s| {
                        s.parse().map_err(|e: uuid::Error| {
                            ReleaseAdmissionJournalError::Store(e.to_string())
                        })
                    })
                    .transpose()?,
            };
            let committed_at = parse_datetime_column(&row, "committed_at", backend)?;

            if stored_digest != request_digest {
                return Err(ReleaseAdmissionJournalError::Conflict(
                    context.idempotency_key,
                    "Idempotency key was already used for a different admission request"
                        .to_string(),
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
            SupportedBackend::Postgres => (1..=8)
                .map(|i| format!("${i}"))
                .collect::<Vec<_>>()
                .join(", "),
            SupportedBackend::Sqlite => (1..=8)
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", "),
        };
        let inserted = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_admission_commands (\
                    scope_kind, scope_tenant_key, actor_id, idempotency_key, trace_id, correlation_id, request_digest, committed_at\
                 ) VALUES ({insert_placeholders}) ON CONFLICT DO NOTHING"
                ),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    backend.uuid_value(context.actor_id),
                    backend.uuid_value(context.idempotency_key),
                    context.trace_id.clone().into(),
                    backend.uuid_value(context.correlation_id),
                    request_digest.into(),
                    datetime_value(backend, &now),
                ],
            ))
            .await
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        if inserted.rows_affected() == 0 {
            // A concurrent reservation for the same (scope, actor, key) won
            // the insert between our check and insert. Re-read it and apply
            // the same digest/conflict decision — never fabricate a fresh
            // `Staged` record for a reservation we did not create.
            let row = db
                .query_one_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
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
                        backend.uuid_value(context.actor_id),
                        backend.uuid_value(context.idempotency_key),
                    ],
                ))
                .await
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?
                .ok_or_else(|| {
                    ReleaseAdmissionJournalError::Store(
                        "admission intent disappeared after insert conflict".to_string(),
                    )
                })?;
            let stored_digest: String = row
                .try_get("", "request_digest")
                .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
            if stored_digest != request_digest {
                return Err(ReleaseAdmissionJournalError::Conflict(
                    context.idempotency_key,
                    "Idempotency key was already used for a different admission request"
                        .to_string(),
                ));
            }
            let installation_id: Option<Uuid> = match backend {
                SupportedBackend::Postgres => row
                    .try_get("", "installation_id")
                    .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?,
                SupportedBackend::Sqlite => row
                    .try_get::<Option<String>>("", "installation_id")
                    .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?
                    .map(|s| {
                        s.parse().map_err(|e: uuid::Error| {
                            ReleaseAdmissionJournalError::Store(e.to_string())
                        })
                    })
                    .transpose()?,
            };
            let committed_at = parse_datetime_column(&row, "committed_at", backend)?;
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
    /// The update is scoped by the intent's full identity (scope, tenant,
    /// actor, idempotency key) — an idempotency key alone is caller-chosen
    /// data and must never be able to bind another scope's intent.
    pub async fn bind_committed_installation<C: ConnectionTrait>(
        db: &C,
        scope: &ModuleInstallationScope,
        context: &ModuleCommandContext,
        installation_id: Uuid,
    ) -> Result<bool, ReleaseAdmissionJournalError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
        let (scope_kind, scope_tenant_key) = match scope {
            ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
        };
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4", "?5"),
        };
        let result = db
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_admission_commands \
                     SET installation_id = {} \
                     WHERE scope_kind = {} AND scope_tenant_key = {} \
                       AND actor_id = {} AND idempotency_key = {} \
                       AND installation_id IS NULL",
                    placeholders.0, placeholders.1, placeholders.2, placeholders.3, placeholders.4
                ),
                vec![
                    backend.uuid_value(installation_id),
                    scope_kind.into(),
                    scope_tenant_key.into(),
                    backend.uuid_value(context.actor_id),
                    backend.uuid_value(context.idempotency_key),
                ],
            ))
            .await
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// Queries unfinished admission reservations older than the grace
    /// period, oldest first, bounded to
    /// [`MAX_STALE_UNFINISHED_INTENTS_PER_SCAN`] rows per scan.
    pub async fn scan_stale_unfinished_intents<C: ConnectionTrait>(
        db: &C,
        grace_period: Duration,
    ) -> Result<Vec<ReleaseAdmissionIntentRecord>, ReleaseAdmissionJournalError> {
        let backend = SupportedBackend::new(db.get_database_backend())
            .map_err(|e| ReleaseAdmissionJournalError::Store(e.to_string()))?;
        let threshold = Utc::now() - grace_period;
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT scope_kind, scope_tenant_key, actor_id, idempotency_key, \
                            trace_id, correlation_id, request_digest, committed_at \
                     FROM module_artifact_admission_commands \
                     WHERE installation_id IS NULL AND committed_at <= {} \
                     ORDER BY committed_at ASC LIMIT {}",
                    if backend == SupportedBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    },
                    MAX_STALE_UNFINISHED_INTENTS_PER_SCAN
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
