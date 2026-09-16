/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::bundle::build_fluent_catalog_report;
use rustok_ui_i18n::{try_build_fluent_catalog, BundleBuildError, UiTranslator};

#[test]
fn thousand_message_resource_resolves_across_the_catalog() {
    const MESSAGE_COUNT: usize = 1_000;

    let mut source = String::new();
    for index in 0..MESSAGE_COUNT {
        source.push_str(&format!("item-{index} = Value {index}\n"));
    }

    let entries = [("en", source.as_str())];
    let catalog = try_build_fluent_catalog(&entries).expect("large valid resource should build");
    let translator = UiTranslator::new(&catalog, "en");

    for index in [0, MESSAGE_COUNT / 2, MESSAGE_COUNT - 1] {
        let key = format!("item-{index}");
        let expected = format!("Value {index}");
        assert_eq!(translator.t(Some("en"), &key, "fallback"), expected);
    }
}

#[test]
fn strict_catalog_handles_many_locale_bundles_without_collision() {
    const LOCALE_COUNT: usize = 64;

    let owned: Vec<(String, String)> = (1..=LOCALE_COUNT)
        .map(|region| {
            (
                format!("en-{region:03}"),
                format!("title = Region {region:03}\n"),
            )
        })
        .collect();
    let entries: Vec<(&str, &str)> = owned
        .iter()
        .map(|(locale, source)| (locale.as_str(), source.as_str()))
        .collect();

    let catalog = try_build_fluent_catalog(&entries).expect("large valid catalog should build");
    assert_eq!(catalog.len(), LOCALE_COUNT);

    let translator = UiTranslator::new(&catalog, "en-001");
    assert_eq!(translator.t(Some("en-001"), "title", "fallback"), "Region 001");
    assert_eq!(translator.t(Some("en-064"), "title", "fallback"), "Region 064");
}

#[test]
fn mixed_malformed_batch_retains_bounded_diagnostics_in_input_order() {
    const VALID_COUNT: usize = 32;
    const INVALID_LOCALE_COUNT: usize = 32;
    const OVERSIZED_LOCALE_COUNT: usize = 32;
    const MALFORMED_RESOURCE_COUNT: usize = 32;

    let mut owned = Vec::<(String, String)>::new();

    for region in 1..=VALID_COUNT {
        owned.push((
            format!("en-{region:03}"),
            format!("title = Valid {region:03}\n"),
        ));
    }

    for index in 0..INVALID_LOCALE_COUNT {
        owned.push((
            format!("invalid!{index}"),
            "title = Invalid locale should be skipped\n".to_string(),
        ));
    }

    for index in 0..OVERSIZED_LOCALE_COUNT {
        owned.push((
            format!("{}-{index}", "x".repeat(96)),
            "title = Oversized locale should be skipped\n".to_string(),
        ));
    }

    for region in 101..(101 + MALFORMED_RESOURCE_COUNT) {
        owned.push((
            format!("fr-{region:03}"),
            "this is not valid fluent".to_string(),
        ));
    }

    let entries: Vec<(&str, &str)> = owned
        .iter()
        .map(|(locale, source)| (locale.as_str(), source.as_str()))
        .collect();
    let report = build_fluent_catalog_report(&entries);

    assert_eq!(report.catalog().len(), VALID_COUNT);
    assert_eq!(
        report.diagnostics().len(),
        INVALID_LOCALE_COUNT + OVERSIZED_LOCALE_COUNT + MALFORMED_RESOURCE_COUNT
    );

    let invalid_end = INVALID_LOCALE_COUNT;
    let oversized_end = invalid_end + OVERSIZED_LOCALE_COUNT;

    for error in &report.diagnostics()[..invalid_end] {
        assert!(matches!(error, BundleBuildError::InvalidLocale { .. }));
    }

    for error in &report.diagnostics()[invalid_end..oversized_end] {
        match error {
            BundleBuildError::LocaleTooLong { length, max_len } => {
                assert!(*length > *max_len);
                assert_eq!(*max_len, 64);
            }
            _ => panic!("expected bounded LocaleTooLong diagnostic, got {error:?}"),
        }
    }

    for error in &report.diagnostics()[oversized_end..] {
        assert!(matches!(error, BundleBuildError::FluentParse { .. }));
    }
}
