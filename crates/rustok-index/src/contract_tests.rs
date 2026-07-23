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

    use crate::{IndexModule, IndexerRuntimeConfig};

    let registry = ModuleRegistry::new().register(IndexModule);
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    let mut extensions = ModuleRuntimeExtensions::default();
    extensions.insert(IndexerRuntimeConfig::new(2, 100, 10));
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
fn index_module_registers_temporary_runtime_config() {
    use rustok_core::ModuleRegistry;

    use crate::{IndexModule, IndexerRuntimeConfig};

    let extensions = ModuleRegistry::new()
        .register(IndexModule)
        .build_runtime_extensions()
        .expect("index runtime extensions should initialize");

    assert!(extensions.contains::<IndexerRuntimeConfig>());
}
