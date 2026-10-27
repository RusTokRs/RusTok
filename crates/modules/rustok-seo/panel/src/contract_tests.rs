//! Public contract tests for `rustok-seo-panel`.

use super::*;
use rustok_seo_targets::{SeoTargetSlug, builtin_slug as seo_builtin_slug};
use uuid::Uuid;

#[test]
fn public_surface_matches_contract() {
    let target_slug = SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("valid page target slug");
    assert_eq!(target_slug.as_str(), "page");

    let form = SeoEntityForm::new("en-US".to_string());
    assert_eq!(form.locale, "en-US");
    assert_eq!(form.title, "");
    assert_eq!(form.description, "");
    assert!(!form.noindex);
    assert!(!form.nofollow);

    let report = form.completeness_report();
    assert_eq!(report.score, 0);
    assert!(
        report
            .recommendations
            .contains(&crate::model::SeoRecommendation::AddSeoTitle)
    );
    assert!(
        report
            .recommendations
            .contains(&crate::model::SeoRecommendation::AddMetaDescription)
    );

    let valid_id = Uuid::new_v4();
    let validated = validate_target_id(&valid_id.to_string()).expect("valid target id");
    assert_eq!(validated, valid_id);

    let nil_err = validate_target_id("00000000-0000-0000-0000-000000000000")
        .expect_err("nil target id must be rejected");
    assert_eq!(nil_err, "Invalid target id.");

    let state = derive_control_plane_widget_state(false, false, None);
    assert_eq!(state.kind, SeoControlPlaneWidgetStateKind::Empty);

    let ready_state = derive_control_plane_widget_state(true, false, None);
    assert_eq!(ready_state.kind, SeoControlPlaneWidgetStateKind::Ready);

    let hint = remediation_hint_for_issue_code("duplicate_canonical_url");
    assert_eq!(hint.action, crate::model::SeoRemediationAction::OpenBulkJob);
    assert_eq!(hint.reason_key, "bulk_consistency_fix");
}
