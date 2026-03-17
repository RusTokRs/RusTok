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
fn module_has_migrations() {
    let module = CommerceModule;
    let migrations = module.migrations();
    assert!(!migrations.is_empty(), "CommerceModule must own its migrations");
    let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
    assert!(names.iter().any(|n| n.contains("create_commerce_products")));
}
