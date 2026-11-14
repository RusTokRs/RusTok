//! Contract tests verifying the architectural invariants and public API of `rustok-pages-admin`.

use crate::access::pages_editor_permissions_from_effective;
use crate::browser_problem::{
    BROWSER_DOCUMENT_UNAVAILABLE_CODE, BROWSER_DRAFT_GENERATION_CONFLICT_CODE,
    BROWSER_PROJECT_HASH_CONFLICT_CODE, BROWSER_REVISION_CONFLICT_CODE,
};
use crate::core::{
    SiteSymbolUsageInfo, SymbolValidationError, TemplateValidationError, compute_symbol_usage,
    validate_symbol_id, validate_template_key,
};
use crate::model::PageTemplateSummary;
use fly_ui::CapabilityState;

const CRATE_API_DOC: &str = include_str!("../CRATE_API.md");

#[test]
fn crate_api_markdown_contains_all_mandatory_contract_sections() {
    let required_sections = [
        "## Minimum Contract Set",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Public surface",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ];

    for section in required_sections {
        assert!(
            CRATE_API_DOC.contains(section),
            "CRATE_API.md must contain mandatory section: `{section}`"
        );
    }
}

#[test]
fn effective_permission_contract_enforces_least_privilege() {
    // A Pages manage grant implies every editor capability
    assert_eq!(
        pages_editor_permissions_from_effective(&["pages:manage".to_string()]),
        CapabilityState::full()
    );

    // Update plus publish grants receive full permissions
    assert_eq!(
        pages_editor_permissions_from_effective(&[
            "pages:update".to_string(),
            "pages:publish".to_string(),
        ]),
        CapabilityState::full()
    );

    // An update-only grant can author but cannot publish
    let author = pages_editor_permissions_from_effective(&["pages:update".to_string()]);
    assert!(author.edit);
    assert!(author.properties);
    assert!(author.assets);
    assert!(!author.publish);

    // Permissions outside the Pages resource, missing snapshots, and
    // malformed entries fail closed to read-only
    assert_eq!(
        pages_editor_permissions_from_effective(&["products:manage".to_string()]),
        CapabilityState::read_only()
    );
    assert_eq!(
        pages_editor_permissions_from_effective(&[]),
        CapabilityState::read_only()
    );
    assert_eq!(
        pages_editor_permissions_from_effective(&["pages".to_string()]),
        CapabilityState::read_only()
    );
}

#[test]
fn selected_starter_requires_an_exact_catalog_key() {
    let item = crate::model::PageStarterSummary {
        key: "team".into(),
        name: "Team".into(),
        category: "about".into(),
        description: None,
        default_template_key: "default".into(),
        revision: Some("revision".into()),
    };
    assert_eq!(crate::core::selected_page_starter(&[item.clone()], "team"), Some(item));
    assert!(crate::core::selected_page_starter(&[], "blank").is_none());
}

#[test]
fn symbol_usage_contract_prevents_unsafe_deletion() {
    let templates = vec![PageTemplateSummary {
        key: "landing".to_string(),
        locale: "en".to_string(),
        header_symbol_ids: vec!["site-header".to_string()],
        footer_symbol_ids: vec!["site-footer".to_string()],
        revision: 1,
    }];

    // Symbol referenced in template
    let usage: SiteSymbolUsageInfo = compute_symbol_usage("site-header", &templates, vec![]);
    assert!(usage.is_used());
    assert!(!usage.can_delete_safely());
    assert!(usage.deletion_warning().is_some());

    // Symbol not referenced
    let unused = compute_symbol_usage("unused-promo", &templates, vec![]);
    assert!(!unused.is_used());
    assert!(unused.can_delete_safely());
    assert!(unused.deletion_warning().is_none());
}

#[test]
fn template_and_symbol_validation_contracts() {
    // Template key bounds
    assert_eq!(validate_template_key("default").unwrap(), "default");
    assert_eq!(
        validate_template_key("").unwrap_err(),
        TemplateValidationError::EmptyKey
    );
    assert_eq!(
        validate_template_key("invalid key!").unwrap_err(),
        TemplateValidationError::InvalidCharacters
    );

    // Symbol ID bounds
    assert_eq!(validate_symbol_id("my-symbol").unwrap(), "my-symbol");
    assert_eq!(
        validate_symbol_id("").unwrap_err(),
        SymbolValidationError::EmptyId
    );
    assert_eq!(
        validate_symbol_id("bad id!").unwrap_err(),
        SymbolValidationError::InvalidCharacters
    );
}

#[test]
fn stable_problem_detail_conflict_codes() {
    assert_eq!(BROWSER_REVISION_CONFLICT_CODE, "REVISION_CONFLICT");
    assert_eq!(BROWSER_PROJECT_HASH_CONFLICT_CODE, "PROJECT_HASH_CONFLICT");
    assert_eq!(
        BROWSER_DRAFT_GENERATION_CONFLICT_CODE,
        "DRAFT_GENERATION_CONFLICT"
    );
    assert_eq!(BROWSER_DOCUMENT_UNAVAILABLE_CODE, "DOCUMENT_UNAVAILABLE");
}

#[test]
fn starter_metadata_update_uses_required_revision_and_no_document() {
    let input = crate::model::PageStarterMetadataUpdate {
        key: "team".into(), category: "custom".into(), name: "Team".into(),
        description: None, default_template_key: "default".into(),
        expected_revision: "observed-revision".into(),
    };
    let value = serde_json::to_value(input).expect("serialize metadata");
    assert_eq!(value["expectedRevision"], "observed-revision");
    assert_eq!(value["defaultTemplateKey"], "default");
    assert!(value.get("contentJson").is_none());
    assert!(value.get("thumbnailUrl").is_none());
}

#[test]
fn selected_layout_must_be_in_the_exact_catalog_snapshot() {
    let keys = vec!["default".to_string(), "site".to_string()];
    assert!(crate::core::selected_layout_available(&keys, "site"));
    assert!(!crate::core::selected_layout_available(&keys, "SITE"));
    assert!(!crate::core::selected_layout_available(&[], "default"));
}

#[test]
fn changing_page_clears_only_the_route_draft_in_the_same_update() {
    use crate::core::{FLY_DRAFT_QUERY_KEY, page_selection_query_update};
    let page_key = rustok_ui_core::AdminQueryKey::PageId.as_str().to_string();
    assert_eq!(
        page_selection_query_update(Some("new-page".to_string())),
        vec![(page_key.clone(), Some("new-page".to_string())), (FLY_DRAFT_QUERY_KEY.to_string(), None)]
    );
    assert_eq!(
        page_selection_query_update(None),
        vec![(page_key, None), (FLY_DRAFT_QUERY_KEY.to_string(), None)]
    );
}
