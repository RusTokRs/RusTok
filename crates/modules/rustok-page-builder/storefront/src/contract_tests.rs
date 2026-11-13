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
    assert!(FLY_STOREFRONT_BRIDGE_JS.contains("buyerHandoffHref"));
    assert!(FLY_STOREFRONT_BRIDGE_JS.contains("cart_unavailable"));
    assert!(!FLY_STOREFRONT_BRIDGE_JS.contains("rustok:cart:add"));
}

#[test]
fn every_storefront_surface_installs_the_action_runtime_and_bridge() {
    for (file, source, expected) in [
        ("lib.rs", include_str!("lib.rs"), 2),
        ("localized_route.rs", include_str!("localized_route.rs"), 1),
        ("inline_edit.rs", include_str!("inline_edit.rs"), 1),
    ] {
        assert_eq!(
            source
                .matches("inner_html=fly_browser::FLY_ACTIONS_JS")
                .count(),
            expected,
            "{file}"
        );
        assert_eq!(
            source
                .matches("data-fly-storefront-bridge=\"true\" nonce=bridge_nonce")
                .count(),
            expected,
            "{file}"
        );
        assert_eq!(
            source.matches("nonce=runtime_nonce").count(),
            expected,
            "{file}"
        );
        assert_eq!(
            source.matches("<LandingBridgeFeedback />").count(),
            expected,
            "{file}"
        );
    }
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
    assert_eq!(
        storefront_scoped_css(".card { color: red; }"),
        format!(
            "{LANDING_CONTAINMENT_CSS}{}",
            scope_landing_css(".card { color: red; }", LANDING_SCOPE_SELECTOR)
        )
    );
    let surfaces = [
        ("lib.rs", include_str!("lib.rs"), 2),
        ("localized_route.rs", include_str!("localized_route.rs"), 1),
        ("inline_edit.rs", include_str!("inline_edit.rs"), 1),
    ];
    for (file, source, expected) in surfaces {
        assert_eq!(
            source
                .matches("<style data-fly-project-styles=\"true\" nonce=style_nonce>")
                .count(),
            expected,
            "{file}: unexpected number of project stylesheets"
        );
        assert_eq!(
            source
                .matches(
                    "<style data-fly-project-styles=\"true\" nonce=style_nonce>{scoped_css}</style>"
                )
                .count(),
            expected,
            "{file}: every project stylesheet must be the scoped one"
        );
        assert_eq!(
            source
                .lines()
                .filter(|line| { line.contains("storefront_scoped_css(&") })
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

#[cfg(feature = "ssr")]
#[test]
fn static_landing_ssr_nonces_only_the_host_owned_style_and_scripts() {
    use leptos::prelude::*;
    let owner = Owner::new();
    let html = owner.with(|| {
        provide_context(UiCspNonce::new("request_nonce_123").expect("valid nonce"));
        provide_context(UiRouteContext {
            locale: Some("ru".to_string()),
            ..UiRouteContext::default()
        });
        view! {
            <StaticLandingStorefront
                html="<button data-fly-action='{}'>Go</button><script data-untrusted=\"true\">window.bad=true</script>".to_string()
                css=".card { color: red; }".to_string()
            />
        }.to_html()
    });
    fn opening_tag<'a>(html: &'a str, marker: &str) -> &'a str {
        let at = html.find(marker).expect("element marker");
        let start = html[..at].rfind('<').expect("element start");
        let end = at + html[at..].find('>').expect("element end");
        &html[start..=end]
    }
    for marker in [
        "data-fly-project-styles=\"true\"",
        "data-fly-actions=\"true\"",
        "data-fly-storefront-bridge=\"true\"",
    ] {
        assert!(
            opening_tag(&html, marker).contains("nonce=\"request_nonce_123\""),
            "{marker}: {html}"
        );
    }
    assert!(!opening_tag(&html, "data-untrusted=\"true\"").contains("nonce="));
    assert!(html.contains("@scope ([data-fly-page-body])"));
    assert!(html.contains("Ссылка для покупки неполна"));
    assert!(html.contains("data-fly-bridge-error=\"cart_unavailable\""));
}

#[cfg(feature = "ssr")]
#[test]
fn dynamic_and_localized_landing_ssr_receive_the_host_nonce() {
    use leptos::prelude::*;
    use serde_json::json;

    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {"slug": "home"},
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "btn",
                    "type": "button",
                    "content": "Toggle",
                    "flyAction": {"kind": "toggle_visibility", "target_id": "panel"}
                }, {
                    "id": "panel",
                    "type": "section",
                    "content": "Panel"
                }]
            }
        }]
    });
    let render = |localized| {
        Owner::new().with(|| {
            provide_context(UiCspNonce::new("request_nonce_456").expect("valid nonce"));
            if localized {
                view! {
                    <LocalizedPageBuilderStorefront project_data=project.clone() requested_slug="home".to_string() />
                }
                .to_html()
            } else {
                view! { <PageBuilderStorefront project_data=project.clone() /> }.to_html()
            }
        })
    };
    for html in [render(false), render(true)] {
        assert!(!html.contains("data-render-error=\"true\""), "{html}");
        for marker in [
            "data-fly-project-styles=\"true\"",
            "data-fly-actions=\"true\"",
            "data-fly-storefront-bridge=\"true\"",
        ] {
            let at = html.find(marker).expect("element marker");
            let start = html[..at].rfind('<').expect("element start");
            let end = at + html[at..].find('>').expect("element end");
            assert!(
                html[start..=end].contains("nonce=\"request_nonce_456\""),
                "{marker}: {html}"
            );
        }
        assert!(html.contains("data-fly-action-kind=\"toggle_visibility\""));
    }
}

#[cfg(feature = "ssr")]
#[test]
fn static_landing_ssr_without_nonce_does_not_invent_one() {
    use leptos::prelude::*;
    let html = Owner::new().with(|| {
        view! {
            <StaticLandingStorefront html="<p>Landing</p>".to_string() css=".card { color: red; }".to_string() />
        }.to_html()
    });
    assert!(!html.contains(" nonce="), "{html}");
}
