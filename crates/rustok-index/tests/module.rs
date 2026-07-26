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
fn module_has_no_legacy_migrations_during_storage_rewrite() {
    let module = IndexModule;
    assert!(
        module.migrations().is_empty(),
        "IndexModule production persistence remains absent until the storage ADR is accepted"
    );
}
