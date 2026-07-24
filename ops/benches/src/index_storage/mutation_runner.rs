use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use serde::Serialize;
use serde_json::Value;

use super::{
    BenchmarkConfig, MutationWorkload, Prototype, connect_benchmark_database,
    explain::parse_mutation_explain_metrics, full_prototype_sql, mutation_workloads,
    source_dataset_sql,
};

#[derive(Debug, Serialize)]
pub struct MutationBenchmarkReport {
    pub generated_at: DateTime<Utc>,
    pub dataset_scale: String,
    pub repetitions: u32,
    pub prototypes: Vec<PrototypeMutationReport>,
}

#[derive(Debug, Serialize)]
pub struct PrototypeMutationReport {
    pub prototype: Prototype,
    pub schema: &'static str,
    pub workloads: Vec<MutationWorkloadReport>,
}

#[derive(Debug, Serialize)]
pub struct MutationWorkloadReport {
    pub name: &'static str,
    pub sql: String,
    pub affected_entities: i64,
    pub affected_fields: Option<i64>,
    pub affected_links: Option<i64>,
    pub repetitions: Vec<MutationExplainEvidence>,
}

#[derive(Debug, Serialize)]
pub struct MutationExplainEvidence {
    pub planning_time_ms: f64,
    pub execution_time_ms: f64,
    pub shared_hit_blocks: u64,
    pub shared_read_blocks: u64,
    pub temporary_read_blocks: Option<u64>,
    pub temporary_written_blocks: Option<u64>,
    pub maximum_node_wal_records: u64,
    pub maximum_node_wal_fpi: u64,
    pub maximum_node_wal_bytes: u64,
    pub plan: Value,
}

#[derive(Debug)]
struct MutationValidation {
    affected_entities: i64,
    affected_fields: Option<i64>,
    affected_links: Option<i64>,
}

pub async fn run_mutations(config: &BenchmarkConfig) -> Result<MutationBenchmarkReport> {
    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared("SET jit = off; SET statement_timeout = '30min';")
        .await
        .context("failed to configure mutation benchmark session")?;
    db.execute_unprepared(&source_dataset_sql(&config.dataset))
        .await
        .context("failed to create mutation benchmark source dataset")?;

    let mut prototypes = Vec::with_capacity(Prototype::ALL.len());
    for prototype in Prototype::ALL {
        db.execute_unprepared(&full_prototype_sql(prototype))
            .await
            .with_context(|| format!("failed to prepare {:?} mutation prototype", prototype))?;
        let mut reports = Vec::new();
        for workload in mutation_workloads(prototype, &config.dataset) {
            reports.push(run_mutation_workload(&db, workload, config.repetitions).await?);
        }
        prototypes.push(PrototypeMutationReport {
            prototype,
            schema: prototype.schema(),
            workloads: reports,
        });
    }
    validate_mutation_shape(&prototypes)?;

    Ok(MutationBenchmarkReport {
        generated_at: Utc::now(),
        dataset_scale: format!("{:?}", config.dataset.scale),
        repetitions: config.repetitions,
        prototypes,
    })
}

pub fn write_mutation_report(path: &Path, report: &MutationBenchmarkReport) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create mutation report directory {parent:?}"))?;
    }
    fs::write(path, serde_json::to_vec_pretty(report)?)
        .with_context(|| format!("failed to write mutation report to {path:?}"))?;
    Ok(())
}

async fn run_mutation_workload(
    db: &DatabaseConnection,
    workload: MutationWorkload,
    repetitions: u32,
) -> Result<MutationWorkloadReport> {
    let validation = validate_mutation(db, &workload).await?;
    let mut evidence = Vec::with_capacity(repetitions as usize);
    for _ in 0..repetitions {
        let transaction = db.begin().await?;
        let result = explain_mutation(&transaction, &workload.sql).await;
        transaction.rollback().await?;
        evidence.push(result.with_context(|| {
            format!("failed to explain mutation workload {}", workload.name)
        })?);
    }

    Ok(MutationWorkloadReport {
        name: workload.name,
        sql: workload.sql,
        affected_entities: validation.affected_entities,
        affected_fields: validation.affected_fields,
        affected_links: validation.affected_links,
        repetitions: evidence,
    })
}

async fn validate_mutation(
    db: &DatabaseConnection,
    workload: &MutationWorkload,
) -> Result<MutationValidation> {
    let transaction = db.begin().await?;
    let result = validate_mutation_in_transaction(&transaction, workload).await;
    transaction.rollback().await?;
    result
}

async fn validate_mutation_in_transaction(
    transaction: &DatabaseTransaction,
    workload: &MutationWorkload,
) -> Result<MutationValidation> {
    let row = transaction
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            workload.sql.clone(),
        ))
        .await?
        .context("mutation validation query returned no row")?;
    let affected_entities: i64 = row.try_get("", "affected_entities")?;
    ensure!(
        affected_entities == workload.expected_affected_entities,
        "mutation {} affected {} entities, expected {}",
        workload.name,
        affected_entities,
        workload.expected_affected_entities
    );

    let affected_fields = validate_optional_effect(
        workload.name,
        "fields",
        row.try_get("", "affected_fields")?,
        row.try_get("", "expected_fields")?,
    )?;
    let affected_links = validate_optional_effect(
        workload.name,
        "links",
        row.try_get("", "affected_links")?,
        row.try_get("", "expected_links")?,
    )?;

    Ok(MutationValidation {
        affected_entities,
        affected_fields,
        affected_links,
    })
}

fn validate_optional_effect(
    workload: &str,
    effect: &str,
    actual: Option<i64>,
    expected: Option<i64>,
) -> Result<Option<i64>> {
    ensure!(
        actual.is_some() == expected.is_some(),
        "mutation {workload} returned inconsistent optional {effect} effect columns"
    );
    if let (Some(actual), Some(expected)) = (actual, expected) {
        ensure!(
            actual == expected,
            "mutation {workload} affected {actual} {effect}, expected {expected}"
        );
        Ok(Some(actual))
    } else {
        Ok(None)
    }
}

async fn explain_mutation(
    transaction: &DatabaseTransaction,
    sql: &str,
) -> Result<MutationExplainEvidence> {
    let row = transaction
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!("EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON) {sql}"),
        ))
        .await?
        .context("mutation EXPLAIN returned no row")?;
    let plan: Value = row
        .try_get("", "QUERY PLAN")
        .context("mutation EXPLAIN result did not contain QUERY PLAN JSON")?;
    let metrics = parse_mutation_explain_metrics(&plan)
        .context("mutation EXPLAIN result did not satisfy the evidence contract")?;

    Ok(MutationExplainEvidence {
        planning_time_ms: metrics.planning_time_ms,
        execution_time_ms: metrics.execution_time_ms,
        shared_hit_blocks: metrics.shared_hit_blocks,
        shared_read_blocks: metrics.shared_read_blocks,
        temporary_read_blocks: metrics.temporary_read_blocks,
        temporary_written_blocks: metrics.temporary_written_blocks,
        maximum_node_wal_records: metrics.maximum_node_wal_records,
        maximum_node_wal_fpi: metrics.maximum_node_wal_fpi,
        maximum_node_wal_bytes: metrics.maximum_node_wal_bytes,
        plan,
    })
}

fn validate_mutation_shape(prototypes: &[PrototypeMutationReport]) -> Result<()> {
    let baseline = prototypes
        .first()
        .context("mutation benchmark produced no prototypes")?;
    for candidate in &prototypes[1..] {
        ensure!(
            candidate.workloads.len() == baseline.workloads.len(),
            "{} mutation workload count differs from {}",
            candidate.schema,
            baseline.schema
        );
        for expected in &baseline.workloads {
            let actual = candidate
                .workloads
                .iter()
                .find(|workload| workload.name == expected.name)
                .with_context(|| {
                    format!("{} is missing mutation {}", candidate.schema, expected.name)
                })?;
            ensure!(
                actual.affected_entities == expected.affected_entities,
                "{} mutation {} entity count differs from {}",
                candidate.schema,
                expected.name,
                baseline.schema
            );
            ensure!(
                actual.affected_links == expected.affected_links,
                "{} mutation {} link count differs from {}",
                candidate.schema,
                expected.name,
                baseline.schema
            );
        }
    }
    Ok(())
}
