use rustok_core::{MigrationSource, RusToKModule};
use rustok_profiles::ProfilesModule;

#[test]
fn module_metadata() {
    let module = ProfilesModule;
    assert_eq!(module.slug(), "profiles");
    assert_eq!(module.name(), "Profiles");
    assert_eq!(
        module.description(),
        "Universal public profile domain for platform users"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert!(module.dependencies().is_empty());
}

#[test]
fn module_has_migrations() {
    let module = ProfilesModule;
    assert!(
        !module.migrations().is_empty(),
        "ProfilesModule must expose profile schema migrations"
    );
}
