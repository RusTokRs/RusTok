use rustok_core::{MigrationSource, RusToKModule};
use rustok_rbac::RbacModule;

#[test]
fn module_metadata() {
    let module = RbacModule;
    assert_eq!(module.slug(), "rbac");
    assert_eq!(module.name(), "RBAC");
    assert_eq!(module.description(), "Role-based access control helpers.");
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_migrations_empty() {
    let module = RbacModule;
    assert!(module.migrations().is_empty());
}
