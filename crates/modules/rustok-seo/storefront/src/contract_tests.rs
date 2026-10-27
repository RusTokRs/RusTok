//! Public contract tests for `rustok-seo-storefront`.

use super::*;

#[test]
fn public_surface_matches_contract() {
    let context = SeoPageContext::default();
    let head = render_head_html(&context);
    assert_eq!(head, r#"<meta name="robots" content="index, follow" />"#);

    let head_with_nonce = render_head_html_with_nonce(&context, Some("xyz-nonce"));
    assert_eq!(
        head_with_nonce,
        r#"<meta name="robots" content="index, follow" />"#
    );

    let vars = SeoPageContextVariables {
        route: "/products/widget".to_string(),
        locale: "en-US".to_string(),
    };
    assert_eq!(vars.route, "/products/widget");
    assert_eq!(vars.locale, "en-US");

    assert!(SEO_PAGE_CONTEXT_QUERY.contains("query SeoPageContext"));
}

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
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
