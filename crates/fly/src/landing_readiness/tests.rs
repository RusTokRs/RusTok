use super::*;
use crate::{GrapesJsV1Codec, ProjectDocument, ValidationSeverity};
use serde_json::json;

fn ready_document() -> ProjectDocument {
    GrapesJsV1Codec::decode_value(json!({
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
    let report = evaluate_landing_readiness(
        &ready_document(),
        LandingReadinessPolicy::default(),
    );
    assert!(report.ready, "{:?}", report.issues);
    assert_eq!(report.page_count, 1);
    assert_eq!(report.categories.len(), 5);
}

#[test]
fn missing_landing_contracts_block_readiness() {
    let document = GrapesJsV1Codec::decode_value(json!({
        "pages": [{
            "component": { "id": "root", "type": "wrapper" }
        }]
    }))
    .expect("document");
    let report = evaluate_landing_readiness(
        &document,
        LandingReadinessPolicy::default(),
    );
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
    let normal = evaluate_landing_readiness(
        &document,
        LandingReadinessPolicy::default(),
    );
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
    let mut source =
        GrapesJsV1Codec::encode_value(&ready_document()).expect("document value");
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
    let document = GrapesJsV1Codec::decode_value(source).expect("document");

    let report = evaluate_landing_readiness(
        &document,
        LandingReadinessPolicy::default(),
    );
    assert!(!report.ready);
    assert!(report.issues.iter().any(|issue| {
        issue.category == LandingReadinessCategory::Locales
            && issue.diagnostic.code == "landing_translation_locale_missing"
            && issue.diagnostic.severity == ValidationSeverity::Error
    }));
    assert!(report
        .categories
        .iter()
        .find(|summary| summary.category == LandingReadinessCategory::Locales)
        .is_some_and(|summary| summary.error_count > 0));
}

#[test]
fn structural_readiness_does_not_require_runtime_instance_data() {
    let mut source =
        GrapesJsV1Codec::encode_value(&ready_document()).expect("document value");
    source["flyRuntimeContextSchema"] = json!([{
        "id": "customer-name",
        "path": "customer.name",
        "kind": "string",
        "required": true
    }]);
    let document = GrapesJsV1Codec::decode_value(source).expect("document");

    let report = evaluate_landing_readiness(
        &document,
        LandingReadinessPolicy::default(),
    );
    assert!(report.ready, "{:?}", report.issues);
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.diagnostic.code == "runtime_context_required_value_missing"));
}

#[test]
fn localized_slug_diagnostics_are_classified_as_routes() {
    let mut source =
        GrapesJsV1Codec::encode_value(&ready_document()).expect("document value");
    source["pages"][0]["flyPageMeta"]["slug"] = json!({
        "$localized": { "en": " " }
    });
    let document = GrapesJsV1Codec::decode_value(source).expect("document");

    let report = evaluate_landing_readiness(
        &document,
        LandingReadinessPolicy::default(),
    );
    assert!(report.issues.iter().any(|issue| {
        issue.category == LandingReadinessCategory::Routes
            && issue.diagnostic.code == "localized_page_slug_empty"
    }));
}
