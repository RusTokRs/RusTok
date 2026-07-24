use std::{fmt::Write as _, fs, path::Path, time::Instant};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;
use serde_json::Value;

use super::{
    BenchmarkConfig, DatasetConfig, Prototype, Workload, connect_benchmark_database,
    explain::parse_read_explain_metrics, full_prototype_sql, read_workload_contract,
    source_dataset_sql, source_workloads, workloads,
};

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub generated_at: DateTime<Utc>,
    pub database: DatabaseMetadata,
    pub dataset: DatasetConfig,
    pub source_load_ms: u128,
    pub source_entity_rows: i64,
    pub source_link_rows: i64,
    pub source_workloads: Vec<SourceWorkloadReport>,
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
pub struct SourceWorkloadReport {
    pub name: &'static str,
    pub sql: String,
    pub result_rows: i64,
    pub result_digest: String,
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
    pub planning_time_ms: f64,
    pub execution_time_ms: f64,
    pub shared_hit_blocks: u64,
    pub shared_read_blocks: u64,
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
    let db = connect_benchmark_database(&config.database_url).await?;
    configure_session(&db).await?;
    let database = read_database_metadata(&db).await?;

    let source_started = Instant::now();
    db.execute_unprepared(&source_dataset_sql(&config.dataset))
        .await
        .context("failed to create deterministic benchmark source dataset")?;
    let source_load_ms = source_started.elapsed().as_millis();
    let source = source_cardinality(&db).await?;
    validate_cardinality("source dataset", source, &config.dataset)?;
    let source_workload_reports = run_source_workloads(&db, &config.dataset).await?;

    let mut prototypes = Vec::with_capacity(Prototype::ALL.len());
    for prototype in Prototype::ALL {
        prototypes.push(run_prototype(&db, prototype, config).await?);
    }
    validate_semantic_parity(&source_workload_reports, &prototypes)?;

    Ok(BenchmarkReport {
        generated_at: Utc::now(),
        database,
        dataset: config.dataset.clone(),
        source_load_ms,
        source_entity_rows: source.entity_rows,
        source_link_rows: source.link_rows,
        source_workloads: source_workload_reports,
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

async fn run_source_workloads(
    db: &DatabaseConnection,
    dataset: &DatasetConfig,
) -> Result<Vec<SourceWorkloadReport>> {
    let mut reports = Vec::new();
    for workload in source_workloads(dataset) {
        let digest = result_digest(db, workload.name, &workload.sql)
            .await
            .with_context(|| format!("failed to digest source workload {}", workload.name))?;
        reports.push(SourceWorkloadReport {
            name: workload.name,
            sql: workload.sql,
            result_rows: digest.rows,
            result_digest: digest.digest,
        });
    }
    Ok(reports)
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
    let digest = result_digest(db, workload.name, &workload.sql)
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

async fn result_digest(
    db: &DatabaseConnection,
    workload_name: &str,
    sql: &str,
) -> Result<ResultDigest> {
    let order_by = read_workload_contract(workload_name).digest_order_by;
    let ordered_json_sql = format!(
        "SELECT row_to_json(result)::text AS result_json FROM ({sql}) AS result ORDER BY {order_by}"
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, ordered_json_sql))
        .await
        .context("ordered workload digest query failed")?;
    let row_count = i64::try_from(rows.len()).context("workload result row count exceeds i64")?;
    let mut payload = String::new();
    for row in rows {
        let result_json: String = row
            .try_get("", "result_json")
            .context("ordered workload digest row did not contain result_json")?;
        write!(&mut payload, "{}:", result_json.len())?;
        payload.push_str(&result_json);
    }

    let digest_row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT md5($1::text) AS result_digest",
            vec![payload.into()],
        ))
        .await?
        .context("workload digest hash query returned no row")?;
    Ok(ResultDigest {
        rows: row_count,
        digest: digest_row.try_get("", "result_digest")?,
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
    let metrics = parse_read_explain_metrics(&plan)
        .context("EXPLAIN result did not satisfy the read evidence contract")?;

    Ok(ExplainEvidence {
        planning_time_ms: metrics.planning_time_ms,
        execution_time_ms: metrics.execution_time_ms,
        shared_hit_blocks: metrics.shared_hit_blocks,
        shared_read_blocks: metrics.shared_read_blocks,
        temporary_read_blocks: metrics.temporary_read_blocks,
        temporary_written_blocks: metrics.temporary_written_blocks,
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

fn validate_semantic_parity(
    source_workloads: &[SourceWorkloadReport],
    prototypes: &[PrototypeReport],
) -> Result<()> {
    ensure!(
        !source_workloads.is_empty(),
        "benchmark produced no source workload oracle"
    );
    ensure!(!prototypes.is_empty(), "benchmark produced no prototype reports");

    for candidate in prototypes {
        ensure!(
            candidate.workloads.len() == source_workloads.len(),
            "{} workload count differs from source oracle",
            candidate.schema
        );
        for expected in source_workloads {
            let actual = candidate
                .workloads
                .iter()
                .find(|workload| workload.name == expected.name)
                .with_context(|| {
                    format!("{} is missing workload {}", candidate.schema, expected.name)
                })?;
            ensure!(
                actual.result_rows == expected.result_rows,
                "{} workload {} row-count mismatch: source expected {}, got {}",
                candidate.schema,
                expected.name,
                expected.result_rows,
                actual.result_rows
            );
            ensure!(
                actual.result_digest == expected.result_digest,
                "{} workload {} result digest differs from source oracle",
                candidate.schema,
                expected.name
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
        assert_eq!(dataset.total_eav_field_rows(), 5_632);
        assert_eq!(dataset.total_link_rows(), 2_400);
    }
}
