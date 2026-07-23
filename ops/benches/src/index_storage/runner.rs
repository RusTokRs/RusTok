use std::{fs, path::Path, time::Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TryGetable};
use serde::Serialize;
use serde_json::Value;

use super::{
    BenchmarkConfig, DatasetConfig, Prototype, Workload, full_prototype_sql, source_dataset_sql,
    workloads,
};

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub generated_at: DateTime<Utc>,
    pub database: DatabaseMetadata,
    pub dataset: DatasetConfig,
    pub source_load_ms: u128,
    pub prototypes: Vec<PrototypeReport>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseMetadata {
    pub version: String,
    pub server_version_num: String,
    pub shared_buffers: String,
    pub effective_cache_size: String,
    pub work_mem: String,
    pub random_page_cost: String,
    pub jit: String,
}

#[derive(Debug, Serialize)]
pub struct PrototypeReport {
    pub prototype: Prototype,
    pub schema: &'static str,
    pub load_ms: u128,
    pub schema_bytes: i64,
    pub workloads: Vec<WorkloadReport>,
}

#[derive(Debug, Serialize)]
pub struct WorkloadReport {
    pub name: &'static str,
    pub sql: String,
    pub repetitions: Vec<ExplainEvidence>,
}

#[derive(Debug, Serialize)]
pub struct ExplainEvidence {
    pub planning_time_ms: Option<f64>,
    pub execution_time_ms: Option<f64>,
    pub shared_hit_blocks: Option<u64>,
    pub shared_read_blocks: Option<u64>,
    pub temporary_read_blocks: Option<u64>,
    pub temporary_written_blocks: Option<u64>,
    pub plan: Value,
}

pub async fn run(config: &BenchmarkConfig) -> Result<BenchmarkReport> {
    let db = Database::connect(&config.database_url)
        .await
        .context("failed to connect to PostgreSQL")?;
    configure_session(&db).await?;
    let database = read_database_metadata(&db).await?;

    let source_started = Instant::now();
    db.execute_unprepared(&source_dataset_sql(&config.dataset))
        .await
        .context("failed to create deterministic benchmark source dataset")?;
    let source_load_ms = source_started.elapsed().as_millis();

    let mut prototypes = Vec::with_capacity(Prototype::ALL.len());
    for prototype in Prototype::ALL {
        prototypes.push(run_prototype(&db, prototype, config).await?);
    }

    Ok(BenchmarkReport {
        generated_at: Utc::now(),
        database,
        dataset: config.dataset.clone(),
        source_load_ms,
        prototypes,
    })
}

pub fn write_report(path: &Path, report: &BenchmarkReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create benchmark output directory {parent:?}"))?;
    }
    let json = serde_json::to_vec_pretty(report).context("failed to serialize benchmark report")?;
    fs::write(path, json)
        .with_context(|| format!("failed to write benchmark report to {path:?}"))?;
    Ok(())
}

async fn run_prototype(
    db: &DatabaseConnection,
    prototype: Prototype,
    config: &BenchmarkConfig,
) -> Result<PrototypeReport> {
    let load_started = Instant::now();
    db.execute_unprepared(&executable_prototype_sql(prototype))
        .await
        .with_context(|| format!("failed to prepare {:?} prototype", prototype))?;
    let load_ms = load_started.elapsed().as_millis();
    let schema_bytes = schema_size_bytes(db, prototype.schema()).await?;

    let mut workload_reports = Vec::new();
    for workload in workloads(prototype, &config.dataset) {
        workload_reports.push(run_workload(db, workload, config.repetitions).await?);
    }

    Ok(PrototypeReport {
        prototype,
        schema: prototype.schema(),
        load_ms,
        schema_bytes,
        workloads: workload_reports,
    })
}

async fn run_workload(
    db: &DatabaseConnection,
    workload: Workload,
    repetitions: u32,
) -> Result<WorkloadReport> {
    let mut evidence = Vec::with_capacity(repetitions as usize);
    for _ in 0..repetitions {
        evidence.push(explain(db, &workload.sql).await.with_context(|| {
            format!("failed to execute benchmark workload {}", workload.name)
        })?);
    }
    Ok(WorkloadReport {
        name: workload.name,
        sql: workload.sql,
        repetitions: evidence,
    })
}

async fn explain(db: &DatabaseConnection, sql: &str) -> Result<ExplainEvidence> {
    let statement = Statement::from_string(
        DbBackend::Postgres,
        format!("EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON) {sql}"),
    );
    let row = db
        .query_one(statement)
        .await
        .context("EXPLAIN query failed")?
        .context("EXPLAIN returned no row")?;
    let plan: Value = row
        .try_get("", "QUERY PLAN")
        .context("EXPLAIN result did not contain QUERY PLAN JSON")?;
    let root = plan.get(0).unwrap_or(&Value::Null);
    let plan_node = root.get("Plan").unwrap_or(&Value::Null);

    Ok(ExplainEvidence {
        planning_time_ms: root.get("Planning Time").and_then(Value::as_f64),
        execution_time_ms: root.get("Execution Time").and_then(Value::as_f64),
        shared_hit_blocks: plan_node.get("Shared Hit Blocks").and_then(Value::as_u64),
        shared_read_blocks: plan_node.get("Shared Read Blocks").and_then(Value::as_u64),
        temporary_read_blocks: plan_node.get("Temp Read Blocks").and_then(Value::as_u64),
        temporary_written_blocks: plan_node.get("Temp Written Blocks").and_then(Value::as_u64),
        plan,
    })
}

async fn configure_session(db: &DatabaseConnection) -> Result<()> {
    db.execute_unprepared(
        "SET jit = off; SET track_io_timing = on; SET statement_timeout = '30min';",
    )
    .await
    .context("failed to configure benchmark PostgreSQL session")?;
    Ok(())
}

async fn read_database_metadata(db: &DatabaseConnection) -> Result<DatabaseMetadata> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT version() AS version, current_setting('server_version_num') AS server_version_num, current_setting('shared_buffers') AS shared_buffers, current_setting('effective_cache_size') AS effective_cache_size, current_setting('work_mem') AS work_mem, current_setting('random_page_cost') AS random_page_cost, current_setting('jit') AS jit".to_owned(),
        ))
        .await?
        .context("database metadata query returned no row")?;

    Ok(DatabaseMetadata {
        version: row.try_get("", "version")?,
        server_version_num: row.try_get("", "server_version_num")?,
        shared_buffers: row.try_get("", "shared_buffers")?,
        effective_cache_size: row.try_get("", "effective_cache_size")?,
        work_mem: row.try_get("", "work_mem")?,
        random_page_cost: row.try_get("", "random_page_cost")?,
        jit: row.try_get("", "jit")?,
    })
}

async fn schema_size_bytes(db: &DatabaseConnection, schema: &str) -> Result<i64> {
    let statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT COALESCE(sum(pg_total_relation_size(class.oid)), 0)::bigint AS bytes FROM pg_class AS class JOIN pg_namespace AS namespace ON namespace.oid = class.relnamespace WHERE namespace.nspname = $1 AND class.relkind IN ('r', 'm')",
        [schema.into()],
    );
    let row = db
        .query_one(statement)
        .await?
        .context("schema size query returned no row")?;
    row.try_get("", "bytes").map_err(Into::into)
}

fn executable_prototype_sql(prototype: Prototype) -> String {
    let invalid_suffix = format!(
        "\nANALYZE {};\nANALYZE {}.link;\n",
        prototype.schema(),
        prototype.schema()
    );
    let analyze = match prototype {
        Prototype::Jsonb => format!(
            "\nANALYZE {}.entity;\nANALYZE {}.link;\n",
            prototype.schema(),
            prototype.schema()
        ),
        Prototype::TypedEav => format!(
            "\nANALYZE {}.entity;\nANALYZE {}.field_value;\nANALYZE {}.link;\n",
            prototype.schema(),
            prototype.schema(),
            prototype.schema()
        ),
        Prototype::HotProjection => format!(
            "\nANALYZE {}.product;\nANALYZE {}.variant;\nANALYZE {}.sales_channel;\nANALYZE {}.link;\n",
            prototype.schema(),
            prototype.schema(),
            prototype.schema(),
            prototype.schema()
        ),
    };
    full_prototype_sql(prototype).replace(&invalid_suffix, &analyze)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_sql_analyzes_real_relations() {
        for prototype in Prototype::ALL {
            let sql = executable_prototype_sql(prototype);
            assert!(!sql.contains(&format!("ANALYZE {};", prototype.schema())));
            assert!(sql.contains(&format!("ANALYZE {}.link;", prototype.schema())));
        }
    }
}
