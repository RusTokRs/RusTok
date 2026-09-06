use rustok_core::{MigrationSource, RusToKModule};
use rustok_pricing::PricingModule;

#[test]
fn module_metadata() {
    let module = PricingModule;
    assert_eq!(module.slug(), "pricing");
    assert_eq!(module.name(), "Pricing");
    assert_eq!(
        module.description(),
        "Variant pricing, price lists, regions, and discount calculations"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(module.dependencies(), ["product"]);
}

#[test]
fn module_has_migrations() {
    let module = PricingModule;
    assert!(
        !module.migrations().is_empty(),
        "PricingModule must expose pricing schema migrations"
    );
}
