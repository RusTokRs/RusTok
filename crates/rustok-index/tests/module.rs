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
fn module_has_migrations() {
    let module = IndexModule;
    let migrations = module.migrations();
    assert!(!migrations.is_empty(), "IndexModule must own its migrations");
    let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
    assert!(names.iter().any(|n| n.contains("create_index_content")));
}
