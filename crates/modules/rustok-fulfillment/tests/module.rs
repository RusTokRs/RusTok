use rustok_core::{MigrationSource, RusToKModule};
use rustok_fulfillment::FulfillmentModule;

#[test]
fn module_metadata() {
    let module = FulfillmentModule;
    assert_eq!(module.slug(), "fulfillment");
    assert_eq!(module.name(), "Fulfillment");
    assert_eq!(
        module.description(),
        "Default fulfillment submodule in the ecommerce family"
    );
}

#[test]
fn module_has_migrations() {
    let module = FulfillmentModule;
    assert!(
        !module.migrations().is_empty(),
        "fulfillment module should expose migrations"
    );
}
