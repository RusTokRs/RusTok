use rustok_cart::CartModule;
use rustok_core::{MigrationSource, RusToKModule};

#[test]
fn module_metadata() {
    let module = CartModule;
    assert_eq!(module.slug(), "cart");
    assert_eq!(module.name(), "Cart");
    assert_eq!(
        module.description(),
        "Default cart submodule in the ecommerce family"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert!(module.dependencies().is_empty());
}

#[test]
fn module_has_migrations() {
    let module = CartModule;
    assert!(
        !module.migrations().is_empty(),
        "CartModule must expose cart schema migrations"
    );
}
