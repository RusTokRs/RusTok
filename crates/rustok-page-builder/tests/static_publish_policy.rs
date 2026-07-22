use fly::GrapesJsCodec;
use rustok_page_builder::{
    PageBuilderStaticPublishPolicyError, validate_static_publish_document,
};
use serde_json::json;
use std::collections::BTreeSet;

fn diagnostic_codes(error: &PageBuilderStaticPublishPolicyError) -> BTreeSet<&str> {
    error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

#[test]
fn class_only_grapesjs_rule_is_rejected_instead_of_silently_dropped() {
    let document = GrapesJsCodec::decode_value(json!({
        "styles": [{
            "selectors": ["hero-class"],
            "style": { "padding": "24px" }
        }],
        "pages": [{
            "id": "home",
            "flyPageMeta": { "title": "Home", "slug": "home" },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "hero",
                    "type": "section",
                    "tagName": "section",
                    "attributes": { "class": "hero-class" }
                }]
            }
        }]
    }))
    .expect("project document");

    let error = validate_static_publish_document(&document)
        .expect_err("unbound style rule must fail closed");
    assert!(diagnostic_codes(&error).contains("landing_style_rule_unbound"));
}

#[test]
fn unsafe_and_malformed_assets_are_rejected() {
    let document = GrapesJsCodec::decode_value(json!({
        "assets": [
            { "id": "hero", "type": "image", "src": "http://cdn.example.com/hero.webp" },
            "https://cdn.example.com/opaque.webp"
        ],
        "pages": [{
            "id": "home",
            "flyPageMeta": { "title": "Home", "slug": "home" },
            "component": { "id": "root", "type": "wrapper" }
        }]
    }))
    .expect("project document");

    let error = validate_static_publish_document(&document)
        .expect_err("unsafe asset catalog must fail closed");
    let codes = diagnostic_codes(&error);
    assert!(codes.contains("landing_asset_url_rejected"));
    assert!(codes.contains("landing_asset_invalid"));
}

#[test]
fn component_bound_https_project_style_is_allowed() {
    let document = GrapesJsCodec::decode_value(json!({
        "assets": [{
            "id": "hero-asset",
            "type": "image",
            "src": "https://cdn.example.com/hero.webp"
        }],
        "styles": [{
            "selectors": [{ "name": "hero", "type": 2 }],
            "flyComponentId": "hero",
            "style": { "padding": "24px" }
        }],
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Home",
                "slug": "home",
                "canonical_url": "https://example.com/home",
                "open_graph_image": "https://cdn.example.com/hero.webp"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "hero",
                    "type": "image",
                    "tagName": "img",
                    "attributes": { "src": "https://cdn.example.com/hero.webp" }
                }]
            }
        }]
    }))
    .expect("project document");

    let evidence = validate_static_publish_document(&document).expect("safe policy evidence");
    evidence.verify_integrity().expect("policy evidence integrity");
}
