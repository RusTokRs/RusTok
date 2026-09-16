/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::{Arc, Barrier};
use std::thread;

use rustok_ui_i18n::{
    build_fluent_bundle, build_fluent_catalog, fluent_args, locale_candidates,
    try_build_fluent_catalog, BundleBuildError, UiMessages,
};

fn strip_bidi_isolates(value: &str) -> String {
    value.replace('\u{2068}', "").replace('\u{2069}', "")
}

#[test]
fn locale_fallback_matrix_preserves_language_script_region_and_variant_levels() {
    assert_eq!(
        locale_candidates(Some("zh-Hant-TW"), "en-GB"),
        vec!["zh-Hant-TW", "zh-Hant", "zh", "en-GB", "en"]
    );
    assert_eq!(
        locale_candidates(Some("zh-Hans-CN"), "en"),
        vec!["zh-Hans-CN", "zh-Hans", "zh", "en"]
    );
    assert_eq!(
        locale_candidates(Some("de-DE-1901"), "en-GB"),
        vec!["de-DE-1901", "de-DE", "de", "en-GB", "en"]
    );
    assert_eq!(
        locale_candidates(Some("sl-rozaj-biske"), "en"),
        vec!["sl-biske-rozaj", "sl-biske", "sl", "en"]
    );
}

#[test]
fn duplicate_message_ids_fail_strict_resource_construction() {
    let source = "title = First\ntitle = Second\n";

    let error = build_fluent_bundle("en", source)
        .expect_err("duplicate Fluent message ids must be rejected by strict construction");
    assert!(matches!(error, BundleBuildError::AddResource { .. }));

    let error = try_build_fluent_catalog(&[("en", source)])
        .expect_err("strict catalog construction must propagate duplicate message ids");
    assert!(matches!(error, BundleBuildError::AddResource { .. }));
}

#[test]
fn lenient_catalog_skips_a_resource_with_duplicate_message_ids() {
    let catalog = build_fluent_catalog(&[("en", "title = First\ntitle = Second\n")]);
    assert!(catalog.is_empty());
}

#[test]
fn concurrent_first_lookup_initializes_once_lock_without_semantic_races() {
    const EN: &str = r#"
items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
"#;
    const RU: &str = r#"
items = { $count ->
    [one] { $count } товар
    [few] { $count } товара
   *[other] { $count } товаров
}
"#;

    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", EN), ("ru", RU)]);

    const THREADS: usize = 24;
    let barrier = Arc::new(Barrier::new(THREADS));
    let mut handles = Vec::with_capacity(THREADS);

    for thread_index in 0..THREADS {
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();

            let count = if thread_index % 2 == 0 { 22 } else { 5 };
            let locale = if thread_index % 3 == 0 { "ru-RU" } else { "en-US" };
            let args = fluent_args!(count = count);
            let actual = MESSAGES.format(Some(locale), "items", Some(&args), "fallback");

            let visible = strip_bidi_isolates(&actual);
            if locale.starts_with("ru") {
                assert_eq!(visible, "22 товара");
            } else {
                assert_eq!(visible, "5 items");
            }
        }));
    }

    for handle in handles {
        handle.join().expect("concurrent first lookup thread panicked");
    }

    assert_eq!(
        strip_bidi_isolates(&MESSAGES.t(Some("en"), "items", "fallback")),
        "{ $count } items"
    );
}
