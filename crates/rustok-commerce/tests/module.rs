use rustok_commerce::CommerceModule;
use rustok_core::{MigrationSource, RusToKModule};

#[test]
fn module_metadata() {
    let module = CommerceModule;
    assert_eq!(module.slug(), "commerce");
    assert_eq!(module.name(), "Ecommerce");
    assert_eq!(
        module.description(),
        "Ecommerce umbrella/root module for the commerce family, compatibility facade, and orchestration surface"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(module.dependencies(), ["product", "pricing", "inventory"]);
}

#[test]
fn module_exposes_facade_migrations() {
    let module = CommerceModule;
    let migrations = module.migrations();
    assert!(
        !migrations.is_empty(),
        "CommerceModule must expose umbrella-owned migrations"
    );
}
