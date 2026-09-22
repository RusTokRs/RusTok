use rustok_core::{MigrationSource, RusToKModule};
use rustok_search::SearchModule;

#[test]
fn module_metadata() {
    let module = SearchModule;
    assert_eq!(module.slug(), "search");
    assert_eq!(module.name(), "Search");
    assert_eq!(
        module.description(),
        "Postgres-first search capability with settings-driven engine selection."
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_has_migrations() {
    let module = SearchModule;
    let migrations = module.migrations();
    assert!(
        !migrations.is_empty(),
        "SearchModule must own its migrations"
    );
    let names: Vec<_> = migrations
        .iter()
        .map(|migration| migration.name())
        .collect();
    assert!(
        names
            .iter()
            .any(|name| name.contains("create_search_settings")),
        "SearchModule migrations should include search settings storage"
    );
    assert!(
        names
            .iter()
            .any(|name| name.contains("create_search_documents")),
        "SearchModule migrations should include search document storage"
    );
}
