use rustok_core::{MigrationSource, RusToKModule};
use rustok_index::IndexModule;

#[test]
fn module_metadata() {
    let module = IndexModule;
    assert_eq!(module.slug(), "index");
    assert_eq!(module.name(), "Index");
    assert_eq!(module.description(), "CQRS Read Model (Fast Search)");
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_migrations_empty() {
    let module = IndexModule;
    assert!(module.migrations().is_empty());
}
