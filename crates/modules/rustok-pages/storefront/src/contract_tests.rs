//! Contract tests verifying the architectural invariants and public API of `rustok-pages-storefront`.

use crate::core::{
    count_label, open_link_label, page_link_href, selected_page_effective_locale,
    selected_page_slug, selected_page_title,
};
use crate::model::{PageDetail, PageTranslation, PublishedLanding};
use crate::transport::{StorefrontPageRouteDecision, StorefrontPageRouteDisposition};

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
fn page_detail_carries_the_published_landing_and_no_authoring_document() {
    let page = PageDetail {
        effective_locale: Some("en".to_string()),
        translation: None,
        landing: Some(PublishedLanding {
            locale: "en".to_string(),
            html: "<h1>Landing</h1>".to_string(),
            css: "h1 { color: red; }".to_string(),
            artifact_hash: "hash".to_string(),
        }),
    };
    let encoded = serde_json::to_value(&page).expect("page detail must serialize");
    let mut keys = encoded
        .as_object()
        .expect("page detail is an object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, ["effectiveLocale", "landing", "translation"]);

    let mut landing_keys = encoded["landing"]
        .as_object()
        .expect("landing is an object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    landing_keys.sort_unstable();
    assert_eq!(landing_keys, ["artifactHash", "css", "html", "locale"]);
}

#[test]
fn core_view_model_helpers_operate_without_ui_runtime() {
    let page = PageDetail {
        effective_locale: Some("en".to_string()),
        translation: Some(PageTranslation {
            locale: "en".to_string(),
            title: Some("About Us".to_string()),
            slug: Some("about-us".to_string()),
            meta_title: None,
            meta_description: None,
        }),
        landing: None,
    };

    assert_eq!(
        selected_page_title(&page, "Fallback".to_string()),
        "About Us"
    );
    assert_eq!(
        selected_page_slug(&page, "fallback".to_string()),
        "about-us"
    );
    assert_eq!(
        selected_page_effective_locale(&page, "ru".to_string()),
        "en"
    );

    assert_eq!(count_label("Pages: {count}", 42), "Pages: 42");
    assert_eq!(open_link_label("Open", "about-us"), "Open about-us");
    assert_eq!(page_link_href("/pages", "about-us"), "/pages?slug=about-us");
}

#[test]
fn route_decision_contract_serialization() {
    let decision = StorefrontPageRouteDecision {
        disposition: StorefrontPageRouteDisposition::Canonical,
        canonical_path: Some("/pages/privacy".to_string()),
        canonical_slug: Some("privacy".to_string()),
        canonical_page_id: Some("uuid-123".to_string()),
        canonical_locale: Some("en".to_string()),
        channel_id: Some("channel-1".to_string()),
        route_generation: Some(1),
        page_generation: Some(2),
        artifact_generation: Some(3),
    };

    let serialized = serde_json::to_string(&decision).expect("Decision must serialize");
    let deserialized: StorefrontPageRouteDecision =
        serde_json::from_str(&serialized).expect("Decision must deserialize");

    assert_eq!(decision, deserialized);
    assert_eq!(
        decision.disposition,
        StorefrontPageRouteDisposition::Canonical
    );
}
