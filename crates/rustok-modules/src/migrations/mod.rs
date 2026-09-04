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
pub(crate) mod m20260716_000012_module_build_requests;
mod m20260716_000013_artifact_admission_commands;
mod m20260716_000014_artifact_secret_bindings;
mod m20260716_000015_artifact_execution_audit;
mod m20260716_000016_artifact_execution_audit_metrics;
mod m20260717_000002_create_registry_publish_build_staging;
mod m20260717_000018_artifact_event_deliveries;
mod m20260717_000019_artifact_schedule_deliveries;
mod m20260717_000020_artifact_schedule_cursors;
mod m20260717_000021_artifact_sandbox_policies;
mod m20260717_000023_artifact_binding_operations;
mod m20260718_000025_artifact_data_objects;
mod m20260718_000026_artifact_data_object_operations;
mod m20260718_000027_artifact_data_object_gc_candidates;
mod m20260718_000028_artifact_data_object_upload_sessions;
mod m20260718_000029_artifact_data_indexes;
mod m20260718_000030_artifact_data_index_contracts;
mod m20260718_000031_artifact_data_exports;
mod m20260720_000032_artifact_binding_operation_rls;
mod m20260722_000033_artifact_data_snapshots;
mod m20260722_000034_static_promotions;
pub(crate) mod m20260722_000035_static_distribution_rollouts;
mod m20260722_000036_artifact_security_state;
mod m20260722_000037_policy_revision_cursors;
mod m20260726_000038_artifact_data_object_deletions;
mod m20260726_000039_artifact_data_record_deletions;
mod m20260727_000040_registry_platform_admission_contracts;
mod m20260727_000041_registry_release_artifact_contracts;
pub(crate) mod m20260814_000042_artifact_node_reconciliation;
mod m20260822_000043_artifact_ui_contribution_locks;
pub(crate) mod m20260822_000044_module_build_execution_claims;
pub(crate) mod m20260901_000045_artifact_admission_reverification_operations;
pub(crate) mod m20260902_000046_module_transition_and_retention_tables;
pub(crate) mod m20260903_000047_artifact_data_copy_operations;
pub(crate) mod m20260903_000048_artifact_data_object_copy_operations;
pub(crate) mod m20260903_000049_artifact_data_snapshot_and_recovery_operations;
pub(crate) mod m20260904_000050_rhai_authoring_packages;
pub(crate) mod m20260904_000051_static_localized_settings;
pub(crate) mod m20260904_000052_static_settings_change_cursor;
pub(crate) mod m20260904_000053_static_settings_source_locale;

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
        Box::new(m20260717_000002_create_registry_publish_build_staging::Migration),
        Box::new(m20260717_000018_artifact_event_deliveries::Migration),
        Box::new(m20260717_000019_artifact_schedule_deliveries::Migration),
        Box::new(m20260717_000020_artifact_schedule_cursors::Migration),
        Box::new(m20260717_000021_artifact_sandbox_policies::Migration),
        Box::new(m20260717_000023_artifact_binding_operations::Migration),
        Box::new(m20260718_000025_artifact_data_objects::Migration),
        Box::new(m20260718_000026_artifact_data_object_operations::Migration),
        Box::new(m20260718_000027_artifact_data_object_gc_candidates::Migration),
        Box::new(m20260718_000028_artifact_data_object_upload_sessions::Migration),
        Box::new(m20260718_000029_artifact_data_indexes::Migration),
        Box::new(m20260718_000030_artifact_data_index_contracts::Migration),
        Box::new(m20260718_000031_artifact_data_exports::Migration),
        Box::new(m20260720_000032_artifact_binding_operation_rls::Migration),
        Box::new(m20260722_000033_artifact_data_snapshots::Migration),
        Box::new(m20260722_000034_static_promotions::Migration),
        Box::new(m20260722_000035_static_distribution_rollouts::Migration),
        Box::new(m20260722_000036_artifact_security_state::Migration),
        Box::new(m20260722_000037_policy_revision_cursors::Migration),
        Box::new(m20260726_000038_artifact_data_object_deletions::Migration),
        Box::new(m20260726_000039_artifact_data_record_deletions::Migration),
        Box::new(m20260727_000040_registry_platform_admission_contracts::Migration),
        Box::new(m20260727_000041_registry_release_artifact_contracts::Migration),
        Box::new(m20260814_000042_artifact_node_reconciliation::Migration),
        Box::new(m20260822_000043_artifact_ui_contribution_locks::Migration),
        Box::new(m20260822_000044_module_build_execution_claims::Migration),
        Box::new(m20260901_000045_artifact_admission_reverification_operations::Migration),
        Box::new(m20260902_000046_module_transition_and_retention_tables::Migration),
        Box::new(m20260903_000047_artifact_data_copy_operations::Migration),
        Box::new(m20260903_000048_artifact_data_object_copy_operations::Migration),
        Box::new(m20260903_000049_artifact_data_snapshot_and_recovery_operations::Migration),
        Box::new(m20260904_000050_rhai_authoring_packages::Migration),
        Box::new(m20260904_000051_static_localized_settings::Migration),
        Box::new(m20260904_000052_static_settings_change_cursor::Migration),
        Box::new(m20260904_000053_static_settings_source_locale::Migration),
    ]
}
