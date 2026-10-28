/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use super::*;

#[test]
fn test_crate_api_defines_minimal_contract_sections() {
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
fn test_denial_code_contract() {
    assert_eq!(BROWSER_CAPABILITY_DENIAL_CODE, "FLY_CAPABILITY_DENIED");
}

#[test]
fn test_consumer_properties_constants() {
    assert_eq!(
        PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT,
        "page_builder_consumer_properties_v1"
    );
    assert_eq!(
        CONSUMER_PROPERTY_CONTRACT_INVALID,
        "PAGE_BUILDER_CONSUMER_PROPERTY_CONTRACT_INVALID"
    );
    assert_eq!(
        CONSUMER_PROPERTY_EDITOR_UNAVAILABLE,
        "PAGE_BUILDER_CONSUMER_PROPERTY_EDITOR_UNAVAILABLE"
    );
    assert_eq!(
        CONSUMER_PROPERTY_SAVE_FAILED,
        "PAGE_BUILDER_CONSUMER_PROPERTY_SAVE_FAILED"
    );
}

#[test]
fn test_in_memory_ssr_draft_session_store_lifecycle() {
    let store = InMemorySsrDraftSessionStore::default();
    assert_eq!(
        store.prune().expect("prune on empty store should succeed"),
        0
    );
    assert!(
        store
            .load("page-1", "non-existent")
            .expect("load should return Ok")
            .is_none()
    );
}
