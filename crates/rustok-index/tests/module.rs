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
        ]
    );

    let dependencies = module.migration_dependencies();
    assert_eq!(dependencies.len(), 3);
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
}
