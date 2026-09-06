use rustok_core::{MigrationSource, RusToKModule};
use rustok_inventory::InventoryModule;

#[test]
fn module_metadata() {
    let module = InventoryModule;
    assert_eq!(module.slug(), "inventory");
    assert_eq!(module.name(), "Inventory");
    assert_eq!(
        module.description(),
        "Inventory adjustments, availability checks, and stock-level persistence"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(module.dependencies(), ["product"]);
}

#[test]
fn module_has_migrations() {
    let module = InventoryModule;
    assert!(
        !module.migrations().is_empty(),
        "InventoryModule must expose inventory schema migrations"
    );
}
