//! Generic bounded rebuild scheduling through the canonical module-work lifecycle.
//! The runner owns durable lease/checkpoint/terminal transitions; jobs carry no read authority.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_runtime::{
    HostRuntimeContext, ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome,
    ModuleWorkRegistration, ModuleWorkRegistrations, ModuleWorkScheduler, ModuleWorkSource,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::source_replay_job::stored_job;
use crate::{
    EntityName, IndexReplayJobLeaseRequest, IndexReplayRunRequest, IndexReplayRunStatus, LocaleKey,
    ModuleName, SchemaRef, SchemaVersion, SharedIndexReplayRuntime, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry,
};

pub const INDEX_REPLAY_WORKER: &str = "index_replay";
const FAILURE: &str = "index.replay_scheduler.unverified";
const WORK_CONTRACT: &str = "index_replay_scheduler_item_v1";

#[derive(Clone)]
struct ReplayWorkRegistered;

/// Register only after the immutable source/schema capabilities exist. Performs no SQL.
pub fn register_postgres_index_replay_work(
    extensions: &mut ModuleRuntimeExtensions,
) -> Result<bool, ModuleWorkError> {
    if extensions.contains::<ReplayWorkRegistered>() {
        return Err(ModuleWorkError::DuplicateHandler(
            INDEX_REPLAY_WORKER.into(),
        ));
    }
    if !extensions.contains::<SharedIndexSourceRegistry>() {
        return Ok(false);
    }
    if !extensions.contains::<SharedIndexSchemaRegistry>() {
        return Err(unverified());
    }
    extensions
        .get_or_insert_with::<ModuleWorkRegistrations, _>(Default::default)
        .register(Arc::new(ReplayWorkRegistration));
    extensions.insert(ReplayWorkRegistered);
    Ok(true)
}

struct ReplayWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for ReplayWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let sources = host
            .shared_get::<SharedIndexSourceRegistry>()
            .ok_or_else(unverified)?;
        let schemas = host
            .shared_get::<SharedIndexSchemaRegistry>()
            .ok_or_else(unverified)?;
        let runtime = host
            .shared_get::<SharedIndexReplayRuntime>()
            .ok_or_else(unverified)?;
        let adapter = Arc::new(PostgresIndexReplayWorkAdapter {
            db: host.db_clone(),
            runtime,
            schemas,
            sources,
        });
        scheduler.register(adapter.clone(), adapter).await
    }
}

#[derive(Clone)]
pub struct PostgresIndexReplayWorkAdapter {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    runtime: SharedIndexReplayRuntime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayWorkPayload {
    contract: String,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
}

impl PostgresIndexReplayWorkAdapter {
    fn decode(&self, item: &ModuleWorkItem) -> Result<(ReplayWorkPayload, Uuid), ModuleWorkError> {
        if item.worker_slug != INDEX_REPLAY_WORKER || item.id.is_nil() || item.tenant_id.is_nil() {
            return Err(unverified());
        }
        let invocation = Uuid::parse_str(&item.lease_token).map_err(|_| unverified())?;
        let payload: ReplayWorkPayload =
            serde_json::from_value(item.payload.clone()).map_err(|_| unverified())?;
        if invocation.is_nil()
            || payload.contract != WORK_CONTRACT
            || self.sources.source_for_schema(&payload.schema).is_none()
        {
            return Err(unverified());
        }
        Ok((payload, invocation))
    }
}

#[async_trait]
impl ModuleWorkSource for PostgresIndexReplayWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != INDEX_REPLAY_WORKER {
            return Ok(None);
        }
        let backend = self.db.get_database_backend();
        if backend != DbBackend::Postgres && !(cfg!(test) && backend == DbBackend::Sqlite) {
            return Err(unverified());
        }
        // Restrict discovery to immutable, actually selected bindings. An earlier retained job
        // for a removed/unselected source must not starve work for another tenant/source.
        let mut scopes = Vec::new();
        let mut values: Vec<SqlValue> = Vec::new();
        let source_name = match backend {
            DbBackend::Postgres => "request->>'source_name'",
            DbBackend::Sqlite => "json_extract(request, '$.source_name')",
            _ => return Err(unverified()),
        };
        for registered in self.schemas.registry().iter() {
            let reference = &registered.schema.reference;
            let Some(source) = self.sources.source_for_schema(reference) else {
                continue;
            };
            let offset = values.len();
            scopes.push(format!(
                "(module_name = ${} AND entity_name = ${} AND schema_version = ${} AND {source_name} = ${})",
                offset + 1, offset + 2, offset + 3, offset + 4,
            ));
            values.extend([
                reference.module.as_str().to_owned().into(),
                reference.entity.as_str().to_owned().into(),
                i64::from(reference.version.get()).into(),
                source.source_name().to_owned().into(),
            ]);
        }
        if scopes.is_empty() {
            return Ok(None);
        }
        let row = self.db.query_one_raw(Statement::from_sql_and_values(backend,
            format!("SELECT tenant_id, job_id, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, scope_kind, locale_key, state, request, last_error_code, CAST(attempt_count AS BIGINT) AS attempt_count_value, TRUE AS claimable FROM index_jobs WHERE kind = 'rebuild' AND ({}) AND ((state = 'pending' AND available_at <= CURRENT_TIMESTAMP) OR (state = 'running' AND lease_expires_at <= CURRENT_TIMESTAMP)) ORDER BY available_at, created_at, job_id LIMIT 1", scopes.join(" OR ")),
            values,
        )).await.map_err(|_| unverified())?;
        let Some(row) = row else {
            return Ok(None);
        };
        let tenant_id = stored_uuid(&row, "tenant_id", backend)?;
        let saved = stored_job(&row, backend).map_err(|_| unverified())?;
        let module: String = row.try_get("", "module_name").map_err(|_| unverified())?;
        let entity: String = row.try_get("", "entity_name").map_err(|_| unverified())?;
        let version: i64 = row
            .try_get("", "schema_version_value")
            .map_err(|_| unverified())?;
        let version = u32::try_from(version).map_err(|_| unverified())?;
        if tenant_id.is_nil() || saved.job_id.is_nil() || version == 0 {
            return Err(unverified());
        }
        let schema = SchemaRef {
            module: ModuleName::new(module).map_err(|_| unverified())?,
            entity: EntityName::new(entity).map_err(|_| unverified())?,
            version: SchemaVersion::new(version),
        };
        let source = self
            .sources
            .source_for_schema(&schema)
            .ok_or_else(unverified)?;
        if source.source_name() != saved.source_name {
            return Err(unverified());
        }
        // Discovery is not a lease. A distinct invocation token and the exact original job UUID
        // reach the canonical runner, whose scope lock/attempt fence arbitrate concurrent hosts.
        Ok(Some(ModuleWorkItem {
            id: saved.job_id,
            tenant_id,
            worker_slug: INDEX_REPLAY_WORKER.into(),
            lease_token: Uuid::new_v4().to_string(),
            payload: serde_json::to_value(ReplayWorkPayload {
                contract: WORK_CONTRACT.into(),
                schema,
                locale: saved.locale,
            })
            .map_err(|_| unverified())?,
        }))
    }

    async fn complete(
        &self,
        item: &ModuleWorkItem,
        outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        let (payload, _) = self.decode(item)?;
        let backend = self.db.get_database_backend();
        let row = self.db.query_one_raw(Statement::from_sql_and_values(
            backend,
            "SELECT job_id, state, scope_kind, locale_key, request, last_error_code, CAST(attempt_count AS BIGINT) AS attempt_count_value, FALSE AS claimable, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value FROM index_jobs WHERE tenant_id = $1 AND job_id = $2 AND kind = 'rebuild'",
            vec![super::source_replay_job::uuid_value(item.tenant_id, backend), super::source_replay_job::uuid_value(item.id, backend)],
        )).await.map_err(|_| unverified())?.ok_or_else(unverified)?;
        let saved = stored_job(&row, backend).map_err(|_| unverified())?;
        let module: String = row.try_get("", "module_name").map_err(|_| unverified())?;
        let entity: String = row.try_get("", "entity_name").map_err(|_| unverified())?;
        let version: i64 = row
            .try_get("", "schema_version_value")
            .map_err(|_| unverified())?;
        let source = self
            .sources
            .source_for_schema(&payload.schema)
            .ok_or_else(unverified)?;
        if saved.job_id != item.id
            || saved.source_name != source.source_name()
            || saved.locale != payload.locale
            || module != payload.schema.module.as_str()
            || entity != payload.schema.entity.as_str()
            || version != i64::from(payload.schema.version.get())
        {
            return Err(unverified());
        }
        // No independent ACK ledger: verify the runner's durable result rather than manufacture
        // success. Another host may already have advanced a yielded/busy job by this point.
        let verified = match outcome {
            ModuleWorkOutcome::Completed => saved.state == "succeeded",
            ModuleWorkOutcome::Cancelled => saved.state == "cancelled",
            ModuleWorkOutcome::Held { .. } => matches!(
                saved.state.as_str(),
                "pending" | "running" | "succeeded" | "failed" | "cancelled"
            ),
            ModuleWorkOutcome::Retryable { .. } => {
                saved.attempt_count > 0
                    && matches!(
                        saved.state.as_str(),
                        "pending" | "running" | "succeeded" | "failed" | "cancelled"
                    )
            }
            ModuleWorkOutcome::Rejected { .. } => {
                matches!(saved.state.as_str(), "failed" | "cancelled")
            }
        };
        if !verified {
            return Err(unverified());
        }
        if saved.state == "succeeded" {
            let scope = match payload.locale {
                Some(locale) => IndexReplayJobLeaseRequest::for_locale(
                    item.tenant_id,
                    payload.schema,
                    locale,
                    source.source_name(),
                    "index-replay-result-check",
                    Duration::from_secs(1),
                ),
                None => IndexReplayJobLeaseRequest::new(
                    item.tenant_id,
                    payload.schema,
                    source.source_name(),
                    "index-replay-result-check",
                    Duration::from_secs(1),
                ),
            }
            .map_err(|_| unverified())?;
            super::source_replay_job::require_complete_checkpoint_for_scope(
                &self.db, &scope, backend,
            )
            .await
            .map_err(|_| unverified())?;
        }
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for PostgresIndexReplayWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        INDEX_REPLAY_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let (payload, invocation) = self.decode(&item)?;
        let worker = format!("index-replay-{}", invocation.simple());
        let request = match payload.locale {
            Some(locale) => IndexReplayRunRequest::for_locale(
                item.tenant_id,
                payload.schema,
                locale,
                worker,
                128,
                8,
                1,
                Duration::from_secs(300),
            ),
            None => IndexReplayRunRequest::new(
                item.tenant_id,
                payload.schema,
                worker,
                128,
                8,
                1,
                Duration::from_secs(300),
            ),
        }
        .and_then(|request| request.for_job(item.id))
        .map_err(|_| unverified())?;
        let result = self.runtime.run(request).await.map_err(|_| unverified())?;
        Ok(match result.status() {
            IndexReplayRunStatus::Cancelled => ModuleWorkOutcome::Cancelled,
            IndexReplayRunStatus::Busy => ModuleWorkOutcome::Held {
                message: "index.replay_busy".into(),
            },
            IndexReplayRunStatus::Yielded => ModuleWorkOutcome::Retryable {
                message: "index.replay_page_budget_yielded".into(),
            },
            IndexReplayRunStatus::AlreadyComplete | IndexReplayRunStatus::Complete => {
                ModuleWorkOutcome::Completed
            }
        })
    }
}

fn stored_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ModuleWorkError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(|_| unverified()),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(|_| unverified())?;
            Uuid::parse_str(&value).map_err(|_| unverified())
        }
        _ => Err(unverified()),
    }
}

fn unverified() -> ModuleWorkError {
    ModuleWorkError::Source(FAILURE.into())
}

#[cfg(test)]
#[path = "source_replay_scheduler_tests.rs"]
mod tests;
