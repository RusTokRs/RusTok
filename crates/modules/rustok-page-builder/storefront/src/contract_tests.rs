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
    let scoped = scope_landing_css(css, LANDING_SCOPE_SELECTOR);
    assert_eq!(
        scoped,
        "@scope ([data-fly-page-body]) {\n  :scope {\n    isolation: isolate;\n  }\n:scope { margin: 0; } .card { padding: 10px; }\n}"
    );
}

/// Every surface that emits project CSS must confine it, and the scope root
/// must be an element the surface renders itself. Before this contract the
/// stylesheet was scoped to a class the host could replace (leaving the
/// landing unstyled), and two surfaces emitted it unscoped.
#[test]
fn test_every_surface_scopes_project_css_to_the_landing_body() {
    assert_eq!(LANDING_SCOPE_SELECTOR, "[data-fly-page-body]");
    let surfaces = [
        ("lib.rs", include_str!("lib.rs"), 2),
        ("localized_route.rs", include_str!("localized_route.rs"), 1),
        ("inline_edit.rs", include_str!("inline_edit.rs"), 1),
    ];
    for (file, source, expected) in surfaces {
        assert_eq!(
            source
                .matches("<style data-fly-project-styles=\"true\">")
                .count(),
            expected,
            "{file}: unexpected number of project stylesheets"
        );
        assert_eq!(
            source
                .matches("<style data-fly-project-styles=\"true\">{scoped_css}</style>")
                .count(),
            expected,
            "{file}: every project stylesheet must be the scoped one"
        );
        assert_eq!(
            source
                .lines()
                .filter(|line| {
                    line.contains("scope_landing_css(&") && line.contains("LANDING_SCOPE_SELECTOR)")
                })
                .count(),
            expected,
            "{file}: every stylesheet must be scoped to LANDING_SCOPE_SELECTOR"
        );
        assert_eq!(
            source
                .matches("<div data-fly-page-body=\"true\" inner_html=html></div>")
                .count(),
            expected,
            "{file}: every stylesheet needs its scope root next to it"
        );
    }
}
