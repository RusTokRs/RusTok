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
