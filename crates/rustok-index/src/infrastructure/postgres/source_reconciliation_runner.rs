use std::{collections::BTreeSet, sync::Arc, time::Duration};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    IndexReplayFailure, IndexReplayFailureKind, IndexReplayMutationOutcome,
    IndexReplayMutationSink, IndexSourceCursor, IndexSourceError, IndexSourceFailureKind,
    IndexSourceScanRequest, SchemaRef, SchemaRegistry, SharedIndexSourceRegistry,
};

use super::{
    IndexReconciliationRetryDisposition, IndexReconciliationRetryError,
    IndexReconciliationRetryFailure, IndexReconciliationRetryLease,
    PostgresIndexReconciliationRetryStore, PostgresMutationStore,
};

const RECONCILIATION_JOB_REQUEST_CONTRACT: &str = "index_reconciliation_job_v1";
const RECONCILIATION_JOB_CURSOR_CONTRACT: &str = "index_reconciliation_cursor_v1";
const MAX_PAGES_PER_RUN: usize = 1_024;
const MAX_PASSES: u32 = 8;
const MAX_SOURCE_NAME_BYTES: usize = 128;
const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_LEASE_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReconciliationRunRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    worker_id: String,
    page_limit: usize,
    max_pages: usize,
    heartbeat_every_pages: usize,
    pass_count: u32,
    lease_seconds: u64,
}

impl IndexReconciliationRunRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: Uuid,
        schema: SchemaRef,
        worker_id: impl Into<String>,
        page_limit: usize,
        max_pages: usize,
        heartbeat_every_pages: usize,
        pass_count: u32,
        lease_duration: Duration,
    ) -> Result<Self, IndexReconciliationRunError> {
        IndexSourceScanRequest::new(tenant_id, schema.clone(), None, page_limit)
            .map_err(IndexReconciliationRunError::InvalidPageRequest)?;
        if !(1..=MAX_PAGES_PER_RUN).contains(&max_pages) {
            return Err(IndexReconciliationRunError::InvalidMaxPages {
                actual: max_pages,
                max: MAX_PAGES_PER_RUN,
            });
        }
        if heartbeat_every_pages == 0 || heartbeat_every_pages > max_pages {
            return Err(IndexReconciliationRunError::InvalidHeartbeatCadence {
                actual: heartbeat_every_pages,
                max: max_pages,
            });
        }
        if !(1..=MAX_PASSES).contains(&pass_count) {
            return Err(IndexReconciliationRunError::InvalidPassCount {
                actual: pass_count,
                max: MAX_PASSES,
            });
        }
        let worker_id = worker_id.into();
        validate_storage_text(&worker_id, MAX_WORKER_ID_BYTES)
            .map_err(|reason| IndexReconciliationRunError::InvalidWorkerId { reason })?;
        let lease_seconds = validate_lease_duration(lease_duration)?;
        Ok(Self {
            tenant_id,
            schema,
            worker_id,
            page_limit,
            max_pages,
            heartbeat_every_pages,
            pass_count,
            lease_seconds,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn page_limit(&self) -> usize {
        self.page_limit
    }

    pub fn max_pages(&self) -> usize {
        self.max_pages
    }

    pub fn heartbeat_every_pages(&self) -> usize {
        self.heartbeat_every_pages
    }

    pub fn pass_count(&self) -> u32 {
        self.pass_count
    }

    pub fn lease_duration(&self) -> Duration {
        Duration::from_secs(self.lease_seconds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReconciliationRunStatus {
    Busy,
    AlreadyComplete,
    Complete,
    Cancelled,
    Yielded,
    RetryScheduled,
    FailedPermanent,
    FailedExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReconciliationRunOutcome {
    status: IndexReconciliationRunStatus,
    job_id: Option<Uuid>,
    attempt_count: Option<u32>,
    retry_after: Option<Duration>,
    next_attempt: Option<u32>,
    pages_processed: usize,
    passes_completed: u32,
    heartbeat_count: usize,
    mutation_count: usize,
    applied_count: usize,
    duplicate_count: usize,
    stale_count: usize,
}

impl IndexReconciliationRunOutcome {
    pub fn status(&self) -> IndexReconciliationRunStatus {
        self.status
    }

    pub fn job_id(&self) -> Option<Uuid> {
        self.job_id
    }

    pub fn attempt_count(&self) -> Option<u32> {
        self.attempt_count
    }

    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    pub fn next_attempt(&self) -> Option<u32> {
        self.next_attempt
    }

    pub fn pages_processed(&self) -> usize {
        self.pages_processed
    }

    pub fn passes_completed(&self) -> u32 {
        self.passes_completed
    }

    pub fn heartbeat_count(&self) -> usize {
        self.heartbeat_count
    }

    pub fn mutation_count(&self) -> usize {
        self.mutation_count
    }

    pub fn applied_count(&self) -> usize {
        self.applied_count
    }

    pub fn duplicate_count(&self) -> usize {
        self.duplicate_count
    }

    pub fn stale_count(&self) -> usize {
        self.stale_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReconciliationTerminalState {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReconciliationCancelOutcome {
    Requested,
    Cancelled,
    AlreadyTerminal(IndexReconciliationTerminalState),
    NotFound,
}

#[derive(Clone)]
pub struct PostgresIndexReconciliationRunner {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    schema_registry: Arc<SchemaRegistry>,
}

impl PostgresIndexReconciliationRunner {
    pub fn new(
        db: DatabaseConnection,
        sources: SharedIndexSourceRegistry,
        schema_registry: Arc<SchemaRegistry>,
    ) -> Self {
        Self {
            db,
            sources,
            schema_registry,
        }
    }

    pub async fn request_cancel(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> Result<IndexReconciliationCancelOutcome, IndexReconciliationRunError> {
        if tenant_id.is_nil() {
            return Err(IndexReconciliationRunError::NilCancelTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexReconciliationRunError::NilCancelJobId);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = request_cancel_in_transaction(&transaction, tenant_id, job_id).await;
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

    pub async fn run(
        &self,
        request: IndexReconciliationRunRequest,
    ) -> Result<IndexReconciliationRunOutcome, IndexReconciliationRunError> {
        let source_name = self
            .sources
            .source_for_schema(request.schema())
            .ok_or_else(|| {
                IndexReconciliationRunError::UnknownSchemaSource(request.schema.clone())
            })?
            .source_name()
            .to_owned();
        validate_source_name(&source_name)?;

        let acquire_request = ReconciliationAcquireRequest {
            tenant_id: request.tenant_id,
            schema: request.schema.clone(),
            source_name,
            worker_id: request.worker_id.clone(),
            pass_count: request.pass_count,
            lease_seconds: request.lease_seconds,
        };
        let acquired = acquire_reconciliation_job(&self.db, &acquire_request).await?;
        let (lease, mut state) = match acquired {
            ReconciliationAcquireOutcome::Busy => {
                return Ok(empty_outcome(
                    IndexReconciliationRunStatus::Busy,
                    None,
                    None,
                    0,
                ));
            }
            ReconciliationAcquireOutcome::AlreadyComplete {
                job_id,
                completed_passes,
            } => {
                return Ok(empty_outcome(
                    IndexReconciliationRunStatus::AlreadyComplete,
                    Some(job_id),
                    None,
                    completed_passes,
                ));
            }
            ReconciliationAcquireOutcome::Acquired { lease, state } => (lease, state),
        };

        let sink = PostgresMutationStore::new(self.db.clone());
        let mut outcome = IndexReconciliationRunOutcome {
            status: IndexReconciliationRunStatus::Yielded,
            job_id: Some(lease.job_id),
            attempt_count: Some(lease.attempt_count),
            retry_after: None,
            next_attempt: None,
            pages_processed: 0,
            passes_completed: state.completed_passes,
            heartbeat_count: 0,
            mutation_count: 0,
            applied_count: 0,
            duplicate_count: 0,
            stale_count: 0,
        };

        for page_index in 0..request.max_pages {
            if cancel_if_requested(&self.db, &lease).await? {
                outcome.status = IndexReconciliationRunStatus::Cancelled;
                return Ok(outcome);
            }
            if page_index > 0 && page_index % request.heartbeat_every_pages == 0 {
                heartbeat(&self.db, &lease, request.lease_duration()).await?;
                outcome.heartbeat_count += 1;
                if cancel_if_requested(&self.db, &lease).await? {
                    outcome.status = IndexReconciliationRunStatus::Cancelled;
                    return Ok(outcome);
                }
            }

            let scan_request = IndexSourceScanRequest::new(
                request.tenant_id,
                request.schema.clone(),
                state.source_cursor.clone(),
                request.page_limit,
            )
            .map_err(IndexReconciliationRunError::InvalidPageRequest)?;
            let page = match self.sources.scan(scan_request).await {
                Ok(page) => page,
                Err(error) => {
                    let run_error = IndexReconciliationRunError::Source(error);
                    return finish_page_error(&self.db, &lease, outcome, run_error).await;
                }
            };

            let mut event_ids = BTreeSet::new();
            for (position, mutation) in page.mutations().iter().enumerate() {
                let event_id = mutation.event_id();
                if event_id.is_nil() {
                    let run_error = IndexReconciliationRunError::NilEventId { position };
                    return finish_page_error(&self.db, &lease, outcome, run_error).await;
                }
                if !event_ids.insert(event_id) {
                    let run_error =
                        IndexReconciliationRunError::DuplicateEventId { position, event_id };
                    return finish_page_error(&self.db, &lease, outcome, run_error).await;
                }
            }

            let mut page_applied = 0usize;
            let mut page_duplicates = 0usize;
            let mut page_stale = 0usize;
            for (position, mutation) in page.mutations().iter().enumerate() {
                let result = sink
                    .apply_replay_mutation(
                        self.schema_registry.as_ref(),
                        lease.source_name.as_str(),
                        mutation,
                    )
                    .await;
                match result {
                    Ok(IndexReplayMutationOutcome::Applied) => page_applied += 1,
                    Ok(IndexReplayMutationOutcome::Duplicate) => page_duplicates += 1,
                    Ok(IndexReplayMutationOutcome::StaleIgnored) => page_stale += 1,
                    Err(failure) => {
                        let run_error =
                            IndexReconciliationRunError::MutationFailed { position, failure };
                        return finish_page_error(&self.db, &lease, outcome, run_error).await;
                    }
                }
            }

            let page_mutations = page.mutations().len();
            let (_, next_cursor) = page.into_parts();
            state.source_cursor = next_cursor;
            state.pages_processed = checked_add_counter(state.pages_processed, 1)?;
            state.mutation_count =
                checked_add_counter(state.mutation_count, usize_to_u64(page_mutations)?)?;
            state.applied_count =
                checked_add_counter(state.applied_count, usize_to_u64(page_applied)?)?;
            state.duplicate_count =
                checked_add_counter(state.duplicate_count, usize_to_u64(page_duplicates)?)?;
            state.stale_count = checked_add_counter(state.stale_count, usize_to_u64(page_stale)?)?;

            outcome.pages_processed += 1;
            outcome.mutation_count += page_mutations;
            outcome.applied_count += page_applied;
            outcome.duplicate_count += page_duplicates;
            outcome.stale_count += page_stale;

            if state.source_cursor.is_none() {
                state.completed_passes = state
                    .completed_passes
                    .checked_add(1)
                    .ok_or(IndexReconciliationRunError::CounterOverflow)?;
                outcome.passes_completed = state.completed_passes;
            }

            if cancel_if_requested(&self.db, &lease).await? {
                outcome.status = IndexReconciliationRunStatus::Cancelled;
                return Ok(outcome);
            }

            if state.completed_passes == request.pass_count {
                match finish_success(&self.db, &lease, &state).await? {
                    LeaseWriteOutcome::Written => {
                        outcome.status = IndexReconciliationRunStatus::Complete;
                        return Ok(outcome);
                    }
                    LeaseWriteOutcome::Cancelled => {
                        outcome.status = IndexReconciliationRunStatus::Cancelled;
                        return Ok(outcome);
                    }
                }
            }

            match persist_progress(&self.db, &lease, &state).await? {
                LeaseWriteOutcome::Written => {}
                LeaseWriteOutcome::Cancelled => {
                    outcome.status = IndexReconciliationRunStatus::Cancelled;
                    return Ok(outcome);
                }
            }
        }

        match yield_for_resume(&self.db, &lease).await? {
            LeaseWriteOutcome::Written => Ok(outcome),
            LeaseWriteOutcome::Cancelled => {
                outcome.status = IndexReconciliationRunStatus::Cancelled;
                Ok(outcome)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ReconciliationAcquireRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    source_name: String,
    worker_id: String,
    pass_count: u32,
    lease_seconds: u64,
}

#[derive(Debug, Clone)]
struct ReconciliationLease {
    tenant_id: Uuid,
    job_id: Uuid,
    #[allow(dead_code)]
    schema: SchemaRef,
    source_name: String,
    worker_id: String,
    attempt_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationJobRequest {
    contract: String,
    source_name: String,
    pass_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationCursor {
    contract: String,
    completed_passes: u32,
    source_cursor: Option<IndexSourceCursor>,
    pages_processed: u64,
    mutation_count: u64,
    applied_count: u64,
    duplicate_count: u64,
    stale_count: u64,
}

impl ReconciliationCursor {
    fn initial() -> Self {
        Self {
            contract: RECONCILIATION_JOB_CURSOR_CONTRACT.to_owned(),
            completed_passes: 0,
            source_cursor: None,
            pages_processed: 0,
            mutation_count: 0,
            applied_count: 0,
            duplicate_count: 0,
            stale_count: 0,
        }
    }

    fn validate(&self, pass_count: u32) -> Result<(), IndexReconciliationRunError> {
        if self.contract != RECONCILIATION_JOB_CURSOR_CONTRACT {
            return Err(IndexReconciliationRunError::InvalidStoredJob(
                "cursor contract is invalid".to_owned(),
            ));
        }
        if self.completed_passes > pass_count {
            return Err(IndexReconciliationRunError::InvalidStoredJob(
                "completed pass count exceeds the requested pass count".to_owned(),
            ));
        }
        if self.completed_passes == pass_count && self.source_cursor.is_some() {
            return Err(IndexReconciliationRunError::InvalidStoredJob(
                "terminal reconciliation cursor retains source continuation".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
enum ReconciliationAcquireOutcome {
    Acquired {
        lease: ReconciliationLease,
        state: ReconciliationCursor,
    },
    Busy,
    AlreadyComplete {
        job_id: Uuid,
        completed_passes: u32,
    },
}

#[derive(Debug)]
struct StoredReconciliationJob {
    job_id: Uuid,
    state: String,
    request: ReconciliationJobRequest,
    cursor: ReconciliationCursor,
    attempt_count: u32,
    claimable: bool,
    last_error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaseWriteOutcome {
    Written,
    Cancelled,
}

async fn acquire_reconciliation_job(
    db: &DatabaseConnection,
    request: &ReconciliationAcquireRequest,
) -> Result<ReconciliationAcquireOutcome, IndexReconciliationRunError> {
    let transaction = db.begin().await.map_err(storage_error)?;
    let result = acquire_in_transaction(&transaction, request).await;
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

async fn acquire_in_transaction(
    transaction: &DatabaseTransaction,
    request: &ReconciliationAcquireRequest,
) -> Result<ReconciliationAcquireOutcome, IndexReconciliationRunError> {
    let backend = transaction.get_database_backend();
    ensure_supported_backend(backend)?;
    lock_reconciliation_scope(transaction, request, backend).await?;
    verify_schema_registration(transaction, request, backend).await?;

    let rows = transaction
        .query_all(Statement::from_sql_and_values(
            backend,
            select_jobs_sql(backend),
            scope_values(request, backend),
        ))
        .await
        .map_err(storage_error)?;
    let mut claimable = None;
    for row in rows {
        let stored = stored_job(&row, backend)?;
        validate_stored_request(&stored, request)?;
        match stored.state.as_str() {
            "succeeded" => {
                if stored.cursor.completed_passes != request.pass_count
                    || stored.cursor.source_cursor.is_some()
                {
                    return Err(IndexReconciliationRunError::InvalidStoredJob(
                        "succeeded reconciliation job has incomplete cursor state".to_owned(),
                    ));
                }
                return Ok(ReconciliationAcquireOutcome::AlreadyComplete {
                    job_id: stored.job_id,
                    completed_passes: stored.cursor.completed_passes,
                });
            }
            "running" | "pending" if !stored.claimable => {
                return Ok(ReconciliationAcquireOutcome::Busy);
            }
            "running" | "pending" => {
                claimable = Some(stored);
                break;
            }
            "failed" => {
                return Err(IndexReconciliationRunError::DeadLettered {
                    job_id: stored.job_id,
                    attempt_count: stored.attempt_count,
                    error_code: stored.last_error_code,
                });
            }
            state => {
                return Err(IndexReconciliationRunError::InvalidStoredJob(format!(
                    "unexpected active reconciliation state {state}"
                )));
            }
        }
    }

    let (job_id, attempt_count, state) = if let Some(stored) = claimable {
        let attempt_count = stored
            .attempt_count
            .checked_add(1)
            .ok_or(IndexReconciliationRunError::CounterOverflow)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                claim_job_sql(backend),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(stored.job_id, backend),
                    request.worker_id.clone().into(),
                    i64::from(attempt_count).into(),
                    i64::try_from(request.lease_seconds)
                        .map_err(|_| IndexReconciliationRunError::InvalidLeaseDuration)?
                        .into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(IndexReconciliationRunError::LeaseLost {
                job_id: stored.job_id,
                attempt_count,
            });
        }
        (stored.job_id, attempt_count, stored.cursor)
    } else {
        let job_id = Uuid::new_v4();
        let state = ReconciliationCursor::initial();
        let job_request = ReconciliationJobRequest {
            contract: RECONCILIATION_JOB_REQUEST_CONTRACT.to_owned(),
            source_name: request.source_name.clone(),
            pass_count: request.pass_count,
        };
        let request_json = serde_json::to_value(job_request)
            .map_err(|error| IndexReconciliationRunError::Storage(error.to_string()))?;
        let cursor_json = serde_json::to_value(&state)
            .map_err(|error| IndexReconciliationRunError::Storage(error.to_string()))?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                insert_job_sql(backend),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(job_id, backend),
                    request.schema.module.as_str().to_owned().into(),
                    request.schema.entity.as_str().to_owned().into(),
                    i64::from(request.schema.version.get()).into(),
                    SqlValue::Json(Some(Box::new(request_json))),
                    SqlValue::Json(Some(Box::new(cursor_json))),
                    request.worker_id.clone().into(),
                    i64::try_from(request.lease_seconds)
                        .map_err(|_| IndexReconciliationRunError::InvalidLeaseDuration)?
                        .into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        (job_id, 1, state)
    };

    Ok(ReconciliationAcquireOutcome::Acquired {
        lease: ReconciliationLease {
            tenant_id: request.tenant_id,
            job_id,
            schema: request.schema.clone(),
            source_name: request.source_name.clone(),
            worker_id: request.worker_id.clone(),
            attempt_count,
        },
        state,
    })
}

fn validate_stored_request(
    stored: &StoredReconciliationJob,
    request: &ReconciliationAcquireRequest,
) -> Result<(), IndexReconciliationRunError> {
    if stored.request.contract != RECONCILIATION_JOB_REQUEST_CONTRACT
        || stored.request.source_name != request.source_name
        || stored.request.pass_count != request.pass_count
    {
        return Err(IndexReconciliationRunError::InvalidStoredJob(
            "stored reconciliation request does not match the source/pass contract".to_owned(),
        ));
    }
    validate_source_name(&stored.request.source_name)?;
    stored.cursor.validate(stored.request.pass_count)
}

fn stored_job(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<StoredReconciliationJob, IndexReconciliationRunError> {
    let request_json: JsonValue = row.try_get("", "request").map_err(storage_error)?;
    let request: ReconciliationJobRequest = serde_json::from_value(request_json)
        .map_err(|error| IndexReconciliationRunError::InvalidStoredJob(error.to_string()))?;
    let cursor_json: JsonValue = row.try_get("", "cursor").map_err(storage_error)?;
    let cursor: ReconciliationCursor = serde_json::from_value(cursor_json)
        .map_err(|error| IndexReconciliationRunError::InvalidStoredJob(error.to_string()))?;
    let attempt_count: i64 = row
        .try_get("", "attempt_count_value")
        .map_err(storage_error)?;
    let attempt_count = u32::try_from(attempt_count).map_err(|_| {
        IndexReconciliationRunError::InvalidStoredJob(
            "attempt count is outside the u32 range".to_owned(),
        )
    })?;
    let last_error_code: Option<String> =
        row.try_get("", "last_error_code").map_err(storage_error)?;
    if let Some(code) = &last_error_code {
        validate_storage_text(code, MAX_ERROR_CODE_BYTES).map_err(|_| {
            IndexReconciliationRunError::InvalidStoredJob(
                "last_error_code is outside the reconciliation error contract".to_owned(),
            )
        })?;
    }
    Ok(StoredReconciliationJob {
        job_id: stored_uuid(row, "job_id", backend)?,
        state: row.try_get("", "state").map_err(storage_error)?,
        request,
        cursor,
        attempt_count,
        claimable: row.try_get("", "claimable").map_err(storage_error)?,
        last_error_code,
    })
}

async fn lock_reconciliation_scope(
    transaction: &DatabaseTransaction,
    request: &ReconciliationAcquireRequest,
    backend: DbBackend,
) -> Result<(), IndexReconciliationRunError> {
    if backend == DbBackend::Sqlite {
        return Ok(());
    }
    let lock_key = format!(
        "reconcile\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        request.tenant_id,
        request.schema.module.as_str(),
        request.schema.entity.as_str(),
        request.schema.version.get(),
    );
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn verify_schema_registration(
    transaction: &DatabaseTransaction,
    request: &ReconciliationAcquireRequest,
    backend: DbBackend,
) -> Result<(), IndexReconciliationRunError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            select_schema_sql(backend),
            scope_values(request, backend),
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| IndexReconciliationRunError::SchemaNotRegistered(request.schema.clone()))?;
    let status: String = row.try_get("", "status").map_err(storage_error)?;
    if status != "active" {
        return Err(IndexReconciliationRunError::SchemaRetired(
            request.schema.clone(),
        ));
    }
    Ok(())
}

async fn persist_progress(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
    state: &ReconciliationCursor,
) -> Result<LeaseWriteOutcome, IndexReconciliationRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let state_json = serde_json::to_value(state)
        .map_err(|error| IndexReconciliationRunError::Storage(error.to_string()))?;
    let mut values = lease_values(lease, backend);
    values.push(SqlValue::Json(Some(Box::new(state_json))));
    let updated = db
        .execute(Statement::from_sql_and_values(
            backend,
            persist_progress_sql(backend),
            values,
        ))
        .await
        .map_err(storage_error)?;
    lease_write_outcome(db, lease, updated.rows_affected()).await
}

async fn heartbeat(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
    duration: Duration,
) -> Result<(), IndexReconciliationRunError> {
    let seconds = validate_lease_duration(duration)?;
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let mut values = lease_values(lease, backend);
    values.push(
        i64::try_from(seconds)
            .map_err(|_| IndexReconciliationRunError::InvalidLeaseDuration)?
            .into(),
    );
    let updated = db
        .execute(Statement::from_sql_and_values(
            backend,
            heartbeat_sql(backend),
            values,
        ))
        .await
        .map_err(storage_error)?;
    if updated.rows_affected() != 1 {
        return Err(IndexReconciliationRunError::LeaseLost {
            job_id: lease.job_id,
            attempt_count: lease.attempt_count,
        });
    }
    Ok(())
}

async fn finish_success(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
    state: &ReconciliationCursor,
) -> Result<LeaseWriteOutcome, IndexReconciliationRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let state_json = serde_json::to_value(state)
        .map_err(|error| IndexReconciliationRunError::Storage(error.to_string()))?;
    let mut values = lease_values(lease, backend);
    values.push(SqlValue::Json(Some(Box::new(state_json))));
    let updated = db
        .execute(Statement::from_sql_and_values(
            backend,
            finish_success_sql(backend),
            values,
        ))
        .await
        .map_err(storage_error)?;
    lease_write_outcome(db, lease, updated.rows_affected()).await
}

async fn finish_page_error(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
    mut outcome: IndexReconciliationRunOutcome,
    error: IndexReconciliationRunError,
) -> Result<IndexReconciliationRunOutcome, IndexReconciliationRunError> {
    let failure = retry_failure(&error)?;
    let retry_lease = IndexReconciliationRetryLease::new(
        lease.tenant_id,
        lease.job_id,
        lease.worker_id.clone(),
        lease.attempt_count,
    )
    .map_err(IndexReconciliationRunError::RetryTransition)?;
    let retry_store = PostgresIndexReconciliationRetryStore::new(db.clone());
    match retry_store.record_failure(&retry_lease, &failure).await {
        Ok(IndexReconciliationRetryDisposition::RetryScheduled {
            retry_after,
            next_attempt,
        }) => {
            outcome.status = IndexReconciliationRunStatus::RetryScheduled;
            outcome.retry_after = Some(retry_after);
            outcome.next_attempt = Some(next_attempt);
            Ok(outcome)
        }
        Ok(IndexReconciliationRetryDisposition::TerminalPermanent { .. }) => {
            outcome.status = IndexReconciliationRunStatus::FailedPermanent;
            Ok(outcome)
        }
        Ok(IndexReconciliationRetryDisposition::TerminalExhausted { .. }) => {
            outcome.status = IndexReconciliationRunStatus::FailedExhausted;
            Ok(outcome)
        }
        Err(IndexReconciliationRetryError::LeaseLost) => {
            if cancel_if_requested(db, lease).await? {
                outcome.status = IndexReconciliationRunStatus::Cancelled;
                return Ok(outcome);
            }
            Err(IndexReconciliationRunError::LeaseLost {
                job_id: lease.job_id,
                attempt_count: lease.attempt_count,
            })
        }
        Err(error) => Err(IndexReconciliationRunError::RetryTransition(error)),
    }
}

async fn yield_for_resume(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
) -> Result<LeaseWriteOutcome, IndexReconciliationRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let updated = db
        .execute(Statement::from_sql_and_values(
            backend,
            yield_job_sql(backend),
            lease_values(lease, backend),
        ))
        .await
        .map_err(storage_error)?;
    lease_write_outcome(db, lease, updated.rows_affected()).await
}

async fn request_cancel_in_transaction(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<IndexReconciliationCancelOutcome, IndexReconciliationRunError> {
    let backend = transaction.get_database_backend();
    ensure_supported_backend(backend)?;
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            select_cancel_job_sql(backend),
            vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(IndexReconciliationCancelOutcome::NotFound);
    };
    let state: String = row.try_get("", "state").map_err(storage_error)?;
    match state.as_str() {
        "pending" => {
            let updated = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    cancel_pending_job_sql(backend),
                    vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
                ))
                .await
                .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(IndexReconciliationRunError::CancellationRace);
            }
            Ok(IndexReconciliationCancelOutcome::Cancelled)
        }
        "running" => {
            let updated = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    request_running_cancel_sql(backend),
                    vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
                ))
                .await
                .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(IndexReconciliationRunError::CancellationRace);
            }
            Ok(IndexReconciliationCancelOutcome::Requested)
        }
        "succeeded" => Ok(IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Succeeded,
        )),
        "failed" => Ok(IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Failed,
        )),
        "cancelled" => Ok(IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Cancelled,
        )),
        other => Err(IndexReconciliationRunError::InvalidStoredJobState(
            other.to_owned(),
        )),
    }
}

async fn cancel_if_requested(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
) -> Result<bool, IndexReconciliationRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let updated = db
        .execute(Statement::from_sql_and_values(
            backend,
            cancel_active_job_sql(backend),
            lease_values(lease, backend),
        ))
        .await
        .map_err(storage_error)?;
    Ok(updated.rows_affected() == 1)
}

async fn lease_write_outcome(
    db: &DatabaseConnection,
    lease: &ReconciliationLease,
    rows_affected: u64,
) -> Result<LeaseWriteOutcome, IndexReconciliationRunError> {
    if rows_affected == 1 {
        return Ok(LeaseWriteOutcome::Written);
    }
    if cancel_if_requested(db, lease).await? {
        return Ok(LeaseWriteOutcome::Cancelled);
    }
    Err(IndexReconciliationRunError::LeaseLost {
        job_id: lease.job_id,
        attempt_count: lease.attempt_count,
    })
}

fn retry_failure(
    error: &IndexReconciliationRunError,
) -> Result<IndexReconciliationRetryFailure, IndexReconciliationRunError> {
    let failure = match error {
        IndexReconciliationRunError::Source(IndexSourceError::SourceFailure {
            failure, ..
        }) => match failure.kind() {
            IndexSourceFailureKind::Retryable => {
                IndexReconciliationRetryFailure::retryable(failure.code())
            }
            IndexSourceFailureKind::Permanent => {
                IndexReconciliationRetryFailure::permanent(failure.code())
            }
        },
        IndexReconciliationRunError::MutationFailed { failure, .. } => match failure.kind() {
            IndexReplayFailureKind::Retryable => {
                IndexReconciliationRetryFailure::retryable(failure.code())
            }
            IndexReplayFailureKind::Permanent => {
                IndexReconciliationRetryFailure::permanent(failure.code())
            }
        },
        IndexReconciliationRunError::Source(_) => {
            IndexReconciliationRetryFailure::permanent("source_contract_invalid")
        }
        _ => IndexReconciliationRetryFailure::permanent("reconciliation_contract_invalid"),
    };
    failure.map_err(IndexReconciliationRunError::RetryTransition)
}

fn empty_outcome(
    status: IndexReconciliationRunStatus,
    job_id: Option<Uuid>,
    attempt_count: Option<u32>,
    passes_completed: u32,
) -> IndexReconciliationRunOutcome {
    IndexReconciliationRunOutcome {
        status,
        job_id,
        attempt_count,
        retry_after: None,
        next_attempt: None,
        pages_processed: 0,
        passes_completed,
        heartbeat_count: 0,
        mutation_count: 0,
        applied_count: 0,
        duplicate_count: 0,
        stale_count: 0,
    }
}

fn validate_source_name(source_name: &str) -> Result<(), IndexReconciliationRunError> {
    if source_name.is_empty()
        || source_name.len() > MAX_SOURCE_NAME_BYTES
        || !source_name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(IndexReconciliationRunError::InvalidSourceName);
    }
    Ok(())
}

fn validate_storage_text(value: &str, max_bytes: usize) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.trim() != value {
        return Err("must not contain leading or trailing whitespace");
    }
    if value.len() > max_bytes {
        return Err("exceeds the storage limit");
    }
    if value.chars().any(char::is_control) {
        return Err("must not contain control characters");
    }
    Ok(())
}

fn validate_lease_duration(lease_duration: Duration) -> Result<u64, IndexReconciliationRunError> {
    if lease_duration.subsec_nanos() != 0 {
        return Err(IndexReconciliationRunError::InvalidLeaseDuration);
    }
    let seconds = lease_duration.as_secs();
    if seconds == 0 || seconds > MAX_LEASE_SECONDS {
        return Err(IndexReconciliationRunError::InvalidLeaseDuration);
    }
    Ok(seconds)
}

fn checked_add_counter(current: u64, increment: u64) -> Result<u64, IndexReconciliationRunError> {
    current
        .checked_add(increment)
        .ok_or(IndexReconciliationRunError::CounterOverflow)
}

fn usize_to_u64(value: usize) -> Result<u64, IndexReconciliationRunError> {
    u64::try_from(value).map_err(|_| IndexReconciliationRunError::CounterOverflow)
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReconciliationRunError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(IndexReconciliationRunError::Storage(format!(
            "Index reconciliation does not support {backend:?}"
        ))),
    }
}

fn storage_error(error: impl std::fmt::Display) -> IndexReconciliationRunError {
    IndexReconciliationRunError::Storage(error.to_string())
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

fn stored_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, IndexReconciliationRunError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn scope_values(request: &ReconciliationAcquireRequest, backend: DbBackend) -> Vec<SqlValue> {
    vec![
        uuid_value(request.tenant_id, backend),
        request.schema.module.as_str().to_owned().into(),
        request.schema.entity.as_str().to_owned().into(),
        i64::from(request.schema.version.get()).into(),
    ]
}

fn lease_values(lease: &ReconciliationLease, backend: DbBackend) -> Vec<SqlValue> {
    vec![
        uuid_value(lease.tenant_id, backend),
        uuid_value(lease.job_id, backend),
        lease.worker_id.clone().into(),
        i64::from(lease.attempt_count).into(),
    ]
}

fn select_schema_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT status FROM index_schemas WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 LIMIT 1"
    )
}

fn select_jobs_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let (attempt_count, claimable) = match backend {
        DbBackend::Postgres => (
            "CAST(attempt_count AS BIGINT)",
            "((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))",
        ),
        DbBackend::Sqlite => (
            "CAST(attempt_count AS INTEGER)",
            "CASE WHEN (state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP) THEN TRUE ELSE FALSE END",
        ),
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT job_id, state, request, cursor, last_error_code, {attempt_count} AS attempt_count_value, {claimable} AS claimable FROM index_jobs WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 AND kind = 'reconcile' AND scope_kind = 'schema' AND state IN ('pending', 'running', 'succeeded', 'failed') ORDER BY CASE state WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1 WHEN 'pending' THEN 2 ELSE 3 END, created_at DESC"
    )
}

fn insert_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 9);
    format!(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, request, cursor, attempt_count, available_at, lease_owner, lease_expires_at, heartbeat_at) VALUES ({prefix}1, {prefix}2, 'reconcile', 'running', 'schema', {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, 1, CURRENT_TIMESTAMP, {prefix}8, {lease_expires}, CURRENT_TIMESTAMP)"
    )
}

fn claim_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET state = 'running', lease_owner = {prefix}3, attempt_count = {prefix}4, lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, completed_at = NULL, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn heartbeat_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 5);
    format!(
        "UPDATE index_jobs SET lease_expires_at = {lease_expires}, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn persist_progress_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET cursor = {prefix}5, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn finish_success_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'succeeded', cursor = {prefix}5, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn yield_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'pending', available_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn select_cancel_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = match backend {
        DbBackend::Postgres => " FOR UPDATE",
        DbBackend::Sqlite => "",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT state FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' LIMIT 1{lock}"
    )
}

fn cancel_pending_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'cancelled', cancel_requested = TRUE, completed_at = CURRENT_TIMESTAMP, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'pending'"
    )
}

fn request_running_cancel_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET cancel_requested = TRUE, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running'"
    )
}

fn cancel_active_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'cancelled', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = TRUE"
    )
}

fn lease_expires_expression(backend: DbBackend, seconds_parameter: usize) -> String {
    let prefix = placeholder_prefix(backend);
    match backend {
        DbBackend::Postgres => {
            format!("CURRENT_TIMESTAMP + ({prefix}{seconds_parameter} * INTERVAL '1 second')")
        }
        DbBackend::Sqlite => {
            format!("datetime(CURRENT_TIMESTAMP, '+' || {prefix}{seconds_parameter} || ' seconds')")
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

#[derive(Debug, Error)]
pub enum IndexReconciliationRunError {
    #[error("Index reconciliation page request is invalid")]
    InvalidPageRequest(#[source] IndexSourceError),
    #[error("Index reconciliation max pages is invalid: actual={actual}, max={max}")]
    InvalidMaxPages { actual: usize, max: usize },
    #[error("Index reconciliation heartbeat cadence is invalid: actual={actual}, max={max}")]
    InvalidHeartbeatCadence { actual: usize, max: usize },
    #[error("Index reconciliation pass count is invalid: actual={actual}, max={max}")]
    InvalidPassCount { actual: u32, max: u32 },
    #[error("Index reconciliation worker id is invalid: {reason}")]
    InvalidWorkerId { reason: &'static str },
    #[error(
        "Index reconciliation lease duration must be a whole number of seconds between 1 and 86400"
    )]
    InvalidLeaseDuration,
    #[error("Index reconciliation source name is invalid")]
    InvalidSourceName,
    #[error("No Index source owns reconciliation schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("Index reconciliation schema is not persisted for this tenant: {0}")]
    SchemaNotRegistered(SchemaRef),
    #[error("Index reconciliation schema is retired: {0}")]
    SchemaRetired(SchemaRef),
    #[error(
        "Index reconciliation scope is blocked by failed job {job_id} after attempt {attempt_count}"
    )]
    DeadLettered {
        job_id: Uuid,
        attempt_count: u32,
        error_code: Option<String>,
    },
    #[error("stored Index reconciliation job is invalid: {0}")]
    InvalidStoredJob(String),
    #[error("stored Index reconciliation job has unsupported state {0}")]
    InvalidStoredJobState(String),
    #[error("Index reconciliation cancellation tenant id must not be nil")]
    NilCancelTenantId,
    #[error("Index reconciliation cancellation job id must not be nil")]
    NilCancelJobId,
    #[error("Index reconciliation cancellation lost a concurrent state transition")]
    CancellationRace,
    #[error("Index reconciliation source failed")]
    Source(#[source] IndexSourceError),
    #[error("Index reconciliation mutation at position {position} has a nil event id")]
    NilEventId { position: usize },
    #[error("Index reconciliation mutation at position {position} duplicates event id {event_id}")]
    DuplicateEventId { position: usize, event_id: Uuid },
    #[error("Index reconciliation mutation at position {position} failed")]
    MutationFailed {
        position: usize,
        #[source]
        failure: IndexReplayFailure,
    },
    #[error("Index reconciliation retry transition failed")]
    RetryTransition(#[source] IndexReconciliationRetryError),
    #[error("Index reconciliation durable counter overflowed")]
    CounterOverflow,
    #[error("Index reconciliation job {job_id} lost attempt {attempt_count} ownership")]
    LeaseLost { job_id: Uuid, attempt_count: u32 },
    #[error("Index reconciliation storage operation failed: {0}")]
    Storage(String),
}
