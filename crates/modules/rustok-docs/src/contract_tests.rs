use rustok_core::module::{ModuleKind, RusToKModule};

use crate::DocsModule;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
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

#[test]
fn module_contract_metadata_matches() {
    let module = DocsModule::new();
    assert_eq!(module.slug(), "docs");
    assert_eq!(module.name(), "Collaborative Documents & Wiki");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["outbox", "flex"]);
}

#[test]
fn fba_registry_contract_matches() {
    let raw = include_str!("../contracts/docs-fba-registry.json");
    let val: serde_json::Value = serde_json::from_str(raw).expect("valid JSON in docs-fba-registry.json");
    assert_eq!(val["module"], "docs");
    assert_eq!(val["role"], "provider");
    assert_eq!(val["crdt_engine"]["name"], "Loro");

    let operations = val["services"][0]["operations"]
        .as_array()
        .expect("operations array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();

    assert!(operations.contains(&"create_page"));
    assert!(operations.contains(&"get_page"));
    assert!(operations.contains(&"get_page_by_slug"));
    assert!(operations.contains(&"list_pages"));
    assert!(operations.contains(&"get_page_tree"));
    assert!(operations.contains(&"update_page"));
    assert!(operations.contains(&"delete_page"));
    assert!(operations.contains(&"load_document"));
    assert!(operations.contains(&"append_updates"));
    assert!(operations.contains(&"compact_snapshot"));
    assert!(operations.contains(&"get_sync_payload"));
    assert!(operations.contains(&"sync_entity_links"));
    assert!(operations.contains(&"render_markdown"));
}

#[test]
fn uer_protocol_urn_formatting() {
    use uuid::Uuid;
    use crate::domain::{EntityDisplayMode, EntityRef};

    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let eref = EntityRef {
        tenant_id,
        module: "commerce".to_string(),
        entity_type: "product".to_string(),
        entity_id: product_id,
        display_mode: EntityDisplayMode::CompactCard,
    };

    let urn = eref.to_urn();
    assert_eq!(
        urn,
        format!("urn:rustok:{tenant_id}:commerce:product:{product_id}")
    );
}

#[test]
fn compaction_threshold_constant() {
    use crate::services::COMPACTION_THRESHOLD;
    assert_eq!(COMPACTION_THRESHOLD, 50);
}

#[test]
fn runtime_extensions_registration_succeeds() {
    let module = DocsModule::new();
    let mut extensions = rustok_core::ModuleRuntimeExtensions::default();
    assert!(module.register_runtime_extensions(&mut extensions).is_ok());
}
