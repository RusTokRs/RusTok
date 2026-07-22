use super::*;
use crate::{GrapesJsCodec, ProjectDocument, ValidationSeverity};
use serde_json::json;

fn ready_document() -> ProjectDocument {
    GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": { "$localized": { "en": "Home", "ru": "Главная" } },
                "description": "A stable landing page",
                "slug": { "$localized": { "en": "home", "ru": "glavnaya" } }
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "tagName": "main",
                "components": [{
                    "id": "hero-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Welcome"
                }]
            }
        }]
    }))
    .expect("document")
}

#[test]
fn localized_metadata_counts_as_ready_content() {
    let report = evaluate_landing_readiness(&ready_document(), LandingReadinessPolicy::default());
    assert!(report.ready, "{:?}", report.issues);
    assert_eq!(report.page_count, 1);
    assert_eq!(report.categories.len(), 5);
}

#[test]
fn missing_landing_contracts_block_readiness() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": { "id": "root", "type": "wrapper" }
        }]
    }))
    .expect("document");
    let report = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(!report.ready);
    for code in [
        "landing_page_id_required",
        "landing_page_title_required",
        "landing_page_slug_required",
        "landing_missing_h1",
    ] {
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.diagnostic.code == code),
            "missing readiness code {code}"
        );
    }
}

#[test]
fn warnings_only_block_when_policy_requires_it() {
    let document = ready_document();
    let normal = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(normal.ready);

    let strict = evaluate_landing_readiness(
        &document,
        LandingReadinessPolicy {
            block_on_warnings: true,
            ..LandingReadinessPolicy::default()
        },
    );
    assert_eq!(
        strict.ready,
        !strict.issues.iter().any(|issue| {
            matches!(
                issue.diagnostic.severity,
                ValidationSeverity::Error | ValidationSeverity::Warning
            )
        })
    );
}

#[test]
fn required_locale_coverage_gaps_block_readiness_without_strict_locale_validation() {
    let mut source = GrapesJsCodec::encode_value(&ready_document()).expect("document value");
    source["flyLocales"] = json!({
        "default_locale": "en",
        "supported_locales": ["en", "ru"],
        "required_locales": ["en", "ru"],
        "enforce_required_locales": false
    });
    source["flyTranslations"] = json!([{
        "id": "hero",
        "values": { "en": "Welcome" }
    }]);
    let document = GrapesJsCodec::decode_value(source).expect("document");

    let report = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(!report.ready);
    assert!(report.issues.iter().any(|issue| {
        issue.category == LandingReadinessCategory::Locales
            && issue.diagnostic.code == "landing_translation_locale_missing"
            && issue.diagnostic.severity == ValidationSeverity::Error
    }));
    assert!(
        report
            .categories
            .iter()
            .find(|summary| summary.category == LandingReadinessCategory::Locales)
            .is_some_and(|summary| summary.error_count > 0)
    );
}

#[test]
fn structural_readiness_does_not_require_runtime_instance_data() {
    let mut source = GrapesJsCodec::encode_value(&ready_document()).expect("document value");
    source["flyRuntimeContextSchema"] = json!([{
        "id": "customer-name",
        "path": "customer.name",
        "kind": "string",
        "required": true
    }]);
    let document = GrapesJsCodec::decode_value(source).expect("document");

    let report = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(report.ready, "{:?}", report.issues);
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.diagnostic.code == "runtime_context_required_missing")
    );
}

#[test]
fn structural_readiness_applies_schema_defaults_before_audit() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Home",
                "description": "A stable landing page",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "tagName": "main",
                "components": [{
                    "id": "hero-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": ""
                }]
            }
        }],
        "flyRuntimeContextSchema": [{
            "id": "hero-title",
            "path": "hero.title",
            "kind": "string",
            "required": true,
            "default": "Default heading"
        }],
        "flyRuntimeBindings": [{
            "id": "hero-heading-content",
            "component_id": "hero-heading",
            "path": "hero.title",
            "target": "field",
            "name": "content"
        }]
    }))
    .expect("document");

    let report = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(report.ready, "{:?}", report.issues);
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.diagnostic.code == "landing_empty_heading")
    );
}

#[test]
fn structural_readiness_validates_binding_fallback_contracts() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Home",
                "description": "A stable landing page",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "tagName": "main",
                "components": [{
                    "id": "hero-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Welcome"
                }, {
                    "id": "cta",
                    "type": "link",
                    "content": "Home",
                    "flyPageLink": { "page_id": "home" }
                }]
            }
        }],
        "flyRuntimeBindings": [{
            "id": "cta-action",
            "component_id": "cta",
            "path": "cta.action",
            "target": "field",
            "name": "flyAction",
            "fallback": { "kind": "navigate_page", "page_id": "home" }
        }]
    }))
    .expect("document");

    let report = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(!report.ready);
    assert!(report.issues.iter().any(|issue| {
        issue.diagnostic.code == "component_navigation_contract_conflict"
            && issue.diagnostic.severity == ValidationSeverity::Error
    }));
}

#[test]
fn localized_slug_diagnostics_are_classified_as_routes() {
    let mut source = GrapesJsCodec::encode_value(&ready_document()).expect("document value");
    source["pages"][0]["flyPageMeta"]["slug"] = json!({
        "$localized": { "en": " " }
    });
    let document = GrapesJsCodec::decode_value(source).expect("document");

    let report = evaluate_landing_readiness(&document, LandingReadinessPolicy::default());
    assert!(report.issues.iter().any(|issue| {
        issue.category == LandingReadinessCategory::Routes
            && issue.diagnostic.code == "localized_page_slug_empty"
    }));
}

#[test]
fn unresolved_runtime_bound_action_is_a_publish_blocker() {
    let mut source = GrapesJsCodec::encode_value(&ready_document()).expect("document value");
    source["pages"][0]["component"]["components"]
        .as_array_mut()
        .expect("components")
        .push(json!({
            "id": "cta",
            "type": "button",
            "content": "Open page"
        }));
    source["flyRuntimeBindings"] = json!([{
        "id": "cta-action",
        "component_id": "cta",
        "path": "cta.action",
        "target": "field",
        "name": "flyAction"
    }]);
    let document = GrapesJsCodec::decode_value(source).expect("document");
    let context = json!({
        "cta": {
            "action": { "kind": "navigate_page", "page_id": "missing" }
        }
    });

    let report = evaluate_landing_readiness_with_context(
        &document,
        Some(&context),
        LandingReadinessPolicy::default(),
    );
    assert!(!report.ready);
    assert!(report.issues.iter().any(|issue| {
        issue.category == LandingReadinessCategory::RuntimeContracts
            && issue.diagnostic.code == "runtime_action_unresolved"
            && issue.diagnostic.severity == ValidationSeverity::Error
    }));
}
