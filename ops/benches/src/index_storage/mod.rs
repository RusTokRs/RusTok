mod config;
mod connection;
mod maintenance_runner;
mod mutation_runner;
mod runner;
mod sql;

pub(crate) use connection::connect as connect_benchmark_database;
pub use config::{BenchmarkConfig, DatasetConfig, DatasetScale};
pub use maintenance_runner::{
    MaintenanceBenchmarkReport, run_maintenance, write_maintenance_report,
};
pub use mutation_runner::{
    MutationBenchmarkReport, run_mutations, write_mutation_report,
};
pub use runner::{BenchmarkReport, run, write_report};
pub use sql::{
    MutationWorkload, Prototype, Workload, analyze_sql, churn_cycle_sql, full_prototype_sql,
    mutation_workloads, prototype_sql, source_dataset_sql, vacuum_statements, workloads,
};

pub async fn run_from_env() -> anyhow::Result<BenchmarkReport> {
    let config = BenchmarkConfig::from_env()?;
    let report = run(&config).await?;
    write_report(&config.output_path, &report)?;
    Ok(report)
}
