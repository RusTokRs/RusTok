use rustok_core::{MigrationSource, RusToKModule};
use rustok_order::OrderModule;

#[test]
fn module_metadata() {
    let module = OrderModule;
    assert_eq!(module.slug(), "order");
    assert_eq!(module.name(), "Order");
    assert_eq!(
        module.description(),
        "Default order submodule in the ecommerce family"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert!(module.dependencies().is_empty());
}

#[test]
fn module_has_migrations() {
    let module = OrderModule;
    assert!(
        !module.migrations().is_empty(),
        "OrderModule must expose order schema migrations"
    );
}
