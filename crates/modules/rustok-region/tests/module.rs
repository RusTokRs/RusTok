use rustok_core::{MigrationSource, RusToKModule};
use rustok_region::RegionModule;

#[test]
fn module_metadata() {
    let module = RegionModule;
    assert_eq!(module.slug(), "region");
    assert_eq!(module.name(), "Region");
    assert_eq!(
        module.description(),
        "Default region submodule in the ecommerce family"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert!(module.dependencies().is_empty());
}

#[test]
fn module_has_migrations() {
    let module = RegionModule;
    assert!(
        !module.migrations().is_empty(),
        "RegionModule must expose region schema migrations"
    );
}
