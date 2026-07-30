use std::{sync::Arc, time::Duration};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};
use serde_json::{json, Value as JsonValue};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    IndexReplayError, IndexReplayFailureKind, IndexReplayPageRequest, IndexReplayPageStatus,
    IndexReplayWorker, IndexSourceError, IndexSourceFailureKind, SchemaRef, SchemaRegistry,
    SharedIndexSourceRegistry,
};

use super::{
    IndexReplayJobAcquireOutcome, IndexReplayJobError, IndexReplayJobLease,
    IndexReplayJobLeaseRequest, PostgresIndexReplayCheckpointStore, PostgresIndexReplayJobStore,
    PostgresMutationStore,
};

const MAX_PAGES_PER_RUN: usize = 1_024;
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
        let page_request = IndexReplayPageRequest::new(tenant_id, schema, page_limit)
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

    pub async fn run(
        &self,
        request: IndexReplayRunRequest,
    ) -> Result<IndexReplayRunOutcome, IndexReplayRunError> {
        let source_name = self
            .sources
            .source_for_schema(request.page_request().schema())
            .ok_or_else(|| {
                IndexReplayRunError::UnknownSchemaSource(
                    request.page_request().schema().clone(),
                )
            })?
            .source_name()
            .to_owned();
        let lease_request = IndexReplayJobLeaseRequest::new(
            request.page_request().tenant_id(),
            request.page_request().schema().clone(),
            source_name,
            request.worker_id().to_owned(),
            request.lease_duration(),
        )?;
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
            if page_index > 0 && page_index % request.heartbeat_every_pages() == 0 {
                heartbeat(&job_store, &lease, request.lease_duration()).await?;
                aggregate.heartbeat_count += 1;
            }

            let page = match worker.run_next_page(request.page_request().clone()).await {
                Ok(page) => page,
                Err(error) if replay_error_is_lease_lost(&error) => {
                    return Err(lease_lost(&lease));
                }
                Err(error) => {
                    let details = replay_failure_details(&error);
                    match job_store
                        .fail(&lease, REPLAY_PAGE_FAILURE_CODE, details)
                        .await
                    {
                        Ok(()) => {
                            return Err(IndexReplayRunError::PageFailed {
                                job_id: lease.job_id(),
                                error: Box::new(error),
                            });
                        }
                        Err(IndexReplayJobError::LeaseLost) => {
                            return Err(lease_lost(&lease));
                        }
                        Err(job_error) => return Err(IndexReplayRunError::Job(job_error)),
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

            if matches!(
                page.status(),
                IndexReplayPageStatus::Complete | IndexReplayPageStatus::AlreadyComplete
            ) {
                match job_store.succeed(&lease).await {
                    Ok(()) => {
                        aggregate.status = IndexReplayRunStatus::Complete;
                        return Ok(aggregate);
                    }
                    Err(IndexReplayJobError::LeaseLost) => return Err(lease_lost(&lease)),
                    Err(error) => return Err(IndexReplayRunError::Job(error)),
                }
            }
        }

        yield_for_resume(&self.db, &lease).await?;
        Ok(aggregate)
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

async fn yield_for_resume(
    db: &DatabaseConnection,
    lease: &IndexReplayJobLease,
) -> Result<(), IndexReplayRunError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let updated = db
        .execute(Statement::from_sql_and_values(
            backend,
            yield_job_sql(backend),
            vec![
                uuid_value(lease.tenant_id(), backend),
                uuid_value(lease.job_id(), backend),
                lease.worker_id().to_owned().into(),
                i64::from(lease.attempt_count()).into(),
            ],
        ))
        .await
        .map_err(|error| IndexReplayRunError::Job(IndexReplayJobError::Storage(error.to_string())))?;
    if updated.rows_affected() != 1 {
        return Err(lease_lost(lease));
    }
    Ok(())
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

fn yield_job_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_jobs SET state = 'pending', available_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = 'rebuild' AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 AND lease_expires_at > CURRENT_TIMESTAMP"
    )
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
    #[error("No Index replay source owns schema {0}")]
    UnknownSchemaSource(SchemaRef),
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
