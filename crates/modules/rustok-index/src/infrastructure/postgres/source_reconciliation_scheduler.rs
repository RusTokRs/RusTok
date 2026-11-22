use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_runtime::{
    HostRuntimeContext, ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome,
    ModuleWorkRegistration, ModuleWorkRegistrations, ModuleWorkScheduler, ModuleWorkSource,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    EntityName, ModuleName, SchemaRef, SchemaRegistry, SchemaVersion, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry,
};

use super::source_job_discovery::{
    DiscoveredJobVersion, MAX_DISCARDED_CANDIDATES_PER_POLL, defer_due_job, discovery_scope,
};
use super::{
    IndexReconciliationBudget, IndexReconciliationRunError, IndexReconciliationRunRequest,
    IndexReconciliationRunStatus, PostgresIndexReconciliationRunner,
};

pub const INDEX_RECONCILIATION_WORKER: &str = "index_reconciliation";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexReconciliationSchedulerCompositionError {
    #[error("Index reconciliation scheduler is already registered")]
    AlreadyRegistered,
    #[error("Index reconciliation scheduler requires the shared schema registry")]
    MissingSchemaRegistry,
}

#[derive(Clone)]
struct IndexReconciliationSchedulerRegistrationMarker;

pub fn register_postgres_index_reconciliation_work(
    extensions: &mut ModuleRuntimeExtensions,
) -> Result<bool, IndexReconciliationSchedulerCompositionError> {
    if extensions.contains::<IndexReconciliationSchedulerRegistrationMarker>() {
        return Err(IndexReconciliationSchedulerCompositionError::AlreadyRegistered);
    }
    if !extensions.contains::<SharedIndexSourceRegistry>() {
        return Ok(false);
    }
    if !extensions.contains::<SharedIndexSchemaRegistry>() {
        return Err(IndexReconciliationSchedulerCompositionError::MissingSchemaRegistry);
    }
    extensions
        .get_or_insert_with::<ModuleWorkRegistrations, _>(Default::default)
        .register(Arc::new(IndexReconciliationWorkRegistration));
    extensions.insert(IndexReconciliationSchedulerRegistrationMarker);
    Ok(true)
}

const RECONCILIATION_JOB_REQUEST_CONTRACT: &str = "index_reconciliation_job_v1";
const RECONCILIATION_WORK_ITEM_CONTRACT: &str = "index_reconciliation_scheduler_item_v1";
const DISCOVERY_FAILED_CODE: &str = "index.reconciliation_scheduler.discovery_failed";
const INVALID_STORED_JOB_CODE: &str = "index.reconciliation_scheduler.invalid_stored_job";
const INVALID_WORK_ITEM_CODE: &str = "index.reconciliation_scheduler.invalid_work_item";
const RUN_FAILED_CODE: &str = "index.reconciliation_scheduler.run_failed";
const DEFAULT_PAGE_LIMIT: usize = 100;
const DEFAULT_MAX_PAGES: usize = 8;
const DEFAULT_HEARTBEAT_EVERY_PAGES: usize = 1;
const DEFAULT_LEASE_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexReconciliationSchedulerPolicy {
    page_limit: usize,
    max_pages: usize,
    heartbeat_every_pages: usize,
    lease_seconds: u64,
}

impl IndexReconciliationSchedulerPolicy {
    pub fn new(
        page_limit: usize,
        max_pages: usize,
        heartbeat_every_pages: usize,
        lease_duration: Duration,
    ) -> Result<Self, IndexReconciliationRunError> {
        let schema = SchemaRef {
            // INVARIANT: static scheduler module name is valid
            module: ModuleName::new("index-scheduler-policy").expect(
                "static scheduler module identifier must be valid: invariant validated at startup",
            ),
            // INVARIANT: static scheduler entity name is valid
            entity: EntityName::new("item").expect(
                "static scheduler entity identifier must be valid: invariant validated at startup",
            ),
            version: SchemaVersion::INITIAL,
        };
        IndexReconciliationRunRequest::new(
            Uuid::from_u128(1),
            schema,
            INDEX_RECONCILIATION_WORKER,
            IndexReconciliationBudget::new(
                page_limit,
                max_pages,
                heartbeat_every_pages,
                1,
                lease_duration,
            ),
        )?;
        Ok(Self {
            page_limit,
            max_pages,
            heartbeat_every_pages,
            lease_seconds: lease_duration.as_secs(),
        })
    }

    pub fn page_limit(self) -> usize {
        self.page_limit
    }

    pub fn max_pages(self) -> usize {
        self.max_pages
    }

    pub fn heartbeat_every_pages(self) -> usize {
        self.heartbeat_every_pages
    }

    pub fn lease_duration(self) -> Duration {
        Duration::from_secs(self.lease_seconds)
    }
}

impl Default for IndexReconciliationSchedulerPolicy {
    fn default() -> Self {
        Self::new(
            DEFAULT_PAGE_LIMIT,
            DEFAULT_MAX_PAGES,
            DEFAULT_HEARTBEAT_EVERY_PAGES,
            Duration::from_secs(DEFAULT_LEASE_SECONDS),
        )
        // INVARIANT: default reconciliation scheduler policy must be valid
        .expect("default reconciliation scheduler policy must be valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationWorkPayload {
    contract: String,
    module_name: String,
    entity_name: String,
    schema_version: u32,
    pass_count: u32,
    lease_generation: i64,
    retry_epoch: i64,
}

#[derive(Debug, Clone)]
struct DueReconciliationWork {
    tenant_id: Uuid,
    job_id: Uuid,
    schema: SchemaRef,
    pass_count: u32,
    lease_generation: i64,
    retry_epoch: i64,
}

#[derive(Clone)]
pub struct PostgresIndexReconciliationWorkAdapter {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    schemas: Arc<SchemaRegistry>,
    runner: PostgresIndexReconciliationRunner,
    policy: IndexReconciliationSchedulerPolicy,
}

impl PostgresIndexReconciliationWorkAdapter {
    pub fn new(
        db: DatabaseConnection,
        sources: SharedIndexSourceRegistry,
        schemas: Arc<SchemaRegistry>,
        policy: IndexReconciliationSchedulerPolicy,
    ) -> Self {
        let runner =
            PostgresIndexReconciliationRunner::new(db.clone(), sources.clone(), schemas.clone());
        Self {
            db,
            sources,
            schemas,
            runner,
            policy,
        }
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    fn decode_item(
        item: &ModuleWorkItem,
    ) -> Result<(SchemaRef, ReconciliationWorkPayload, Uuid), ModuleWorkError> {
        if item.worker_slug != INDEX_RECONCILIATION_WORKER
            || item.id.is_nil()
            || item.tenant_id.is_nil()
        {
            return Err(invalid_work_item());
        }
        let invocation_id = Uuid::parse_str(&item.lease_token).map_err(|_| invalid_work_item())?;
        if invocation_id.is_nil() {
            return Err(invalid_work_item());
        }
        let payload: ReconciliationWorkPayload =
            serde_json::from_value(item.payload.clone()).map_err(|_| invalid_work_item())?;
        if payload.contract != RECONCILIATION_WORK_ITEM_CONTRACT
            || payload.schema_version == 0
            || payload.pass_count == 0
            || payload.lease_generation < 0
            || payload.retry_epoch < 0
        {
            return Err(invalid_work_item());
        }
        let schema = SchemaRef {
            module: ModuleName::new(&payload.module_name).map_err(|_| invalid_work_item())?,
            entity: EntityName::new(&payload.entity_name).map_err(|_| invalid_work_item())?,
            version: SchemaVersion::new(payload.schema_version),
        };
        Ok((schema, payload, invocation_id))
    }

    fn work_item(work: DueReconciliationWork) -> Result<ModuleWorkItem, ModuleWorkError> {
        let payload = serde_json::to_value(ReconciliationWorkPayload {
            contract: RECONCILIATION_WORK_ITEM_CONTRACT.to_owned(),
            module_name: work.schema.module.as_str().to_owned(),
            entity_name: work.schema.entity.as_str().to_owned(),
            schema_version: work.schema.version.get(),
            pass_count: work.pass_count,
            lease_generation: work.lease_generation,
            retry_epoch: work.retry_epoch,
        })
        .map_err(|_| invalid_stored_job())?;
        Ok(ModuleWorkItem {
            id: work.job_id,
            tenant_id: work.tenant_id,
            worker_slug: INDEX_RECONCILIATION_WORKER.to_owned(),
            lease_token: Uuid::new_v4().to_string(),
            payload,
        })
    }
}

pub(crate) struct IndexReconciliationWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for IndexReconciliationWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let Some(sources) = host.shared_get::<SharedIndexSourceRegistry>() else {
            return Ok(());
        };
        let schemas = match host.shared_get::<SharedIndexSchemaRegistry>() {
            Some(schemas) => schemas,
            None => {
                return Err(ModuleWorkError::Handler(
                    "index.reconciliation_scheduler.missing_schema_registry".to_owned(),
                ));
            }
        };
        PostgresIndexReconciliationWorkAdapter::new(
            host.db_clone(),
            sources,
            schemas.shared(),
            IndexReconciliationSchedulerPolicy::default(),
        )
        .register_with(scheduler)
        .await
    }
}

#[async_trait]
impl ModuleWorkSource for PostgresIndexReconciliationWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != INDEX_RECONCILIATION_WORKER {
            return Ok(None);
        }
        let Some(work) =
            discover_due_reconciliation(&self.db, &self.sources, &self.schemas).await?
        else {
            return Ok(None);
        };
        Self::work_item(work).map(Some)
    }

    async fn complete(
        &self,
        item: &ModuleWorkItem,
        outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        use super::source_reconciliation_runner::{ReconciliationCursor, ReconciliationJobRequest};
        let (schema, payload, _) = Self::decode_item(item)?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let source = self
            .sources
            .source_for_schema(&schema)
            .ok_or_else(unverified)?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                "SELECT state, request, cursor, CAST(attempt_count AS BIGINT) AS lease_generation,
                    CAST(retry_epoch AS BIGINT) AS retry_epoch FROM index_jobs
             WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND scope_kind = 'schema'
                AND module_name = $3 AND entity_name = $4 AND schema_version = $5",
                vec![
                    uuid_value(item.tenant_id, backend),
                    uuid_value(item.id, backend),
                    payload.module_name.into(),
                    payload.entity_name.into(),
                    i64::from(payload.schema_version).into(),
                ],
            ))
            .await
            .map_err(|_| unverified())?
            .ok_or_else(unverified)?;
        let state: String = row.try_get("", "state").map_err(|_| unverified())?;
        let request: JsonValue = row.try_get("", "request").map_err(|_| unverified())?;
        let request: ReconciliationJobRequest =
            serde_json::from_value(request).map_err(|_| unverified())?;
        if request.contract != RECONCILIATION_JOB_REQUEST_CONTRACT
            || request.source_name != source.source_name()
            || request.pass_count != payload.pass_count
        {
            return Err(unverified());
        }
        let discovered = DiscoveredJobVersion {
            generation: payload.lease_generation,
            retry_epoch: payload.retry_epoch,
        };
        let persisted = DiscoveredJobVersion {
            generation: stored_counter(&row, "lease_generation")?,
            retry_epoch: stored_counter(&row, "retry_epoch")?,
        };
        if !discovered.verifies_outcome(persisted, &state, outcome) {
            return Err(unverified());
        }
        if state == "succeeded" {
            let cursor: JsonValue = row.try_get("", "cursor").map_err(|_| unverified())?;
            let cursor: ReconciliationCursor =
                serde_json::from_value(cursor).map_err(|_| unverified())?;
            cursor
                .validate(request.pass_count)
                .map_err(|_| unverified())?;
            if cursor.completed_passes != request.pass_count || cursor.source_cursor.is_some() {
                return Err(unverified());
            }
        }
        // The runner owns the durable write. Completion only verifies the original row.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for PostgresIndexReconciliationWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        INDEX_RECONCILIATION_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let (schema, payload, invocation_id) = Self::decode_item(&item)?;
        let request = IndexReconciliationRunRequest::new(
            item.tenant_id,
            schema,
            format!("index-reconciliation-{}", invocation_id.simple()),
            IndexReconciliationBudget::new(
                self.policy.page_limit(),
                self.policy.max_pages(),
                self.policy.heartbeat_every_pages(),
                payload.pass_count,
                self.policy.lease_duration(),
            ),
        )
        .and_then(|request| {
            request.for_discovered_job(
                item.id,
                DiscoveredJobVersion {
                    generation: payload.lease_generation,
                    retry_epoch: payload.retry_epoch,
                },
            )
        });
        let run = match request {
            Ok(request) => self
                .runner
                .run(request)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let outcome = match run {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(job_id = %item.id, tenant_id = %item.tenant_id, %error, "Index reconciliation dispatch failed");
                defer_due_job(
                    &self.db,
                    item.tenant_id,
                    item.id,
                    "reconcile",
                    payload.lease_generation,
                    payload.retry_epoch,
                    RUN_FAILED_CODE,
                )
                .await
                .map_err(|_| ModuleWorkError::Handler(RUN_FAILED_CODE.to_owned()))?;
                return Err(ModuleWorkError::Handler(RUN_FAILED_CODE.to_owned()));
            }
        };
        Ok(match outcome.status() {
            IndexReconciliationRunStatus::Cancelled => ModuleWorkOutcome::Cancelled,
            IndexReconciliationRunStatus::Busy => ModuleWorkOutcome::Held {
                message: "index.reconciliation_busy".into(),
            },
            IndexReconciliationRunStatus::AlreadyComplete
            | IndexReconciliationRunStatus::Complete => ModuleWorkOutcome::Completed,
            IndexReconciliationRunStatus::Yielded
            | IndexReconciliationRunStatus::RetryScheduled => ModuleWorkOutcome::Retryable {
                message: "index.reconciliation_pending".into(),
            },
            IndexReconciliationRunStatus::FailedPermanent
            | IndexReconciliationRunStatus::FailedExhausted => ModuleWorkOutcome::Rejected {
                message: "index.reconciliation_dead_lettered".into(),
            },
        })
    }
}

async fn discover_due_reconciliation(
    db: &DatabaseConnection,
    sources: &SharedIndexSourceRegistry,
    schemas: &SchemaRegistry,
) -> Result<Option<DueReconciliationWork>, ModuleWorkError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let (scope, values) = discovery_scope(backend, schemas, sources);
    for _ in 0..MAX_DISCARDED_CANDIDATES_PER_POLL {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                due_reconciliation_sql(&scope),
                values.clone(),
            ))
            .await
            .map_err(|_| ModuleWorkError::Source(DISCOVERY_FAILED_CODE.to_owned()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        match decode_due_work(&row, backend, sources) {
            Ok(work) => return Ok(Some(work)),
            Err(error) => {
                defer_due_job(
                    db,
                    stored_uuid(&row, "tenant_id", backend)?,
                    stored_uuid(&row, "job_id", backend)?,
                    "reconcile",
                    stored_counter(&row, "lease_generation")?,
                    stored_counter(&row, "retry_epoch")?,
                    INVALID_STORED_JOB_CODE,
                )
                .await
                .map_err(|_| ModuleWorkError::Source(DISCOVERY_FAILED_CODE.to_owned()))?;
                tracing::warn!(%error, "Deferred malformed Index reconciliation job");
            }
        }
    }
    Ok(None)
}

fn decode_due_work(
    row: &QueryResult,
    backend: DbBackend,
    sources: &SharedIndexSourceRegistry,
) -> Result<DueReconciliationWork, ModuleWorkError> {
    let tenant_id = stored_uuid(row, "tenant_id", backend)?;
    let job_id = stored_uuid(row, "job_id", backend)?;
    if tenant_id.is_nil() || job_id.is_nil() {
        return Err(invalid_stored_job());
    }
    let module_name: String = row
        .try_get("", "module_name")
        .map_err(|_| invalid_stored_job())?;
    let entity_name: String = row
        .try_get("", "entity_name")
        .map_err(|_| invalid_stored_job())?;
    let schema_version: i32 = row
        .try_get("", "schema_version")
        .map_err(|_| invalid_stored_job())?;
    let schema_version = u32::try_from(schema_version).map_err(|_| invalid_stored_job())?;
    if schema_version == 0 {
        return Err(invalid_stored_job());
    }
    let schema = SchemaRef {
        module: ModuleName::new(module_name).map_err(|_| invalid_stored_job())?,
        entity: EntityName::new(entity_name).map_err(|_| invalid_stored_job())?,
        version: SchemaVersion::new(schema_version),
    };
    let request_json: JsonValue = row
        .try_get("", "request")
        .map_err(|_| invalid_stored_job())?;
    let request: super::source_reconciliation_runner::ReconciliationJobRequest =
        serde_json::from_value(request_json).map_err(|_| invalid_stored_job())?;
    if request.contract != RECONCILIATION_JOB_REQUEST_CONTRACT || request.pass_count == 0 {
        return Err(invalid_stored_job());
    }
    let Some(source) = sources.source_for_schema(&schema) else {
        return Err(invalid_stored_job());
    };
    if source.source_name() != request.source_name {
        return Err(invalid_stored_job());
    }
    IndexReconciliationRunRequest::new(
        tenant_id,
        schema.clone(),
        INDEX_RECONCILIATION_WORKER,
        IndexReconciliationBudget::new(1, 1, 1, request.pass_count, Duration::from_secs(1)),
    )
    .map_err(|_| invalid_stored_job())?;
    Ok(DueReconciliationWork {
        tenant_id,
        job_id,
        schema,
        pass_count: request.pass_count,
        lease_generation: stored_counter(row, "lease_generation")?,
        retry_epoch: stored_counter(row, "retry_epoch")?,
    })
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), ModuleWorkError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(ModuleWorkError::Source(DISCOVERY_FAILED_CODE.to_owned())),
    }
}

fn uuid_value(id: Uuid, backend: DbBackend) -> sea_orm::Value {
    super::source_replay_job::uuid_value(id, backend)
}

fn stored_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ModuleWorkError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(|_| invalid_stored_job()),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(|_| invalid_stored_job())?;
            Uuid::parse_str(&value).map_err(|_| invalid_stored_job())
        }
        _ => Err(ModuleWorkError::Source(DISCOVERY_FAILED_CODE.to_owned())),
    }
}

fn stored_counter(row: &QueryResult, column: &str) -> Result<i64, ModuleWorkError> {
    row.try_get("", column).map_err(|_| invalid_stored_job())
}

fn due_reconciliation_sql(scope: &str) -> String {
    format!("WITH ranked AS (
        SELECT tenant_id, job_id, state, module_name, entity_name, schema_version, scope_kind, request,
            available_at, lease_expires_at, created_at, attempt_count, retry_epoch,
            ROW_NUMBER() OVER (PARTITION BY tenant_id, module_name, entity_name, schema_version
                ORDER BY CASE state WHEN 'succeeded' THEN 0 WHEN 'running' THEN 1
                WHEN 'pending' THEN 2 ELSE 3 END, created_at DESC, job_id DESC) AS scope_rank
        FROM index_jobs WHERE kind = 'reconcile' AND scope_kind = 'schema'
            AND state IN ('pending', 'running', 'succeeded', 'failed')
    ) SELECT tenant_id, job_id, module_name, entity_name, schema_version, request,
          CAST(attempt_count AS BIGINT) AS lease_generation, CAST(retry_epoch AS BIGINT) AS retry_epoch
      FROM ranked AS j WHERE scope_rank = 1 AND {scope} AND available_at <= CURRENT_TIMESTAMP
          AND (state = 'pending' OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP))
      ORDER BY available_at, created_at, tenant_id, job_id LIMIT 1")
}

fn unverified() -> ModuleWorkError {
    ModuleWorkError::Source("index.reconciliation_scheduler.result_unverified".to_owned())
}

fn invalid_stored_job() -> ModuleWorkError {
    ModuleWorkError::Source(INVALID_STORED_JOB_CODE.to_owned())
}

fn invalid_work_item() -> ModuleWorkError {
    ModuleWorkError::Handler(INVALID_WORK_ITEM_CODE.to_owned())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_core::ModuleRuntimeExtensions;
    use rustok_runtime::{
        HostRuntimeContext, ModuleWorkRegistration, ModuleWorkRegistrations, ModuleWorkScheduler,
    };
    use sea_orm::Database;

    use super::{
        IndexReconciliationSchedulerPolicy, IndexReconciliationWorkRegistration,
        ReconciliationWorkPayload, due_reconciliation_sql,
        register_postgres_index_reconciliation_work,
    };

    #[test]
    fn default_policy_is_bounded_and_validated() {
        let policy = IndexReconciliationSchedulerPolicy::default();
        assert_eq!(policy.page_limit(), 100);
        assert_eq!(policy.max_pages(), 8);
        assert_eq!(policy.heartbeat_every_pages(), 1);
        assert_eq!(policy.lease_duration(), Duration::from_secs(300));
        assert!(IndexReconciliationSchedulerPolicy::new(1, 0, 1, Duration::from_secs(1)).is_err());
    }

    #[test]
    fn discovery_sql_ranks_scope_authority_and_is_read_only() {
        {
            let sql = due_reconciliation_sql("TRUE");
            for marker in [
                "ROW_NUMBER() OVER",
                "PARTITION BY tenant_id, module_name, entity_name, schema_version",
                "WHEN 'succeeded' THEN 0",
                "WHEN 'running' THEN 1",
                "WHEN 'pending' THEN 2",
                "scope_rank = 1",
                "available_at <= CURRENT_TIMESTAMP",
                "state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP",
                "LIMIT 1",
            ] {
                assert!(sql.contains(marker), "missing {marker}");
            }
            for forbidden in ["INSERT ", "UPDATE ", "DELETE "] {
                assert!(!sql.contains(forbidden));
            }
        }
    }

    #[test]
    fn work_payload_rejects_unknown_fields() {
        let value = serde_json::json!({
            "contract": "index_reconciliation_scheduler_item_v1",
            "module_name": "demo",
            "entity_name": "item",
            "schema_version": 1,
            "pass_count": 1,
            "lease_generation": 0,
            "retry_epoch": 0,
            "unexpected": true
        });
        assert!(serde_json::from_value::<ReconciliationWorkPayload>(value).is_err());
    }

    #[test]
    fn absent_sources_publish_no_module_work_registration() {
        let mut extensions = ModuleRuntimeExtensions::default();
        assert!(!register_postgres_index_reconciliation_work(&mut extensions).unwrap());
        assert!(!extensions.contains::<ModuleWorkRegistrations>());
    }

    #[tokio::test]
    async fn missing_source_registry_registers_no_worker() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        let host = HostRuntimeContext::new(db);
        let scheduler = ModuleWorkScheduler::new();
        IndexReconciliationWorkRegistration
            .register(&host, &scheduler)
            .await
            .expect("missing optional sources must not fail");
        assert_eq!(scheduler.run_once().await.expect("scheduler runs"), 0);
    }
}
