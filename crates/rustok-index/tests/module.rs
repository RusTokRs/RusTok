use rustok_core::{MigrationSource, RusToKModule};
use rustok_index::IndexModule;

#[test]
fn module_metadata() {
    let module = IndexModule;
    assert_eq!(module.slug(), "index");
    assert_eq!(module.name(), "Index");
    assert_eq!(
        module.description(),
        "Cross-module relational index and query engine."
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_registers_canonical_storage_migrations() {
    let module = IndexModule;
    let migrations = module.migrations();
    let names = migrations
        .iter()
        .map(|migration| migration.name())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "m20260727_000001_create_index_records",
            "m20260727_000002_create_index_delivery_state",
            "m20260727_000003_create_index_operations",
            "m20260803_000004_create_index_reconciliation_recovery",
            "m20260804_000005_relax_index_finding_locale_scope",
            "m20260806_000006_add_index_finding_lifecycle_audit",
            "m20260806_000007_add_index_finding_repair_commands",
            "m20260806_000008_add_index_finding_repair_recovery",
            "m20260808_000009_add_index_job_locale_scope",
        ]
    );

    let dependencies = module.migration_dependencies();
    assert_eq!(dependencies.len(), 9);
    assert_eq!(
        dependencies[0].migration,
        "m20260727_000001_create_index_records"
    );
    assert_eq!(
        dependencies[1].migration,
        "m20260727_000002_create_index_delivery_state"
    );
    assert_eq!(
        dependencies[2].migration,
        "m20260727_000003_create_index_operations"
    );
    assert_eq!(
        dependencies[3].migration,
        "m20260803_000004_create_index_reconciliation_recovery"
    );
    assert_eq!(
        dependencies[4].migration,
        "m20260804_000005_relax_index_finding_locale_scope"
    );
    assert_eq!(
        dependencies[5].migration,
        "m20260806_000006_add_index_finding_lifecycle_audit"
    );
    assert_eq!(
        dependencies[6].migration,
        "m20260806_000007_add_index_finding_repair_commands"
    );
    assert_eq!(
        dependencies[7].migration,
        "m20260806_000008_add_index_finding_repair_recovery"
    );
    assert_eq!(
        dependencies[8].migration,
        "m20260808_000009_add_index_job_locale_scope"
    );
}
