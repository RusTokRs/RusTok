mod m20260727_000001_create_translation_inventory;
mod m20260728_000002_create_translation_workflow;
mod m20260728_000003_create_translation_apply_operations;
mod m20260728_000004_create_translation_workflow_controls;
mod m20260728_000005_create_translation_progress_and_retries;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260727_000001_create_translation_inventory::Migration),
        Box::new(m20260728_000002_create_translation_workflow::Migration),
        Box::new(m20260728_000003_create_translation_apply_operations::Migration),
        Box::new(m20260728_000004_create_translation_workflow_controls::Migration),
        Box::new(m20260728_000005_create_translation_progress_and_retries::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260727_000001_create_translation_inventory",
            vec!["m20250101_000001_create_tenants"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260728_000002_create_translation_workflow",
            vec!["m20260727_000001_create_translation_inventory"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260728_000003_create_translation_apply_operations",
            vec!["m20260728_000002_create_translation_workflow"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260728_000004_create_translation_workflow_controls",
            vec!["m20260728_000003_create_translation_apply_operations"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260728_000005_create_translation_progress_and_retries",
            vec!["m20260728_000004_create_translation_workflow_controls"],
        ),
    ]
}
