use rustok_core::{MigrationSource, RusToKModule};
use rustok_customer::CustomerModule;

#[test]
fn module_metadata() {
    let module = CustomerModule;
    assert_eq!(module.slug(), "customer");
    assert_eq!(module.name(), "Customer");
    assert_eq!(
        module.description(),
        "Default storefront customer submodule in the ecommerce family"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert!(module.dependencies().is_empty());
}

#[test]
fn module_has_migrations() {
    let module = CustomerModule;
    assert!(
        !module.migrations().is_empty(),
        "CustomerModule must expose customer schema migrations"
    );
}
