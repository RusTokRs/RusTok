use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde_json::Value;

mod config;
mod connection;
mod explain;
mod maintenance_runner;
mod mutation_runner;
mod runner;
mod sql;

pub use config::{BenchmarkConfig, DatasetConfig, DatasetScale};
pub(crate) use connection::{BENCHMARK_SESSION_METADATA, connect as connect_benchmark_database};
pub use maintenance_runner::{
    MaintenanceBenchmarkReport, run_maintenance, write_maintenance_report,
};
pub use mutation_runner::{MutationBenchmarkReport, run_mutations, write_mutation_report};
pub use runner::{BenchmarkReport, run, write_report};
pub(crate) use sql::read_workload_contract;
pub use sql::{
    MutationWorkload, Prototype, RESULT_DIGEST_CONTRACT, Workload, analyze_sql, churn_cycle_sql,
    full_prototype_sql, mutation_workloads, prototype_sql, source_dataset_sql, source_workloads,
    vacuum_statements, workloads,
};

fn write_report_with_session_metadata(path: &Path, report: &BenchmarkReport) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create benchmark output directory {parent:?}"))?;
    }

    let mut json = serde_json::to_value(report).context("failed to serialize benchmark report")?;
    let database = json
        .get_mut("database")
        .and_then(Value::as_object_mut)
        .context("benchmark report database metadata must be an object")?;
    for (field, setting_value) in BENCHMARK_SESSION_METADATA {
        database.insert(field.to_owned(), Value::String(setting_value.to_owned()));
    }

    let bytes = serde_json::to_vec_pretty(&json)
        .context("failed to serialize benchmark report with session metadata")?;
    fs::write(path, bytes)
        .with_context(|| format!("failed to write benchmark report to {path:?}"))?;
    Ok(())
}

pub async fn run_from_env() -> anyhow::Result<BenchmarkReport> {
    let config = BenchmarkConfig::from_env()?;
    let report = run(&config).await?;
    write_report_with_session_metadata(&config.output_path, &report)?;
    Ok(report)
}
