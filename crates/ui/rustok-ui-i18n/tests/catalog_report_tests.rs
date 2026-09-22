/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::BundleBuildError;
use rustok_ui_i18n::bundle::build_fluent_catalog_report;

#[test]
fn lenient_report_retains_every_skipped_entry_reason_in_input_order() {
    let report = build_fluent_catalog_report(&[
        ("en-US", "title = First\n"),
        ("en_US", "title = Duplicate\n"),
        ("!", "title = Invalid locale\n"),
        ("ru", "this is not valid fluent"),
    ]);

    assert_eq!(report.catalog().len(), 1);
    assert!(report.catalog().contains_key("en-US"));
    assert!(!report.is_clean());
    assert_eq!(report.diagnostics().len(), 3);

    assert!(matches!(
        &report.diagnostics()[0],
        BundleBuildError::DuplicateLocale { locale } if locale == "en-US"
    ));
    assert!(matches!(
        &report.diagnostics()[1],
        BundleBuildError::InvalidLocale { locale, .. } if locale == "!"
    ));
    assert!(matches!(
        &report.diagnostics()[2],
        BundleBuildError::FluentParse { locale, .. } if locale == "ru"
    ));
}

#[test]
fn malformed_first_entry_still_reserves_normalized_locale_identity() {
    let report = build_fluent_catalog_report(&[
        ("en_US", "this is not valid fluent"),
        ("en-US", "title = Later valid duplicate\n"),
        ("ru", "title = Заголовок\n"),
    ]);

    assert!(!report.catalog().contains_key("en-US"));
    assert!(report.catalog().contains_key("ru"));
    assert_eq!(report.diagnostics().len(), 2);

    assert!(matches!(
        &report.diagnostics()[0],
        BundleBuildError::FluentParse { locale, .. } if locale == "en-US"
    ));
    assert!(matches!(
        &report.diagnostics()[1],
        BundleBuildError::DuplicateLocale { locale } if locale == "en-US"
    ));
}

#[test]
fn unparseable_locale_does_not_reserve_a_different_valid_identity() {
    let report = build_fluent_catalog_report(&[
        ("!", "title = Invalid locale\n"),
        ("en", "title = Title\n"),
    ]);

    assert!(report.catalog().contains_key("en"));
    assert_eq!(report.diagnostics().len(), 1);
    assert!(matches!(
        &report.diagnostics()[0],
        BundleBuildError::InvalidLocale { locale, .. } if locale == "!"
    ));
}

#[test]
fn clean_lenient_report_can_be_consumed_without_diagnostics() {
    let report =
        build_fluent_catalog_report(&[("en", "title = Title\n"), ("ru", "title = Заголовок\n")]);

    assert!(report.is_clean());
    let (catalog, diagnostics) = report.into_parts();
    assert_eq!(catalog.len(), 2);
    assert!(diagnostics.is_empty());
}
