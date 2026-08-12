mod m20260727_000001_create_index_records;
mod m20260727_000002_create_index_delivery_state;
mod m20260727_000003_create_index_operations;
mod m20260803_000004_create_index_reconciliation_recovery;
mod m20260804_000005_relax_index_finding_locale_scope;
mod m20260806_000006_add_index_finding_lifecycle_audit;
mod m20260806_000007_add_index_finding_repair_commands;
mod m20260806_000008_add_index_finding_repair_recovery;
mod m20260808_000009_add_index_job_locale_scope;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DbBackend;

pub(super) fn source_version_column<T: IntoIden>(
    backend: DbBackend,
    name: T,
    default_zero: bool,
) -> ColumnDef {
    let mut column = ColumnDef::new(name);
    match backend {
        DbBackend::Sqlite => {
            column.big_integer();
        }
        DbBackend::Postgres | DbBackend::MySql => {
            column.decimal_len(20, 0);
        }
    }
    column.not_null();
    if default_zero {
        column.default(0);
    }
    column
}

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260727_000001_create_index_records::Migration),
        Box::new(m20260727_000002_create_index_delivery_state::Migration),
        Box::new(m20260727_000003_create_index_operations::Migration),
        Box::new(m20260803_000004_create_index_reconciliation_recovery::Migration),
        Box::new(m20260804_000005_relax_index_finding_locale_scope::Migration),
        Box::new(m20260806_000006_add_index_finding_lifecycle_audit::Migration),
        Box::new(m20260806_000007_add_index_finding_repair_commands::Migration),
        Box::new(m20260806_000008_add_index_finding_repair_recovery::Migration),
        Box::new(m20260808_000009_add_index_job_locale_scope::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260727_000001_create_index_records",
            vec!["m20250101_000001_create_tenants"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260727_000002_create_index_delivery_state",
            vec!["m20260727_000001_create_index_records"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260727_000003_create_index_operations",
            vec!["m20260727_000001_create_index_records"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260803_000004_create_index_reconciliation_recovery",
            vec!["m20260727_000003_create_index_operations"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260804_000005_relax_index_finding_locale_scope",
            vec!["m20260803_000004_create_index_reconciliation_recovery"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260806_000006_add_index_finding_lifecycle_audit",
            vec!["m20260804_000005_relax_index_finding_locale_scope"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260806_000007_add_index_finding_repair_commands",
            vec!["m20260806_000006_add_index_finding_lifecycle_audit"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260806_000008_add_index_finding_repair_recovery",
            vec!["m20260806_000007_add_index_finding_repair_commands"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260808_000009_add_index_job_locale_scope",
            vec!["m20260806_000008_add_index_finding_repair_recovery"],
        ),
    ]
}
