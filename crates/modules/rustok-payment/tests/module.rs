use rustok_core::{MigrationSource, RusToKModule};
use rustok_payment::PaymentModule;

#[test]
fn module_metadata() {
    let module = PaymentModule;
    assert_eq!(module.slug(), "payment");
    assert_eq!(module.name(), "Payment");
    assert_eq!(
        module.description(),
        "Default payment submodule in the ecommerce family"
    );
}

#[test]
fn module_has_migrations() {
    let module = PaymentModule;
    assert!(
        !module.migrations().is_empty(),
        "payment module should expose migrations"
    );
}
