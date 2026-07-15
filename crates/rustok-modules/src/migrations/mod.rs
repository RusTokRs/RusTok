mod m20260711_000001_module_artifact_installations;
mod m20260713_000002_module_artifact_admissions;
mod m20260713_000003_artifact_installation_rollback_pointer;
mod m20260713_000004_artifact_capability_grant_revision;
mod m20260713_000005_module_artifact_rollback_operations;
mod m20260715_000006_module_artifact_uninstall_operations;

use sea_orm_migration::prelude::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260711_000001_module_artifact_installations::Migration),
        Box::new(m20260713_000002_module_artifact_admissions::Migration),
        Box::new(m20260713_000003_artifact_installation_rollback_pointer::Migration),
        Box::new(m20260713_000004_artifact_capability_grant_revision::Migration),
        Box::new(m20260713_000005_module_artifact_rollback_operations::Migration),
        Box::new(m20260715_000006_module_artifact_uninstall_operations::Migration),
    ]
}
