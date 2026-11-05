//! Contract tests verifying the architectural invariants and public API of `rustok-pages-admin`.

use crate::access::pages_editor_permissions_for_role;
use crate::browser_problem::{
    BROWSER_DRAFT_GENERATION_CONFLICT_CODE, BROWSER_PROJECT_HASH_CONFLICT_CODE,
    BROWSER_REVISION_CONFLICT_CODE,
};
use crate::core::{
    SiteSymbolUsageInfo, SymbolValidationError, TemplateValidationError, compute_symbol_usage,
    validate_symbol_id, validate_template_key,
};
use crate::model::PageTemplateSummary;
use crate::starters::{instantiate_starter_preset, page_starters};
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
fn role_permission_contract_enforces_least_privilege() {
    // Super admin & admin receive full permissions
    assert_eq!(
        pages_editor_permissions_for_role(Some("super_admin")),
        CapabilityState::full()
    );
    assert_eq!(
        pages_editor_permissions_for_role(Some("admin")),
        CapabilityState::full()
    );

    // Manager can author but cannot publish
    let manager = pages_editor_permissions_for_role(Some("manager"));
    assert!(manager.edit);
    assert!(manager.properties);
    assert!(manager.assets);
    assert!(!manager.publish);

    // Customer, unrecognized, or None fail closed to read-only
    assert_eq!(
        pages_editor_permissions_for_role(Some("customer")),
        CapabilityState::read_only()
    );
    assert_eq!(
        pages_editor_permissions_for_role(Some("guest")),
        CapabilityState::read_only()
    );
    assert_eq!(
        pages_editor_permissions_for_role(None),
        CapabilityState::read_only()
    );
}

#[test]
fn starter_catalog_contract_is_consistent() {
    let starters = page_starters();
    assert!(
        !starters.is_empty(),
        "Page starter catalog must not be empty"
    );

    let blank = starters.iter().find(|s| s.id == "blank");
    assert!(blank.is_some(), "Blank starter must always be available");

    for starter in &starters {
        assert!(!starter.id.is_empty());
        assert!(!starter.title.is_empty());
        let doc = instantiate_starter_preset(&starter.id, "Test Page");
        assert!(
            doc.is_ok(),
            "Starter `{}` must instantiate successfully",
            starter.id
        );
    }
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
}
