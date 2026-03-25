use rustok_core::{MigrationSource, RusToKModule};
use rustok_product::ProductModule;

#[test]
fn module_metadata() {
    let module = ProductModule;
    assert_eq!(module.slug(), "product");
    assert_eq!(module.name(), "Product");
    assert_eq!(
        module.description(),
        "Product catalog, variants, translations, options, and publication lifecycle"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert!(module.dependencies().is_empty());
}

#[test]
fn module_has_migrations() {
    let module = ProductModule;
    assert!(
        !module.migrations().is_empty(),
        "ProductModule must expose product schema migrations"
    );
}
