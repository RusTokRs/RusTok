//! Contract tests verifying the architectural invariants and public API of `rustok-pages-storefront`.

use crate::builder::{
    PAGE_BUILDER_DOCUMENT_FORMAT, STATIC_LANDING_PAYLOAD_BODY_FORMAT,
    STATIC_LANDING_URL_BODY_FORMAT, is_page_builder_body,
};
use crate::core::{
    count_label, open_link_label, page_link_href, selected_page_effective_locale,
    selected_page_slug, selected_page_title,
};
use crate::model::{PageBody, PageDetail, PageTranslation};
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
fn format_constants_adhere_to_platform_spec() {
    assert_eq!(PAGE_BUILDER_DOCUMENT_FORMAT, "grapesjs");
    assert_eq!(STATIC_LANDING_PAYLOAD_BODY_FORMAT, "fly_artifact_payload");
    assert_eq!(STATIC_LANDING_URL_BODY_FORMAT, "fly_artifact_url");
}

#[test]
fn page_builder_format_detection_contract() {
    let supported_formats = [
        PAGE_BUILDER_DOCUMENT_FORMAT,
        STATIC_LANDING_PAYLOAD_BODY_FORMAT,
        STATIC_LANDING_URL_BODY_FORMAT,
    ];

    for format in supported_formats {
        let body = PageBody {
            locale: "en".to_string(),
            content: "{}".to_string(),
            format: format.to_string(),
        };
        assert!(
            is_page_builder_body(&body),
            "Format `{format}` must be recognized as page builder content"
        );
    }

    let unsupported = PageBody {
        locale: "en".to_string(),
        content: "plain text".to_string(),
        format: "text/plain".to_string(),
    };
    assert!(!is_page_builder_body(&unsupported));
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
        body: None,
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
