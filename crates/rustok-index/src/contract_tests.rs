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
async fn index_module_registers_registry_owned_event_listeners() {
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
    let mut names: Vec<&'static str> = handlers.iter().map(|handler| handler.name()).collect();
    names.sort_unstable();

    assert_eq!(
        names,
        vec!["content_indexer", "flex_indexer", "product_indexer"]
    );
}

#[test]
fn index_module_registers_default_runtime_config() {
    use rustok_core::ModuleRegistry;

    use crate::{IndexModule, IndexerRuntimeConfig};

    let extensions = ModuleRegistry::new()
        .register(IndexModule)
        .build_runtime_extensions()
        .expect("index runtime extensions should initialize");

    assert!(extensions.contains::<IndexerRuntimeConfig>());
}
