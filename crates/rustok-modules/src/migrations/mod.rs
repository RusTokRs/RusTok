mod m20260711_000001_module_artifact_installations;
mod m20260713_000002_module_artifact_admissions;
mod m20260713_000003_artifact_installation_rollback_pointer;
mod m20260713_000004_artifact_capability_grant_revision;
mod m20260713_000005_module_artifact_rollback_operations;
mod m20260715_000006_module_artifact_uninstall_operations;
mod m20260716_000007_artifact_migration_checkpoints;
mod m20260716_000008_module_artifact_deactivation_operations;
mod m20260716_000009_artifact_tenant_lifecycle;
mod m20260716_000010_artifact_data_broker;
mod m20260716_000011_artifact_data_namespace_lifecycle;
mod m20260716_000012_module_build_requests;
mod m20260716_000013_artifact_admission_commands;
mod m20260716_000014_artifact_secret_bindings;
mod m20260716_000015_artifact_execution_audit;
mod m20260716_000016_artifact_execution_audit_metrics;

use sea_orm_migration::prelude::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260711_000001_module_artifact_installations::Migration),
        Box::new(m20260713_000002_module_artifact_admissions::Migration),
        Box::new(m20260713_000003_artifact_installation_rollback_pointer::Migration),
        Box::new(m20260713_000004_artifact_capability_grant_revision::Migration),
        Box::new(m20260713_000005_module_artifact_rollback_operations::Migration),
        Box::new(m20260715_000006_module_artifact_uninstall_operations::Migration),
        Box::new(m20260716_000007_artifact_migration_checkpoints::Migration),
        Box::new(m20260716_000008_module_artifact_deactivation_operations::Migration),
        Box::new(m20260716_000009_artifact_tenant_lifecycle::Migration),
        Box::new(m20260716_000010_artifact_data_broker::Migration),
        Box::new(m20260716_000011_artifact_data_namespace_lifecycle::Migration),
        Box::new(m20260716_000012_module_build_requests::Migration),
        Box::new(m20260716_000013_artifact_admission_commands::Migration),
        Box::new(m20260716_000014_artifact_secret_bindings::Migration),
        Box::new(m20260716_000015_artifact_execution_audit::Migration),
        Box::new(m20260716_000016_artifact_execution_audit_metrics::Migration),
    ]
}
