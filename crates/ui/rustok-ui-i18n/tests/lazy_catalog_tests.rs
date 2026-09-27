/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::Arc;
use std::thread;

use rustok_ui_i18n::{BundleBuildError, LazyUiMessages};

const EN: &str = "hello = Hello\ndefault-only = Default\ncard = Card\n    .label = English label\n";
const FR: &str = "hello = Bonjour\ncard = Carte\n    .label = Libellé français\n";
const AR: &str = "hello = مرحبًا\n";
const CATALOG: &[(&str, &str)] = &[("en", EN), ("fr", FR), ("ar", AR)];

#[test]
fn locale_resources_are_parsed_only_when_the_fallback_chain_needs_them() {
    let messages = LazyUiMessages::new("en", CATALOG);

    assert_eq!(
        messages.declared_locales().collect::<Vec<_>>(),
        vec!["ar", "en", "fr"]
    );
    assert_eq!(messages.loaded_locales().count(), 0);

    assert_eq!(messages.t(Some("fr-CA"), "hello", "fallback"), "Bonjour");
    assert_eq!(messages.loaded_locales().collect::<Vec<_>>(), vec!["fr"]);

    assert_eq!(
        messages.t(Some("fr-CA"), "default-only", "fallback"),
        "Default"
    );
    assert_eq!(
        messages.loaded_locales().collect::<Vec<_>>(),
        vec!["en", "fr"]
    );
    assert!(!messages.loaded_locales().any(|locale| locale == "ar"));
}

#[test]
fn lazy_lookup_preserves_attributes_provenance_and_prepared_candidates() {
    let messages = LazyUiMessages::new("en", CATALOG);
    let translator = messages.for_locale(Some("fr-CA-u-ca-gregory"));

    assert_eq!(
        translator.candidates(),
        &["fr-CA".to_string(), "fr".to_string(), "en".to_string()]
    );
    assert_eq!(
        translator.format_attribute("card", "label", None, "fallback"),
        "Libellé français"
    );

    let resolved = translator
        .try_format_with_locale("hello", None)
        .expect("the French locale must resolve");
    assert_eq!(resolved.value(), "Bonjour");
    assert_eq!(resolved.locale(), "fr");
}

#[test]
fn lazy_lookup_uses_cldr_likely_script_without_loading_unrelated_locales() {
    const BUNDLES: &[(&str, &str)] = &[
        ("en", "hello = Hello\n"),
        ("zh-Hans", "hello = 你好（简体）\n"),
        ("zh-Hant", "hello = 你好（繁體）\n"),
    ];
    let messages = LazyUiMessages::new("en", BUNDLES);

    assert_eq!(
        messages.t(Some("zh-TW-u-ca-chinese"), "hello", "fallback"),
        "你好（繁體）"
    );
    assert_eq!(
        messages.loaded_locales().collect::<Vec<_>>(),
        vec!["zh-Hant"]
    );
}

#[test]
fn malformed_lazy_bundle_is_cached_diagnosed_and_skipped_for_default() {
    const BUNDLES: &[(&str, &str)] = &[
        ("en", "hello = Hello\n"),
        ("de", "this is not valid fluent"),
    ];
    let messages = LazyUiMessages::new("en", BUNDLES);

    assert!(messages.initialization_diagnostics().is_empty());
    assert_eq!(messages.t(Some("de"), "hello", "fallback"), "Hello");
    assert_eq!(messages.loaded_locales().collect::<Vec<_>>(), vec!["en"]);

    let diagnostics = messages.loaded_bundle_diagnostics().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert!(matches!(
        &diagnostics[0],
        BundleBuildError::FluentParse { locale, .. } if locale == "de"
    ));

    assert!(matches!(
        messages.validate(),
        Err(BundleBuildError::FluentParse { locale, .. }) if locale == "de"
    ));
}

#[test]
fn lazy_index_canonicalizes_aliases_and_reserves_first_identity() {
    const BUNDLES: &[(&str, &str)] = &[
        ("iw-IL", "hello = Legacy spelling\n"),
        ("he-IL", "hello = Modern spelling\n"),
        ("en", "hello = English\n"),
    ];
    let messages = LazyUiMessages::new("en", BUNDLES);

    assert_eq!(
        messages.declared_locales().collect::<Vec<_>>(),
        vec!["en", "he-IL"]
    );
    let diagnostics = messages.initialization_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert!(matches!(
        &diagnostics[0],
        BundleBuildError::DuplicateLocale { locale } if locale == "he-IL"
    ));
    assert_eq!(
        messages.t(Some("he-IL"), "hello", "fallback"),
        "Legacy spelling"
    );
}

#[test]
fn concurrent_first_use_initializes_shared_locale_once_without_races() {
    const BUNDLES: &[(&str, &str)] = &[("en", "hello = Hello\n"), ("ar", AR)];
    let messages = Arc::new(LazyUiMessages::new("en", BUNDLES));
    let mut handles = Vec::new();

    for _ in 0..16 {
        let messages = Arc::clone(&messages);
        handles.push(thread::spawn(move || {
            assert_eq!(messages.t(Some("ar"), "hello", "fallback"), "مرحبًا");
        }));
    }

    for handle in handles {
        handle.join().expect("lazy lookup thread must not panic");
    }

    assert_eq!(messages.loaded_locales().collect::<Vec<_>>(), vec!["ar"]);
}

mod lazy_macro_consumer {
    const EN: &str = "hello = Hello lazily\n";
    const JA: &str = "hello = こんにちは\n";

    rustok_ui_i18n::declare_module_i18n!(lazy, "en", &[("en", EN), ("ja", JA)]);

    pub fn loaded_count() -> usize {
        MESSAGES.loaded_locales().count()
    }
}

#[test]
fn module_macro_supports_explicit_lazy_catalogs() {
    assert_eq!(lazy_macro_consumer::loaded_count(), 0);
    assert_eq!(
        lazy_macro_consumer::t(Some("ja"), "hello", "fallback"),
        "こんにちは"
    );
    assert_eq!(lazy_macro_consumer::loaded_count(), 1);
    lazy_macro_consumer::validate().expect("the complete lazy macro catalog must validate");
    assert!(lazy_macro_consumer::initialization_diagnostics().is_empty());
}

#[test]
fn lazy_runtime_remains_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LazyUiMessages>();
}
