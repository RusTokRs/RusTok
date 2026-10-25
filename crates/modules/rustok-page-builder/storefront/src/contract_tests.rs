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
fn test_bridge_script_constant_integrity() {
    assert!(FLY_STOREFRONT_BRIDGE_JS.contains("fly:action"));
    assert!(FLY_STOREFRONT_BRIDGE_JS.contains("rustok:cart:add"));
}

#[test]
fn test_scope_landing_css_contract() {
    let css = "body { margin: 0; } .card { padding: 10px; }";
    let scoped = scope_landing_css(css, ".rustok-page-builder-storefront");
    assert!(scoped.contains("@scope (.rustok-page-builder-storefront)"));
    assert!(scoped.contains(":scope"));
}
