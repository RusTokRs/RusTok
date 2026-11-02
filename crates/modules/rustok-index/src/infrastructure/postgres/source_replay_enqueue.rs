//! Atomic original-identity rebuild requests. No source read, worker lease or authorization.

use super::{
    Duration, IndexReplayJobError, IndexReplayJobLeaseRequest, PostgresIndexReplayJobStore,
    SqlValue, Statement, Uuid, lock_replay_scope, replay_job_request, replay_scope_values,
    require_complete_checkpoint_for_scope, select_replay_job_history_sql, storage_error,
    stored_job, uuid_value, validate_source_name, verify_schema_registration,
};
use crate::SchemaRef;
use sea_orm::{ConnectionTrait, DatabaseTransaction, TransactionTrait};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexReplayJobEnqueueRequest {
    tenant_id: Uuid,
    job_id: Uuid,
    schema: SchemaRef,
    source_name: String,
}

impl IndexReplayJobEnqueueRequest {
    pub fn new(
        tenant_id: Uuid,
        job_id: Uuid,
        schema: SchemaRef,
        source_name: impl Into<String>,
    ) -> Result<Self, IndexReplayJobError> {
        if tenant_id.is_nil() {
            return Err(IndexReplayJobError::NilTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexReplayJobError::NilJobId);
        }
        let source_name = source_name.into();
        validate_source_name(&source_name)?;
        Ok(Self {
            tenant_id,
            job_id,
            schema,
            source_name,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }
    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    fn scope_request(&self) -> Result<IndexReplayJobLeaseRequest, IndexReplayJobError> {
        // A scope-validation label only. Enqueue never acquires/persists a lease or principal.
        IndexReplayJobLeaseRequest::new(
            self.tenant_id,
            self.schema.clone(),
            &self.source_name,
            "index-replay-enqueue",
            Duration::from_secs(1),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexReplayJobEnqueueOutcome {
    Enqueued { job_id: Uuid },
    AlreadyRequested { job_id: Uuid },
    AlreadyComplete { job_id: Uuid },
}

impl PostgresIndexReplayJobStore {
    /// Commits an exact pending job before returning, or proves that same durable request.
    /// Repeated activation cannot rotate identity, advance an attempt or reset a checkpoint.
    pub async fn enqueue(
        &self,
        request: &IndexReplayJobEnqueueRequest,
    ) -> Result<IndexReplayJobEnqueueOutcome, IndexReplayJobError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self.enqueue_in_transaction(&transaction, request).await;
        match result {
            Ok(outcome) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction.rollback().await.map_err(storage_error)?;
                Err(error)
            }
        }
    }

    async fn enqueue_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &IndexReplayJobEnqueueRequest,
    ) -> Result<IndexReplayJobEnqueueOutcome, IndexReplayJobError> {
        let backend = transaction.get_database_backend();
        super::ensure_supported_backend(backend)?;
        let scope = request.scope_request()?;
        lock_replay_scope(transaction, &scope, backend).await?;
        verify_schema_registration(transaction, &scope, backend).await?;

        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                // With a unique (tenant, job UUID), any second row proves a conflicting
                // original identity; two rows suffice even for long cancelled history.
                format!("{} LIMIT 2", select_replay_job_history_sql(backend, true)),
                replay_scope_values(&scope, backend),
            ))
            .await
            .map_err(storage_error)?;
        for row in &rows {
            let saved = stored_job(row, backend)?;
            if saved.job_id != request.job_id || saved.source_name != request.source_name {
                return Err(IndexReplayJobError::JobIdentityConflict {
                    expected: request.job_id,
                    actual: saved.job_id,
                });
            }
        }
        // Include terminal/cancelled history and other kinds: an original UUID is never reset.
        let existing = transaction.query_one_raw(Statement::from_sql_and_values(
            backend,
            "SELECT kind, state, cancel_requested, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, scope_kind, locale_key, request, last_error_code, CAST(attempt_count AS BIGINT) AS attempt_count_value, FALSE AS claimable, job_id FROM index_jobs WHERE tenant_id = $1 AND job_id = $2",
            vec![uuid_value(request.tenant_id, backend), uuid_value(request.job_id, backend)],
        )).await.map_err(storage_error)?;
        if let Some(row) = existing {
            let kind: String = row.try_get("", "kind").map_err(storage_error)?;
            let module: String = row.try_get("", "module_name").map_err(storage_error)?;
            let entity: String = row.try_get("", "entity_name").map_err(storage_error)?;
            let version: i64 = row
                .try_get("", "schema_version_value")
                .map_err(storage_error)?;
            let saved = stored_job(&row, backend)?;
            if kind != "rebuild"
                || module != request.schema.module.as_str()
                || entity != request.schema.entity.as_str()
                || version != i64::from(request.schema.version.get())
                || saved.locale.is_some()
                || saved.source_name != request.source_name
            {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "original replay UUID is bound to another exact scope".into(),
                ));
            }
            let cancelled: bool = row.try_get("", "cancel_requested").map_err(storage_error)?;
            if cancelled {
                return Err(IndexReplayJobError::InvalidStoredJob(
                    "original replay cancellation is requested; recover it without replacement"
                        .into(),
                ));
            }
            return match saved.state.as_str() {
                "pending" | "running" => Ok(IndexReplayJobEnqueueOutcome::AlreadyRequested { job_id: saved.job_id }),
                "succeeded" => {
                    require_complete_checkpoint_for_scope(transaction, &scope, backend).await?;
                    Ok(IndexReplayJobEnqueueOutcome::AlreadyComplete { job_id: saved.job_id })
                }
                "failed" => Err(IndexReplayJobError::DeadLettered { job_id: saved.job_id, attempt_count: saved.attempt_count, error_code: saved.last_error_code }),
                _ => Err(IndexReplayJobError::InvalidStoredJob("original replay request is terminal or unverified; recover it without replacement".into())),
            };
        }
        if !rows.is_empty() {
            return Err(IndexReplayJobError::InvalidStoredJob(
                "replay scope and original job UUID disagree".into(),
            ));
        }
        // Scope serialization also fences native checkpoint writers through their retained
        // job lease. A checkpoint without any original job is inconsistent history, not a
        // completion certificate for a freshly inserted initial replay.
        let orphan = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                super::complete_checkpoint_sql(backend),
                super::replay_checkpoint_scope_values(&scope, backend),
            ))
            .await
            .map_err(storage_error)?;
        if orphan.is_some() {
            return Err(IndexReplayJobError::CheckpointWithoutJob);
        }
        let inserted = transaction.execute_raw(Statement::from_sql_and_values(
            backend,
            "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, locale_key, request, attempt_count, available_at) VALUES ($1, $2, 'rebuild', 'pending', 'schema', $3, $4, $5, NULL, $6, 0, CURRENT_TIMESTAMP)",
            vec![
                uuid_value(request.tenant_id, backend), uuid_value(request.job_id, backend),
                request.schema.module.as_str().to_owned().into(), request.schema.entity.as_str().to_owned().into(),
                i64::from(request.schema.version.get()).into(), SqlValue::Json(Some(Box::new(replay_job_request(&scope)))),
            ],
        )).await.map_err(storage_error)?;
        if inserted.rows_affected() != 1 {
            return Err(IndexReplayJobError::InvalidStoredJob(
                "replay enqueue did not persist its exact original job".into(),
            ));
        }
        Ok(IndexReplayJobEnqueueOutcome::Enqueued {
            job_id: request.job_id,
        })
    }
}
