use std::{future::Future, sync::Arc, time::Duration};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tokio::time::{Instant, sleep_until};
use uuid::Uuid;

use crate::{
    IndexReplayError, IndexReplayFailureKind, IndexReplayPageRequest, IndexReplayPageStatus,
    IndexReplayWorker, IndexSourceError, IndexSourceFailureKind, LocaleKey, SchemaRef,
    SchemaRegistry, SharedIndexSourceRegistry,
};

use super::{
    IndexReplayJobAcquireOutcome, IndexReplayJobError, IndexReplayJobLease,
    IndexReplayJobLeaseRequest, PostgresIndexReplayCheckpointStore, PostgresIndexReplayJobStore,
    PostgresMutationStore,
};

const MAX_PAGES_PER_RUN: usize = 1_024;
const MIN_REPLAY_RUN_LEASE_DURATION: Duration = Duration::from_secs(60);
const PAGE_LEASE_HEARTBEAT_DIVISOR: u32 = 3;
const REPLAY_PAGE_FAILURE_CODE: &str = "index.replay_page_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayRunRequest {
    page_request: IndexReplayPageRequest,
    worker_id: String,
    lease_duration: Duration,
    max_pages: usize,
    heartbeat_every_pages: usize,
}

impl IndexReplayRunRequest {
    pub fn new(
        tenant_id: Uuid,
        schema: SchemaRef,
        worker_id: impl Into<String>,
        page_limit: usize,
        max_pages: usize,
        heartbeat_every_pages: usize,
        lease_duration: Duration,
    ) -> Result<Self, IndexReplayRunError> {
        Self::new_scoped(
            tenant_id,
            schema,
            None,
            worker_id,
            page_limit,
            max_pages,
            heartbeat_every_pages,
            lease_duration,
        )
    }

    pub fn for_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: LocaleKey,
        worker_id: impl Into<String>,
        page_limit: usize,
        max_pages: usize,
        heartbeat_every_pages: usize,
        lease_duration: Duration,
    ) -> Result<Self, IndexReplayRunError> {
        Self::new_scoped(
            tenant_id,
            schema,
            Some(locale),
            worker_id,
            page_limit,
            max_pages,
            heartbeat_every_pages,
            lease_duration,
        )
    }

    fn new_scoped(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: Option<LocaleKey>,
        worker_id: impl Into<String>,
        page_limit: usize,
        max_pages: usize,
        heartbeat_every_pages: usize,
        lease_duration: Duration,
    ) -> Result<Self, IndexReplayRunError> {
        if !(1..=MAX_PAGES_PER_RUN).contains(&max_pages) {
            return Err(IndexReplayRunError::InvalidMaxPages {
                actual: max_pages,
                max: MAX_PAGES_PER_RUN,
            });
        }
        if heartbeat_every_pages == 0 || heartbeat_every_pages > max_pages {
            return Err(IndexReplayRunError::InvalidHeartbeatCadence {
                actual: heartbeat_every_pages,
                max: max_pages,
            });
        }
        if lease_duration < MIN_REPLAY_RUN_LEASE_DURATION {
            return Err(IndexReplayRunError::LeaseDurationTooShort {
                actual: lease_duration,
                minimum: MIN_REPLAY_RUN_LEASE_DURATION,
            });
        }
        let page_request = match locale {
            Some(locale) => {
                IndexReplayPageRequest::for_locale(tenant_id, schema, locale, page_limit)
            }
            None => IndexReplayPageRequest::new(tenant_id, schema, page_limit),
        }
        .map_err(IndexReplayRunError::InvalidPageRequest)?;
        Ok(Self {
            page_request,
            worker_id: worker_id.into(),
            lease_duration,
            max_pages,
            heartbeat_every_pages,
        })
    }

    pub fn page_request(&self) -> &IndexReplayPageRequest {
        &self.page_request
    }

    pub fn locale(&self) -> Option<&LocaleKey> {
        self.page_request.locale()
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn lease_duration(&self) -> Duration {
        self.lease_duration
    }

    pub fn max_pages(&self) -> usize {
        self.max_pages
    }

    pub fn heartbeat_every_pages(&self) -> usize {
        self.heartbeat_every_pages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayRunStatus {
    Busy,
    AlreadyComplete,
    Complete,
    Cancelled,
    Yielded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexReplayRunOutcome {
    status: IndexReplayRunStatus,
    job_id: Option<Uuid>,
    attempt_count: Option<u32>,
    pages_processed: usize,
    heartbeat_count: usize,
    mutation_count: usize,
    applied_count: usize,
    duplicate_count: usize,
    stale_count: usize,
}

impl IndexReplayRunOutcome {
    pub fn status(&self) -> IndexReplayRunStatus {
        self.status
    }

    pub fn job_id(&self) -> Option<Uuid> {
        self.job_id
    }

    pub fn attempt_count(&self) -> Option<u32> {
        self.attempt_count
    }

    pub fn pages_processed(&self) -> usize {
        self.pages_processed
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
pub enum IndexReplayTerminalState {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexReplayCancelOutcome {
    Requested,
    Cancelled,
    AlreadyTerminal(IndexReplayTerminalState),
    NotFound,
}

#[derive(Clone)]
pub struct PostgresIndexReplayRunner {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    schema_registry: Arc<SchemaRegistry>,
}

impl PostgresIndexReplayRunner {
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
    ) -> Result<IndexReplayCancelOutcome, IndexReplayRunError> {
        if tenant_id.is_nil() {
            return Err(IndexReplayRunError::NilCancelTenantId);
        }
        if job_id.is_nil() {
            return Err(IndexReplayRunError::NilCancelJobId);
        }
        let transaction = self.db.begin().await.map_err(job_storage_error)?;
        let result = request_cancel_in_transaction(&transaction, tenant_id, job_id).await;
        match result {
            Ok(outcome) => {
                transaction.commit().await.map_err(job_storage_error)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction.rollback().await.map_err(job_storage_error)?;
                Err(error)
            }
        }
    }

    pub async fn run(
        &self,
        request: IndexReplayRunRequest,
    ) -> Result<IndexReplayRunOutcome, IndexReplayRunError> {
        let source_name = self
            .sources
            .source_for_schema(request.page_request().schema())
            .ok_or_else(|| {
                IndexReplayRunError::UnknownSchemaSource(request.page_request().schema().clone())
            })?
            .source_name()
            .to_owned();
        let lease_request = lease_request_for_run(&request, source_name)?;
        let job_store = PostgresIndexReplayJobStore::new(self.db.clone());
        let lease = match job_store.acquire(&lease_request).await? {
            IndexReplayJobAcquireOutcome::Busy => {
                return Ok(empty_outcome(IndexReplayRunStatus::Busy, None, None));
            }
            IndexReplayJobAcquireOutcome::AlreadyComplete { job_id } => {
                return Ok(empty_outcome(
                    IndexReplayRunStatus::AlreadyComplete,
                    Some(job_id),
                    None,
                ));
            }
            IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
        };

        let checkpoint_store =
            PostgresIndexReplayCheckpointStore::new(self.db.clone(), lease.clone());
        let worker = IndexReplayWorker::new(
            self.sources.clone(),
            self.schema_registry.clone(),
            PostgresMutationStore::new(self.db.clone()),
            checkpoint_store,
        );

        let mut aggregate = IndexReplayRunOutcome {
            status: IndexReplayRunStatus::Yielded,
            job_id: Some(lease.job_id()),
            attempt_count: Some(lease.attempt_count()),
            pages_processed: 0,
            heartbeat_count: 0,
            mutation_count: 0,
            applied_count: 0,
            duplicate_count: 0,
            stale_count: 0,
        };

        for page_index in 0..request.max_pages() {
            if cancel_if_requested(&self.db, &lease).await? {
                aggregate.status = IndexReplayRunStatus::Cancelled;
                return Ok(aggregate);
            }

            if page_index > 0 && page_index % request.heartbeat_every_pages() == 0 {
                heartbeat(&job_store, &lease, request.lease_duration()).await?;
                aggregate.heartbeat_count += 1;
                if cancel_if_requested(&self.db, &lease).await? {
                    aggregate.status = IndexReplayRunStatus::Cancelled;
                    return Ok(aggregate);
                }
            }

            let (page_result, in_page_heartbeat_count) = await_page_with_lease_heartbeats(
                &job_store,
                &lease,
                request.lease_duration(),
                worker.run_next_page(request.page_request().clone()),
            )
            .await?;
            aggregate.heartbeat_count += in_page_heartbeat_count;
            let page = match page_result {
                Ok(page) => page,
                Err(error) if replay_error_is_lease_lost(&error) => {
                    return Err(lease_lost(&lease));
                }
                Err(error) => {
                    if cancel_if_requested(&self.db, &lease).await? {
                        aggregate.status = IndexReplayRunStatus::Cancelled;
                        return Ok(aggregate);
                    }
                    let details = replay_failure_details(&error);
                    match finish_failure(&self.db, &lease, details).await? {
                        TerminalWriteOutcome::Written => {
                            return Err(IndexReplayRunError::PageFailed {
                                job_id: lease.job_id(),
                                error: Box::new(error),
                            });
                        }
                        TerminalWriteOutcome::Cancelled => {
                            aggregate.status = IndexReplayRunStatus::Cancelled;
                            return Ok(aggregate);
                        }
                    }
                }
            };

            if page.status() != IndexReplayPageStatus::AlreadyComplete {
                aggregate.pages_processed += 1;
            }
            aggregate.mutation_count += page.mutation_count();
            aggregate.applied_count += page.applied_count();
            aggregate.duplicate_count += page.duplicate_count();
            aggregate.stale_count += page.stale_count();

            if cancel_if_requested(&self.db, &lease).await? {
                aggregate.status = IndexReplayRunStatus::Cancelled;
                return Ok(aggregate);
            }

            if matches!(
                page.status(),
                IndexReplayPageStatus::Complete | IndexReplayPageStatus::AlreadyComplete
            ) {
                match finish_success(&self.db, &lease).await? {
                    TerminalWriteOutcome::Written => {
                        aggregate.status = IndexReplayRunStatus::Complete;
                        return Ok(aggregate);
                    }
                    TerminalWriteOutcome::Cancelled => {
                        aggregate.status = IndexReplayRunStatus::Cancelled;
                        return Ok(aggregate);
                    }
                }
            }
        }

        match yield_for_resume(&self.db, &lease).await? {
            TerminalWriteOutcome::Written => Ok(aggregate),
            TerminalWriteOutcome::Cancelled => {
                aggregate.status = IndexReplayRunStatus::Cancelled;
                Ok(aggregate)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalWriteOutcome {
    Written,
    Cancelled,
}

fn lease_request_for_run(
    request: &IndexReplayRunRequest,
    source_name: String,
) -> Result<IndexReplayJobLeaseRequest, IndexReplayRunError> {
    match request.locale() {
        Some(locale) => Ok(IndexReplayJobLeaseRequest::for_locale(
            request.page_request().tenant_id(),
            request.page_request().schema().clone(),
            locale.clone(),
            source_name,
            request.worker_id().to_owned(),
            request.lease_duration(),
        )?),
        None => Ok(IndexReplayJobLeaseRequest::new(
            request.page_request().tenant_id(),
            request.page_request().schema().clone(),
            source_name,
            request.worker_id().to_owned(),
            request.lease_duration(),
        )?),
    }
}

fn empty_outcome(
    status: IndexReplayRunStatus,
    job_id: Option<Uuid>,
    attempt_count: Option<u32>,
) -> IndexReplayRunOutcome {
    IndexReplayRunOutcome {
        status,
        job_id,
        attempt_count,
        pages_processed: 0,
        heartbeat_count: 0,
        mutation_count: 0,
        applied_count: 0,
        duplicate_count: 0,
        stale_count: 0,
    }
}

fn page_lease_heartbeat_interval(lease_duration: Duration) -> Duration {
    lease_duration / PAGE_LEASE_HEARTBEAT_DIVISOR
}

async fn await_page_with_lease_heartbeats<T, F>(
    job_store: &PostgresIndexReplayJobStore,
    lease: &IndexReplayJobLease,
    lease_duration: Duration,
    page_future: F,
) -> Result<(Result<T, IndexReplayError>, usize), IndexReplayRunError>
where
    F: Future<Output = Result<T, IndexReplayError>>,
{
    let heartbeat_interval = page_lease_heartbeat_interval(lease_duration);
    debug_assert!(!heartbeat_interval.is_zero());
    tokio::pin!(page_future);
    let mut heartbeat_count = 0usize;
    let mut next_heartbeat = Instant::now() + heartbeat_interval;

    loop {
        tokio::select! {
            page_result = &mut page_future => return Ok((page_result, heartbeat_count)),
            _ = sleep_until(next_heartbeat) => {
                let heartbeat_future = heartbeat(job_store, lease, lease_duration);
                tokio::pin!(heartbeat_future);
                tokio::select! {
                    page_result = &mut page_future => return Ok((page_result, heartbeat_count)),
                    heartbeat_result = &mut heartbeat_future => {
                        heartbeat_result?;
                        heartbeat_count += 1;
                        next_heartbeat = Instant::now() + heartbeat_interval;
                    }
                }
            }
        }
    }
}

async fn heartbeat(
    job_store: &PostgresIndexReplayJobStore,
    lease: &IndexReplayJobLease,
    lease_duration: Duration,
) -> Result<(), IndexReplayRunError> {
    match job_store.heartbeat(lease, lease_duration).await {
        Ok(()) => Ok(()),
        Err(IndexReplayJobError::LeaseLost) => Err(lease_lost(lease)),
        Err(error) => Err(IndexReplayRunError::Job(error)),
    }
}

async fn request_cancel_in_transaction(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<IndexReplayCancelOutcome, IndexReplayRunError> {
    let backend = transaction.get_database_backend();
    ensure_supported_backend(backend)?;
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            select_cancel_job_sql(backend),
            vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
        ))
        .await
        .map_err(job_storage_error)?;
    let Some(row) = row else {
        return Ok(IndexReplayCancelOutcome::NotFound);
    };
    let state: String = row.try_get("", "state").map_err(job_storage_error)?;
    match state.as_str() {
        "pending" => {
            let updated = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    cancel_pending_job_sql(backend),
                    vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
                ))
                .await
                .map_err(job_storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(IndexReplayRunError::CancellationRace);
            }
            Ok(IndexReplayCancelOutcome::Cancelled)
        }
        "running" => {
            let updated = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    request_running_cancel_sql(backend),
                    vec![uuid_value(tenant_id, backend), uuid_value(job_id, backend)],
                ))
                .await
                .map_err(job_storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(IndexReplayRunError::CancellationRace);
            }
            Ok(IndexReplayCancelOutcome::Requested)
        }
        "succeeded" => Ok(IndexReplayCancelOutcome::AlreadyTerminal(
            IndexReplayTerminalState::Succeeded,
        )),
        "failed" => Ok(IndexReplayCancelOutcome::AlreadyTerminal(
            IndexReplayTerminalState::Failed,
        )),
        "cancelled" => Ok(IndexReplayCancelOutcome::AlreadyTerminal(
            IndexReplayTerminalState::Cancelled,
        )),
        other => Err(IndexReplayRunError::InvalidStoredJobState(other.to_owned())),
    }
}

async fn cancel_if_requested(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
) -> Result<bool, IndexReplayRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            backend,
            cancel_active_job_sql(backend),
            lease_values(lease, backend),
        ))
        .await
        .map_err(job_storage_error)?;
    Ok(updated.rows_affected() == 1)
}

async fn finish_success(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
) -> Result<TerminalWriteOutcome, IndexReplayRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let mut values = lease_values(lease, backend);
    values.push(lease.source_name().to_owned().into());
    values.push(lease.schema().module.as_str().to_owned().into());
    values.push(lease.schema().entity.as_str().to_owned().into());
    values.push(i64::from(lease.schema().version.get()).into());
    let locale_value = lease.locale().map(|locale| locale.as_str().to_owned()).unwrap_or_default();
    values.push(locale_value.into());
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            backend,
            finish_success_sql(backend),
            values,
        ))
        .await
        .map_err(job_storage_error)?;
    terminal_write_outcome(db, lease, updated.rows_affected()).await
}

async fn finish_failure(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
    details: JsonValue,
) -> Result<TerminalWriteOutcome, IndexReplayRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let mut values = lease_values(lease, backend);
    values.push(REPLAY_PAGE_FAILURE_CODE.to_owned().into());
    values.push(SqlValue::Json(Some(Box::new(details))));
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            backend,
            finish_failure_sql(backend),
            values,
        ))
        .await
        .map_err(job_storage_error)?;
    terminal_write_outcome(db, lease, updated.rows_affected()).await
}

async fn yield_for_resume(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
) -> Result<TerminalWriteOutcome, IndexReplayRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            backend,
            yield_job_sql(backend),
            lease_values(lease, backend),
        ))
        .await
        .map_err(job_storage_error)?;
    terminal_write_outcome(db, lease, updated.rows_affected()).await
}

async fn terminal_write_outcome(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
    rows_affected: u64,
) -> Result<TerminalWriteOutcome, IndexReplayRunError> {
    if rows_affected == 1 {
        return Ok(TerminalWriteOutcome::Written);
    }
    if cancel_if_requested(db, lease).await? {
        return Ok(TerminalWriteOutcome::Cancelled);
    }
    Err(lease_lost(lease))
}

fn replay_error_is_lease_lost(error: &IndexReplayError) -> bool {
    match error {
        IndexReplayError::CheckpointReadFailed(failure)
        | IndexReplayError::CheckpointCommitFailed(failure) => {
            failure.code() == "checkpoint_lease_lost"
        }
        _ => false,
    }
}

fn replay_failure_details(error: &IndexReplayError) -> JsonValue {
    let (code, retryable) = match error {
        IndexReplayError::SourceContract(IndexSourceError::SourceFailure { failure, .. }) => (
            failure.code(),
            failure.kind() == IndexSourceFailureKind::Retryable,
        ),
        IndexReplayError::MutationFailed { failure, .. }
        | IndexReplayError::CheckpointReadFailed(failure)
        | IndexReplayError::CheckpointCommitFailed(failure) => (
            failure.code(),
            failure.kind() == IndexReplayFailureKind::Retryable,
        ),
        IndexReplayError::SourceContract(_) => ("source_contract_invalid", false),
        _ => ("replay_contract_invalid", false),
    };
    json!({
        "contract": "index_replay_run_failure_v1",
        "dependency_code": code,
        "retryable": retryable,
    })
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReplayRunError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(IndexReplayRunError::Job(IndexReplayJobError::Storage(
            format!("Index replay runner does not support {backend:?}"),
        ))),
}
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

fn lease_values(lease: &IndexReplayJobLease, backend: DbBackend) -> Vec<SqlValue> {
    vec![
        uuid_value(lease.tenant_id(), backend),
        uuid_value(lease.job_id(), backend),
        lease.worker_id().to_owned().into(),
        i64::from(lease.attempt_count()).into(),
    ]
}

fn select_cancel_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = match backend {
        DbBackend::Postgres => " FOR UPDATE",
        DbBackend::Sqlite => "",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT state FROM index_jobs WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' LIMIT 1{lock}"
    )
}

fn cancel_pending_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'cancelled', cancel_requested = TRUE, completed_at = CURRENT_TIMESTAMP, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'pending'"
    )
}

fn request_running_cancel_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET cancel_requested = TRUE, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running'"
    )
}

fn cancel_active_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'cancelled', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = TRUE"
    )
}

fn finish_success_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let complete_cursor = match backend {
        DbBackend::Postgres => "checkpoint.cursor = 'null'::jsonb",
        DbBackend::Sqlite => "json_type(checkpoint.cursor) = 'null'",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "UPDATE index_jobs SET state = 'succeeded', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = NULL, last_error_details = NULL WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE AND EXISTS (SELECT 1 FROM index_checkpoints AS checkpoint WHERE checkpoint.tenant_id = {prefix}1 AND checkpoint.checkpoint_kind = 'rebuild' AND checkpoint.source_name = {prefix}5 AND checkpoint.module_name = {prefix}6 AND checkpoint.entity_name = {prefix}7 AND checkpoint.schema_version = {prefix}8 AND checkpoint.locale_key = {prefix}9 AND checkpoint.partition_key = '' AND {complete_cursor})"
    )
}

fn finish_failure_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'failed', lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, last_error_code = {prefix}5, last_error_details = {prefix}6 WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn yield_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'pending', available_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP AND cancel_requested = FALSE"
    )
}

fn job_storage_error(error: impl std::fmt::Display) -> IndexReplayRunError {
    IndexReplayRunError::Job(IndexReplayJobError::Storage(error.to_string()))
}

fn lease_lost(lease: &IndexReplayJobLease) -> IndexReplayRunError {
    IndexReplayRunError::LeaseLost {
        job_id: lease.job_id(),
        attempt_count: lease.attempt_count(),
    }
}

#[derive(Debug, Error)]
pub enum IndexReplayRunError {
    #[error("Index replay run page request is invalid")]
    InvalidPageRequest(#[source] IndexReplayError),
    #[error("Index replay run max pages is invalid: actual={actual}, max={max}")]
    InvalidMaxPages { actual: usize, max: usize },
    #[error("Index replay heartbeat cadence is invalid: actual={actual}, max={max}")]
    InvalidHeartbeatCadence { actual: usize, max: usize },
    #[error(
        "Index replay lease duration is too short for the page heartbeat policy: actual={actual:?}, minimum={minimum:?}"
    )]
    LeaseDurationTooShort { actual: Duration, minimum: Duration },
    #[error("Index replay cancellation tenant id must not be nil")]
    NilCancelTenantId,
    #[error("Index replay cancellation job id must not be nil")]
    NilCancelJobId,
    #[error("No Index replay source owns schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("stored Index replay job has unsupported state {0}")]
    InvalidStoredJobState(String),
    #[error("Index replay cancellation lost a concurrent state transition")]
    CancellationRace,
    #[error(transparent)]
    Job(#[from] IndexReplayJobError),
    #[error("Index replay job {job_id} lost attempt {attempt_count} ownership")]
    LeaseLost { job_id: Uuid, attempt_count: u32 },
    #[error("Index replay job {job_id} failed while processing a page")]
    PageFailed {
        job_id: Uuid,
        #[source]
        error: Box<IndexReplayError>,
    },
}

#[cfg(test)]
mod locale_scope_tests {
    use super::*;

    fn product_schema() -> SchemaRef {
        SchemaRef {
            module: crate::ModuleName::new("rustok-product").unwrap(),
            entity: crate::EntityName::new("product").unwrap(),
            version: crate::SchemaVersion::new(4),
        }
    }

    #[test]
    fn locale_run_request_keeps_page_job_and_terminal_checkpoint_scope_identical() {
        let request = IndexReplayRunRequest::for_locale(
            Uuid::new_v4(),
            product_schema(),
            LocaleKey::new("EN-us").unwrap(),
            "locale-runner-test",
            100,
            8,
            1,
            Duration::from_secs(60),
        )
        .unwrap();

        assert_eq!(request.locale().map(LocaleKey::as_str), Some("en-US"));
        let lease_request =
            lease_request_for_run(&request, "product-postgres-primary".to_owned()).unwrap();
        assert_eq!(lease_request.locale().map(LocaleKey::as_str), Some("en-US"));

        let postgres = finish_success_sql(DbBackend::Postgres);
        assert!(postgres.contains("checkpoint.locale_key = $9"));
        assert!(postgres.contains("checkpoint.partition_key = ''"));
        let sqlite = finish_success_sql(DbBackend::Sqlite);
        assert!(sqlite.contains("checkpoint.locale_key = ?9"));
        assert!(sqlite.contains("checkpoint.partition_key = ''"));
    }

    #[test]
    fn schema_run_request_preserves_empty_locale_identity() {
        let request = IndexReplayRunRequest::new(
            Uuid::new_v4(),
            product_schema(),
            "schema-runner-test",
            100,
            8,
            1,
            Duration::from_secs(60),
        )
        .unwrap();
        assert!(request.locale().is_none());
        let lease_request =
            lease_request_for_run(&request, "product-postgres-primary".to_owned()).unwrap();
        assert!(lease_request.locale().is_none());
    }

    #[test]
    fn page_lease_policy_requires_two_dependency_windows_and_heartbeats_at_one_third() {
        let too_short = IndexReplayRunRequest::new(
            Uuid::new_v4(),
            product_schema(),
            "short-lease-test",
            100,
            8,
            1,
            Duration::from_secs(59),
        )
        .expect_err("lease shorter than the canonical page reserve must fail closed");
        assert!(matches!(
            too_short,
            IndexReplayRunError::LeaseDurationTooShort { .. }
        ));

        let minimum = IndexReplayRunRequest::new(
            Uuid::new_v4(),
            product_schema(),
            "minimum-lease-test",
            100,
            8,
            1,
            Duration::from_secs(60),
        )
        .expect("canonical 60 second replay lease should remain valid");
        assert_eq!(minimum.lease_duration(), Duration::from_secs(60));
        assert_eq!(
            page_lease_heartbeat_interval(minimum.lease_duration()),
            Duration::from_secs(20)
        );
    }
}
