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

use super::{
    IndexReconciliationRunError, IndexReconciliationRunRequest, IndexReconciliationRunStatus,
    PostgresIndexReconciliationRunner,
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
            module: ModuleName::new("index-scheduler-policy")
                .expect("static scheduler module identifier must be valid"),
            entity: EntityName::new("item")
                .expect("static scheduler entity identifier must be valid"),
            version: SchemaVersion::INITIAL,
        };
        IndexReconciliationRunRequest::new(
            Uuid::from_u128(1),
            schema,
            INDEX_RECONCILIATION_WORKER,
            page_limit,
            max_pages,
            heartbeat_every_pages,
            1,
            lease_duration,
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
        .expect("default reconciliation scheduler policy must be valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredReconciliationJobRequest {
    contract: String,
    source_name: String,
    pass_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationWorkPayload {
    contract: String,
    module_name: String,
    entity_name: String,
    schema_version: u32,
    pass_count: u32,
}

#[derive(Debug, Clone)]
struct DueReconciliationWork {
    tenant_id: Uuid,
    job_id: Uuid,
    schema: SchemaRef,
    pass_count: u32,
}

#[derive(Clone)]
pub struct PostgresIndexReconciliationWorkAdapter {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
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
        let runner = PostgresIndexReconciliationRunner::new(db.clone(), sources.clone(), schemas);
        Self {
            db,
            sources,
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

    fn decode_item(item: &ModuleWorkItem) -> Result<(SchemaRef, u32, Uuid), ModuleWorkError> {
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
        {
            return Err(invalid_work_item());
        }
        let schema = SchemaRef {
            module: ModuleName::new(payload.module_name).map_err(|_| invalid_work_item())?,
            entity: EntityName::new(payload.entity_name).map_err(|_| invalid_work_item())?,
            version: SchemaVersion::new(payload.schema_version),
        };
        Ok((schema, payload.pass_count, invocation_id))
    }

    fn work_item(work: DueReconciliationWork) -> Result<ModuleWorkItem, ModuleWorkError> {
        let payload = serde_json::to_value(ReconciliationWorkPayload {
            contract: RECONCILIATION_WORK_ITEM_CONTRACT.to_owned(),
            module_name: work.schema.module.as_str().to_owned(),
            entity_name: work.schema.entity.as_str().to_owned(),
            schema_version: work.schema.version.get(),
            pass_count: work.pass_count,
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
        let schemas = host
            .shared_get::<SharedIndexSchemaRegistry>()
            .ok_or_else(|| {
                ModuleWorkError::Handler(
                    "index.reconciliation_scheduler.missing_schema_registry".to_owned(),
                )
            })?;
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
        let Some(work) = discover_due_reconciliation(&self.db, &self.sources).await? else {
            return Ok(None);
        };
        Self::work_item(work).map(Some)
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The canonical reconciliation runner owns every durable transition.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for PostgresIndexReconciliationWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        INDEX_RECONCILIATION_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let (schema, pass_count, invocation_id) = Self::decode_item(&item)?;
        let request = IndexReconciliationRunRequest::new(
            item.tenant_id,
            schema,
            format!("index-reconciliation-{}", invocation_id.simple()),
            self.policy.page_limit(),
            self.policy.max_pages(),
            self.policy.heartbeat_every_pages(),
            pass_count,
            self.policy.lease_duration(),
        )
        .map_err(|_| ModuleWorkError::Handler(RUN_FAILED_CODE.to_owned()))?;
        let outcome = self
            .runner
            .run(request)
            .await
            .map_err(|_| ModuleWorkError::Handler(RUN_FAILED_CODE.to_owned()))?;
        Ok(match outcome.status() {
            IndexReconciliationRunStatus::Cancelled => ModuleWorkOutcome::Cancelled,
            IndexReconciliationRunStatus::Busy
            | IndexReconciliationRunStatus::AlreadyComplete
            | IndexReconciliationRunStatus::Complete
            | IndexReconciliationRunStatus::Yielded
            | IndexReconciliationRunStatus::RetryScheduled
            | IndexReconciliationRunStatus::FailedPermanent
            | IndexReconciliationRunStatus::FailedExhausted => ModuleWorkOutcome::Completed,
        })
    }
}

async fn discover_due_reconciliation(
    db: &DatabaseConnection,
    sources: &SharedIndexSourceRegistry,
) -> Result<Option<DueReconciliationWork>, ModuleWorkError> {
    let backend = db.get_database_backend();
    ensure_supported_backend(backend)?;
    let row = db
        .query_one_raw(Statement::from_string(
            backend,
            due_reconciliation_sql(backend),
        ))
        .await
        .map_err(|_| ModuleWorkError::Source(DISCOVERY_FAILED_CODE.to_owned()))?;
    let Some(row) = row else {
        return Ok(None);
    };
    decode_due_work(&row, backend, sources).map(Some)
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
    let schema_version: i64 = row
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
    let request: StoredReconciliationJobRequest =
        serde_json::from_value(request_json).map_err(|_| invalid_stored_job())?;
    if request.contract != RECONCILIATION_JOB_REQUEST_CONTRACT || request.pass_count == 0 {
        return Err(invalid_stored_job());
    }
    let source = sources
        .source_for_schema(&schema)
        .ok_or_else(invalid_stored_job)?;
    if source.source_name() != request.source_name {
        return Err(invalid_stored_job());
    }
    IndexReconciliationRunRequest::new(
        tenant_id,
        schema.clone(),
        INDEX_RECONCILIATION_WORKER,
        1,
        1,
        1,
        request.pass_count,
        Duration::from_secs(1),
    )
    .map_err(|_| invalid_stored_job())?;
    Ok(DueReconciliationWork {
        tenant_id,
        job_id,
        schema,
        pass_count: request.pass_count,
    })
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), ModuleWorkError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(ModuleWorkError::Source(DISCOVERY_FAILED_CODE.to_owned())),
    }
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

fn due_reconciliation_sql(backend: DbBackend) -> String {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => {}
        _ => unreachable!("scheduler backend was validated"),
    }
    "WITH ranked AS (\
     SELECT tenant_id, job_id, state, module_name, entity_name, schema_version, request, \
     available_at, lease_expires_at, created_at, \
     ROW_NUMBER() OVER (\
       PARTITION BY tenant_id, module_name, entity_name, schema_version \
       ORDER BY CASE state \
         WHEN 'succeeded' THEN 0 \
         WHEN 'running' THEN 1 \
         WHEN 'pending' THEN 2 \
         ELSE 3 END, created_at DESC\
     ) AS scope_rank \
     FROM index_jobs \
     WHERE kind = 'reconcile' \
       AND scope_kind = 'schema' \
       AND state IN ('pending', 'running', 'succeeded', 'failed')\
     ) \
     SELECT tenant_id, job_id, module_name, entity_name, schema_version, request \
     FROM ranked \
     WHERE scope_rank = 1 \
       AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) \
         OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP)) \
     ORDER BY CASE state WHEN 'running' THEN 0 ELSE 1 END, \
       COALESCE(lease_expires_at, available_at), created_at, tenant_id, job_id \
     LIMIT 1"
        .to_owned()
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
    use sea_orm::{Database, DbBackend};

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
        for backend in [DbBackend::Postgres, DbBackend::Sqlite] {
            let sql = due_reconciliation_sql(backend);
            for marker in [
                "ROW_NUMBER() OVER",
                "PARTITION BY tenant_id, module_name, entity_name, schema_version",
                "WHEN 'succeeded' THEN 0",
                "WHEN 'running' THEN 1",
                "WHEN 'pending' THEN 2",
                "scope_rank = 1",
                "state = 'pending' AND available_at <= CURRENT_TIMESTAMP",
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
