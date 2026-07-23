use std::{fs, path::Path, time::Instant};

use anyhow::{Context, Result, ensure};
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
    pub source_entity_rows: i64,
    pub source_link_rows: i64,
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
    pub entity_rows: i64,
    pub link_rows: i64,
    pub workloads: Vec<WorkloadReport>,
}

#[derive(Debug, Serialize)]
pub struct WorkloadReport {
    pub name: &'static str,
    pub sql: String,
    pub result_rows: i64,
    pub result_digest: String,
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

#[derive(Debug, Clone, Copy)]
struct Cardinality {
    entity_rows: i64,
    link_rows: i64,
}

#[derive(Debug)]
struct ResultDigest {
    rows: i64,
    digest: String,
}

pub async fn run(config: &BenchmarkConfig) -> Result<BenchmarkReport> {
    let db = Database::connect(config.database_url.as_str())
        .await
        .context("failed to connect to PostgreSQL")?;
    configure_session(&db).await?;
    let database = read_database_metadata(&db).await?;

    let source_started = Instant::now();
    db.execute_unprepared(&source_dataset_sql(&config.dataset))
        .await
        .context("failed to create deterministic benchmark source dataset")?;
    let source_load_ms = source_started.elapsed().as_millis();
    let source = source_cardinality(&db).await?;
    validate_cardinality("source dataset", source, &config.dataset)?;

    let mut prototypes = Vec::with_capacity(Prototype::ALL.len());
    for prototype in Prototype::ALL {
        prototypes.push(run_prototype(&db, prototype, config).await?);
    }
    validate_semantic_parity(&prototypes)?;

    Ok(BenchmarkReport {
        generated_at: Utc::now(),
        database,
        dataset: config.dataset.clone(),
        source_load_ms,
        source_entity_rows: source.entity_rows,
        source_link_rows: source.link_rows,
        prototypes,
    })
}

pub fn write_report(path: &Path, report: &BenchmarkReport) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
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
    db.execute_unprepared(&full_prototype_sql(prototype))
        .await
        .with_context(|| format!("failed to prepare {:?} prototype", prototype))?;
    let load_ms = load_started.elapsed().as_millis();
    let schema_bytes = schema_size_bytes(db, prototype.schema()).await?;
    let cardinality = prototype_cardinality(db, prototype).await?;
    validate_cardinality(prototype.schema(), cardinality, &config.dataset)?;

    let mut workload_reports = Vec::new();
    for workload in workloads(prototype, &config.dataset) {
        workload_reports.push(run_workload(db, workload, config.repetitions).await?);
    }

    Ok(PrototypeReport {
        prototype,
        schema: prototype.schema(),
        load_ms,
        schema_bytes,
        entity_rows: cardinality.entity_rows,
        link_rows: cardinality.link_rows,
        workloads: workload_reports,
    })
}

async fn run_workload(
    db: &DatabaseConnection,
    workload: Workload,
    repetitions: u32,
) -> Result<WorkloadReport> {
    let digest = result_digest(db, &workload.sql)
        .await
        .with_context(|| format!("failed to digest workload result {}", workload.name))?;
    let mut evidence = Vec::with_capacity(repetitions as usize);
    for _ in 0..repetitions {
        evidence.push(explain(db, &workload.sql).await.with_context(|| {
            format!("failed to execute benchmark workload {}", workload.name)
        })?);
    }
    Ok(WorkloadReport {
        name: workload.name,
        sql: workload.sql,
        result_rows: digest.rows,
        result_digest: digest.digest,
        repetitions: evidence,
    })
}

async fn result_digest(db: &DatabaseConnection, sql: &str) -> Result<ResultDigest> {
    let digest_sql = format!(
        "SELECT count(*)::bigint AS result_rows, md5(COALESCE(string_agg(row_to_json(result)::text, '|' ORDER BY row_to_json(result)::text), '')) AS result_digest FROM ({sql}) AS result"
    );
    let row = db
        .query_one(Statement::from_string(DbBackend::Postgres, digest_sql))
        .await?
        .context("result digest query returned no row")?;
    Ok(ResultDigest {
        rows: row.try_get("", "result_rows")?,
        digest: row.try_get("", "result_digest")?,
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
    db.execute_unprepared("SET jit = off; SET statement_timeout = '30min';")
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
        vec![schema.into()],
    );
    let row = db
        .query_one(statement)
        .await?
        .context("schema size query returned no row")?;
    row.try_get("", "bytes").map_err(Into::into)
}

async fn source_cardinality(db: &DatabaseConnection) -> Result<Cardinality> {
    cardinality_query(
        db,
        "SELECT ((SELECT count(*) FROM idx_bench_source.product) + (SELECT count(*) FROM idx_bench_source.variant) + (SELECT count(*) FROM idx_bench_source.channel))::bigint AS entity_rows, ((SELECT count(*) FROM idx_bench_source.variant) + (SELECT count(*) FROM idx_bench_source.variant_channel))::bigint AS link_rows",
    )
    .await
}

async fn prototype_cardinality(
    db: &DatabaseConnection,
    prototype: Prototype,
) -> Result<Cardinality> {
    let sql = match prototype {
        Prototype::Jsonb => "SELECT (SELECT count(*) FROM idx_bench_jsonb.entity)::bigint AS entity_rows, (SELECT count(*) FROM idx_bench_jsonb.link)::bigint AS link_rows".to_owned(),
        Prototype::TypedEav => "SELECT (SELECT count(*) FROM idx_bench_eav.entity)::bigint AS entity_rows, (SELECT count(*) FROM idx_bench_eav.link)::bigint AS link_rows".to_owned(),
        Prototype::HotProjection => "SELECT ((SELECT count(*) FROM idx_bench_hot.product) + (SELECT count(*) FROM idx_bench_hot.variant) + (SELECT count(*) FROM idx_bench_hot.sales_channel))::bigint AS entity_rows, (SELECT count(*) FROM idx_bench_hot.link)::bigint AS link_rows".to_owned(),
    };
    cardinality_query(db, &sql).await
}

async fn cardinality_query(db: &DatabaseConnection, sql: &str) -> Result<Cardinality> {
    let row = db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .context("cardinality query returned no row")?;
    Ok(Cardinality {
        entity_rows: row.try_get("", "entity_rows")?,
        link_rows: row.try_get("", "link_rows")?,
    })
}

fn validate_cardinality(
    label: &str,
    actual: Cardinality,
    dataset: &DatasetConfig,
) -> Result<()> {
    let expected_entities = i64::try_from(dataset.total_entity_rows())
        .context("expected entity cardinality exceeds i64")?;
    let expected_links =
        i64::try_from(dataset.total_link_rows()).context("expected link cardinality exceeds i64")?;
    ensure!(
        actual.entity_rows == expected_entities,
        "{label} entity cardinality drift: expected {expected_entities}, got {}",
        actual.entity_rows
    );
    ensure!(
        actual.link_rows == expected_links,
        "{label} link cardinality drift: expected {expected_links}, got {}",
        actual.link_rows
    );
    Ok(())
}

fn validate_semantic_parity(prototypes: &[PrototypeReport]) -> Result<()> {
    let baseline = prototypes
        .first()
        .context("benchmark produced no prototype reports")?;
    for candidate in &prototypes[1..] {
        ensure!(
            candidate.workloads.len() == baseline.workloads.len(),
            "{} workload count differs from {}",
            candidate.schema,
            baseline.schema
        );
        for expected in &baseline.workloads {
            let actual = candidate
                .workloads
                .iter()
                .find(|workload| workload.name == expected.name)
                .with_context(|| {
                    format!("{} is missing workload {}", candidate.schema, expected.name)
                })?;
            ensure!(
                actual.result_rows == expected.result_rows,
                "{} workload {} row-count mismatch: expected {}, got {}",
                candidate.schema,
                expected.name,
                expected.result_rows,
                actual.result_rows
            );
            ensure!(
                actual.result_digest == expected.result_digest,
                "{} workload {} result digest differs from {}",
                candidate.schema,
                expected.name,
                baseline.schema
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_storage::DatasetScale;

    #[test]
    fn cardinality_contract_matches_generated_link_shape() {
        let dataset = DatasetConfig::for_scale(
            DatasetScale::Smoke,
            vec!["en-US".to_owned(), "ru-RU".to_owned()],
        )
        .unwrap();
        assert_eq!(dataset.total_entity_rows(), 1_216);
        assert_eq!(dataset.total_link_rows(), 2_400);
    }
}
