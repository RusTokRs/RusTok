use std::collections::BTreeMap;

use rustok_api::{HostRuntimeContext, TenantContext};
use rustok_core::RusToKModule;
use rustok_index::{
    IndexSchema, IndexSchemaSourceCatalog, PostgresIndexQueryAdmissionCatalog, SchemaRef,
    SharedIndexQueryRuntime, SharedIndexReplayRuntime, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry,
};
use sea_orm::{
    ConnectionTrait, DatabaseTransaction, DbBackend, DbErr, QueryResult, Statement,
    TransactionTrait,
};
use uuid::Uuid;

use crate::model::*;

pub(super) async fn read(
    host: &HostRuntimeContext,
    tenant: TenantContext,
    can_manage: bool,
) -> Result<IndexAdminBootstrap, DbErr> {
    let db = host.db_clone();
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(DbErr::Custom("Index admin requires PostgreSQL".to_owned()));
    }
    let transaction = db.begin().await?;
    transaction
        .execute_unprepared("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await?;
    transaction
        .execute_unprepared("SET LOCAL statement_timeout = '2s'")
        .await?;
    transaction
        .execute_unprepared("SET LOCAL TIME ZONE 'UTC'")
        .await?;

    let source_registry = host.shared_get::<SharedIndexSourceRegistry>();
    let schema_registry = host.shared_get::<SharedIndexSchemaRegistry>();
    let catalog = host.shared_get::<IndexSchemaSourceCatalog>();
    let schemas = schema_snapshot(
        &transaction,
        tenant.id,
        schema_registry.as_ref(),
        catalog.as_ref(),
        source_registry.as_ref(),
    )
    .await?;
    let sources = source_registry.as_ref().map(|registry| {
        registry
            .iter()
            .flat_map(|source| {
                source
                    .schemas()
                    .iter()
                    .map(move |schema| IndexSourceDescriptorSnapshot {
                        name: source.source_name().to_owned(),
                        entity: schema.to_string(),
                        owner_module: source.owner_module().to_owned(),
                    })
            })
            .collect()
    });
    let query_diagnostics = host.shared_get::<SharedIndexQueryRuntime>().map(|_| {
        // The runtime uses the same empty catalog when no owner registered admission rules.
        let admissions = host
            .shared_get::<PostgresIndexQueryAdmissionCatalog>()
            .unwrap_or_default();
        IndexQueryDiagnosticsSnapshot {
            admission_rules_count: admissions.iter().filter(|rule| rule.is_governed()).count(),
            link_availability_rules_count: admissions.link_availability_len(),
        }
    });
    let inbox_metrics = inbox_metrics(&transaction, tenant.id).await?;
    let job_metrics = job_metrics(&transaction, tenant.id).await?;
    let recent_jobs = recent_jobs(&transaction, tenant.id).await?;
    let partitioned: bool = transaction
        .query_one_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT relkind = 'p' AS partitioned FROM pg_catalog.pg_class WHERE oid = to_regclass('index_entities')"))
        .await?.ok_or_else(invalid_snapshot)?.try_get("", "partitioned")?;
    transaction.commit().await?;

    let module = rustok_index::IndexModule;
    let replay_available = host.shared_get::<SharedIndexReplayRuntime>().is_some();
    Ok(IndexAdminBootstrap {
        tenant: IndexTenantSnapshot {
            id: tenant.id.to_string(),
            slug: tenant.slug,
            name: tenant.name,
            default_locale: tenant.default_locale,
        },
        module: IndexModuleSnapshot {
            slug: module.slug().to_owned(),
            name: module.name().to_owned(),
            description: module.description().to_owned(),
            version: module.version().to_owned(),
        },
        storage: IndexStorageSnapshot {
            backend: "PostgreSQL".to_owned(),
            layout: "JSONB".to_owned(),
            partition_status: if partitioned {
                "partitioned"
            } else {
                "unpartitioned"
            }
            .to_owned(),
            tables: [
                ("index_entities", "Materialized records and source versions"),
                ("index_links", "Scoped directional projection links"),
                (
                    "index_schemas",
                    "Tenant schema registrations and fingerprints",
                ),
                ("index_inbox", "Mutation delivery and deduplication state"),
                ("index_jobs", "Durable jobs and lease generations"),
                ("index_checkpoints", "Replay checkpoints"),
                ("index_schema_leases", "Schema coordination leases"),
                (
                    "index_job_recovery_audits",
                    "Append-only operator requeue audit",
                ),
            ]
            .into_iter()
            .map(|(name, role)| IndexTableSnapshot {
                name: name.to_owned(),
                role: role.to_owned(),
            })
            .collect(),
        },
        schemas,
        operations: IndexOperationsSnapshot {
            replay_available,
            can_manage,
            registered_sources: sources,
            inbox_metrics,
            job_metrics,
            query_diagnostics,
            recent_jobs,
        },
    })
}

const MAX_SCHEMA_ROWS: usize = 512;

async fn schema_snapshot(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
    registry: Option<&SharedIndexSchemaRegistry>,
    catalog: Option<&IndexSchemaSourceCatalog>,
    sources: Option<&SharedIndexSourceRegistry>,
) -> Result<Vec<IndexSchemaSnapshot>, DbErr> {
    let rows = db.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT module_name, entity_name, schema_version, schema_fingerprint, schema_json, status FROM index_schemas WHERE tenant_id = $1 ORDER BY module_name, entity_name, schema_version LIMIT $2",
        vec![tenant_id.into(), ((MAX_SCHEMA_ROWS + 1) as i64).into()],
    )).await?;
    if rows.len() > MAX_SCHEMA_ROWS {
        return Err(DbErr::Custom(
            "Index admin schema snapshot budget exceeded".to_owned(),
        ));
    }
    let mut snapshots = BTreeMap::new();
    for row in rows {
        let version: i32 = row.try_get("", "schema_version")?;
        let reference = SchemaRef {
            module: rustok_index::ModuleName::new(row.try_get::<String>("", "module_name")?)
                .map_err(|_| invalid_snapshot())?,
            entity: rustok_index::EntityName::new(row.try_get::<String>("", "entity_name")?)
                .map_err(|_| invalid_snapshot())?,
            version: rustok_index::SchemaVersion::new(
                u32::try_from(version).map_err(|_| invalid_snapshot())?,
            ),
        };
        let fingerprint: String = row.try_get("", "schema_fingerprint")?;
        let stored: serde_json::Value = row.try_get("", "schema_json")?;
        let schema: IndexSchema = serde_json::from_value(stored).map_err(|_| invalid_snapshot())?;
        let status: String = row.try_get("", "status")?;
        if schema.reference != reference {
            return Err(invalid_snapshot());
        }
        let readiness = match registry.and_then(|registry| registry.registry().get(&reference)) {
            _ if status != "active" => SchemaReadiness::Inactive,
            None => SchemaReadiness::Unselected,
            Some(registered)
                if registered.fingerprint.to_string() == fingerprint
                    && registered.schema.canonical() == schema.canonical() =>
            {
                SchemaReadiness::Ready
            }
            Some(_) => SchemaReadiness::ContractMismatch,
        };
        let module = owner_for(&reference, catalog, sources);
        snapshots.insert(
            reference.clone(),
            schema_row(&schema, fingerprint, readiness, module, sources),
        );
    }
    if let Some(registry) = registry {
        for registered in registry.registry().iter() {
            let schema = &registered.schema;
            snapshots
                .entry(schema.reference.clone())
                .or_insert_with(|| {
                    schema_row(
                        schema,
                        registered.fingerprint.to_string(),
                        SchemaReadiness::Missing,
                        owner_for(&schema.reference, catalog, sources),
                        sources,
                    )
                });
            if snapshots.len() > MAX_SCHEMA_ROWS {
                return Err(DbErr::Custom(
                    "Index admin schema snapshot budget exceeded".to_owned(),
                ));
            }
        }
    }
    Ok(snapshots.into_values().collect())
}

fn owner_for(
    reference: &SchemaRef,
    catalog: Option<&IndexSchemaSourceCatalog>,
    sources: Option<&SharedIndexSourceRegistry>,
) -> Option<String> {
    catalog
        .and_then(|catalog| catalog.get(reference))
        .map(|descriptor| descriptor.owner_module.clone())
        .or_else(|| {
            sources
                .and_then(|sources| sources.source_for_schema(reference))
                .map(|source| source.owner_module().to_owned())
        })
}

fn schema_row(
    schema: &IndexSchema,
    fingerprint: String,
    readiness: SchemaReadiness,
    owner_module: Option<String>,
    sources: Option<&SharedIndexSourceRegistry>,
) -> IndexSchemaSnapshot {
    IndexSchemaSnapshot {
        module: schema.reference.module.as_str().to_owned(),
        entity: schema.reference.entity.as_str().to_owned(),
        version: schema.reference.version.get(),
        fingerprint,
        field_count: schema.fields.len(),
        link_count: schema.links.len(),
        owner_module,
        readiness,
        source_selected: sources
            .and_then(|sources| sources.source_for_schema(&schema.reference))
            .is_some(),
    }
}

async fn inbox_metrics(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<IndexInboxMetricsSnapshot, DbErr> {
    let row = tenant_summary(db, tenant_id,
        "SELECT count(*) AS total, count(*) FILTER (WHERE state = 'pending') AS pending,
            count(*) FILTER (WHERE state = 'processing') AS processing,
            count(*) FILTER (WHERE state = 'applied') AS applied,
            count(*) FILTER (WHERE state = 'rejected') AS rejected,
            EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - min(created_at) FILTER (WHERE state = 'pending')))::bigint AS lag
         FROM index_inbox WHERE tenant_id = $1").await?;
    Ok(IndexInboxMetricsSnapshot {
        total_messages: count(&row, "total")?,
        pending_messages: count(&row, "pending")?,
        processing_messages: count(&row, "processing")?,
        applied_messages: count(&row, "applied")?,
        rejected_messages: count(&row, "rejected")?,
        oldest_pending_age_seconds: row
            .try_get::<Option<i64>>("", "lag")?
            .map(|lag| lag.max(0) as u64),
    })
}

async fn job_metrics(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<IndexJobMetricsSnapshot, DbErr> {
    let row = tenant_summary(
        db,
        tenant_id,
        "SELECT count(*) AS total, count(*) FILTER (WHERE state = 'pending') AS pending,
            count(*) FILTER (WHERE state = 'running') AS running,
            count(*) FILTER (WHERE state = 'succeeded') AS succeeded,
            count(*) FILTER (WHERE state = 'failed') AS failed,
            count(*) FILTER (WHERE state = 'cancelled') AS cancelled,
            (SELECT count(*) FROM index_job_recovery_audits WHERE tenant_id = $1) AS recovery
         FROM index_jobs WHERE tenant_id = $1",
    )
    .await?;
    Ok(IndexJobMetricsSnapshot {
        total_jobs: count(&row, "total")?,
        pending_jobs: count(&row, "pending")?,
        running_jobs: count(&row, "running")?,
        succeeded_jobs: count(&row, "succeeded")?,
        failed_jobs: count(&row, "failed")?,
        cancelled_jobs: count(&row, "cancelled")?,
        retry_recovery_count: count(&row, "recovery")?,
    })
}

async fn recent_jobs(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<Vec<IndexJobSnapshot>, DbErr> {
    db.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT job_id, kind, state, module_name || '.' || entity_name || '@' || schema_version AS schema,
                locale_key, failure_count, cancel_requested, CASE WHEN state = 'pending' THEN available_at::text END AS available_at,
                last_error_code FROM index_jobs WHERE tenant_id = $1 AND kind IN ('rebuild', 'reconcile')
         AND scope_kind IN ('schema', 'locale') ORDER BY updated_at DESC, job_id DESC LIMIT 32",
        [tenant_id.into()],
    )).await?.into_iter().map(|row| {
        Ok(IndexJobSnapshot {
            job_id: row.try_get::<Uuid>("", "job_id")?.to_string(),
            kind: row.try_get("", "kind")?,
            state: row.try_get("", "state")?,
            schema: row.try_get("", "schema")?,
            locale: row.try_get("", "locale_key")?,
            cancel_requested: row.try_get("", "cancel_requested")?,
            failure_count: u32::try_from(row.try_get::<i32>("", "failure_count")?).map_err(|_| invalid_snapshot())?,
            available_at: row.try_get("", "available_at")?,
            last_error_code: row.try_get("", "last_error_code")?,
        })
    }).collect()
}

async fn tenant_summary(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
    sql: &str,
) -> Result<QueryResult, DbErr> {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [tenant_id.into()],
    ))
    .await?
    .ok_or_else(invalid_snapshot)
}

fn count(row: &QueryResult, alias: &str) -> Result<u64, DbErr> {
    u64::try_from(row.try_get::<i64>("", alias)?).map_err(|_| invalid_snapshot())
}

fn invalid_snapshot() -> DbErr {
    DbErr::Custom("invalid persisted Index snapshot".to_owned())
}
