#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[tokio::test]
async fn index_module_registers_no_legacy_event_listeners() {
    use rustok_core::{ModuleEventListenerContext, ModuleRegistry, ModuleRuntimeExtensions};
    use sea_orm::Database;

    use crate::IndexModule;

    let registry = ModuleRegistry::new().register(IndexModule);
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    let extensions = ModuleRuntimeExtensions::default();
    let ctx = ModuleEventListenerContext {
        db,
        extensions: &extensions,
    };

    let handlers = registry.build_event_listeners(&ctx);
    assert!(
        handlers.is_empty(),
        "legacy Content/Product/Flex listeners must not return"
    );
}

#[test]
fn index_module_has_no_legacy_migrations() {
    use rustok_core::MigrationSource;

    use crate::IndexModule;

    assert!(IndexModule.migrations().is_empty());
}
