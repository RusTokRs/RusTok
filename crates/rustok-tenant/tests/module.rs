use rustok_core::{MigrationSource, RusToKModule};
use rustok_tenant::TenantModule;

#[test]
fn module_metadata() {
    let module = TenantModule;
    assert_eq!(module.slug(), "tenant");
    assert_eq!(module.name(), "Tenant");
    assert_eq!(
        module.description(),
        "Multi-tenancy helpers and tenant metadata."
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_migrations_empty() {
    let module = TenantModule;
    assert!(module.migrations().is_empty());
}
