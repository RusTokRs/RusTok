use rustok_commerce::CommerceModule;
use rustok_core::{MigrationSource, RusToKModule};

#[test]
fn module_metadata() {
    let module = CommerceModule;
    assert_eq!(module.slug(), "commerce");
    assert_eq!(module.name(), "Commerce");
    assert_eq!(module.description(), "Products, Orders, Cart, Checkout");
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_migrations_empty() {
    let module = CommerceModule;
    assert!(module.migrations().is_empty());
}
