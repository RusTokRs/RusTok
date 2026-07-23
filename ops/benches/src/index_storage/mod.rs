mod config;
mod mutation_runner;
mod runner;
mod sql;

pub use config::{BenchmarkConfig, DatasetConfig, DatasetScale};
pub use mutation_runner::{
    MutationBenchmarkReport, run_mutations, write_mutation_report,
};
pub use runner::{BenchmarkReport, run, write_report};
pub use sql::{
    MutationWorkload, Prototype, Workload, full_prototype_sql, mutation_workloads, prototype_sql,
    source_dataset_sql, workloads,
};

pub async fn run_from_env() -> anyhow::Result<BenchmarkReport> {
    let config = BenchmarkConfig::from_env()?;
    let report = run(&config).await?;
    write_report(&config.output_path, &report)?;
    Ok(report)
}
