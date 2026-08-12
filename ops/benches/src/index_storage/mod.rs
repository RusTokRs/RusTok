mod config;
mod connection;
mod database_metadata;
mod explain;
mod maintenance_runner;
mod mutation_runner;
mod partition_capture;
mod partition_cutover;
mod partition_maintenance;
mod partition_mutation;
mod partition_query;
mod partition_snapshot;
mod query_equivalence_admission;
mod query_equivalence_capture;
mod report_provenance;
mod runner;
mod sql;

pub use config::{BenchmarkConfig, BenchmarkRunProvenance, DatasetConfig, DatasetScale};
pub(crate) use connection::connect as connect_benchmark_database;
pub use database_metadata::DatabaseMetadata;
pub(crate) use database_metadata::{ensure_database_metadata_stable, read_database_metadata};
pub use maintenance_runner::{
    MaintenanceBenchmarkReport, run_maintenance, write_maintenance_report,
};
pub use mutation_runner::{MutationBenchmarkReport, run_mutations, write_mutation_report};
pub use partition_capture::{
    PartitionCaptureFinalize, PartitionCaptureFinalizeConfig, finalize_partition_capture,
};
pub use partition_cutover::{
    PartitionCutoverCapture, PartitionCutoverConfig, capture_partition_cutover_evidence,
};
pub use partition_maintenance::{
    PartitionMaintenanceCapture, PartitionMaintenanceConfig, capture_partition_maintenance_evidence,
};
pub use partition_mutation::{
    PartitionMutationCapture, PartitionMutationConfig, capture_partition_mutation_evidence,
};
pub use partition_query::{
    PartitionQueryCapture, PartitionQueryConfig, capture_partition_query_evidence,
};
pub use partition_snapshot::{
    BaselineSnapshot, PartitionSnapshotCapture, PartitionSnapshotConfig, RelationEvidence,
    ShadowRelationEvidence, ShadowSnapshot, TenantPredicateAudit, capture_partition_snapshot,
};
pub use query_equivalence_admission::{
    QueryEquivalenceAdmission, QueryEquivalenceAdmissionConfig, admit_query_equivalence_bundle,
};
pub use query_equivalence_capture::{
    QueryEquivalenceCapture, QueryEquivalenceCaptureConfig, capture_query_equivalence,
};
pub use report_provenance::write_provenance_bound_report;
pub use runner::{BenchmarkReport, run, write_report};
pub(crate) use sql::read_workload_contract;
pub use sql::{
    MutationWorkload, Prototype, RESULT_DIGEST_CONTRACT, Workload, analyze_sql, churn_cycle_sql,
    full_prototype_sql, mutation_workloads, prototype_sql, source_dataset_sql, source_workloads,
    vacuum_statements, workloads,
};

pub async fn run_from_env() -> anyhow::Result<BenchmarkReport> {
    let config = BenchmarkConfig::from_env()?;
    let report = run(&config).await?;
    write_provenance_bound_report(&config.output_path, &report, &config.run_provenance)?;
    Ok(report)
}
