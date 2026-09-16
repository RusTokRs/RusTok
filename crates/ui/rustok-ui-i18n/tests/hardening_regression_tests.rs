/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{
    build_fluent_catalog, fluent_args, locale_candidates, normalize_locale_tag,
    try_build_fluent_catalog, BundleBuildError, I18nError, UiMessages, UiTranslator,
};

fn strip_bidi_isolates(value: &str) -> String {
    value.replace('\u{2068}', "").replace('\u{2069}', "")
}

#[test]
fn fallback_chain_preserves_script_region_and_variant_levels() {
    assert_eq!(
        locale_candidates(Some("zh-Hant-TW"), "en"),
        vec!["zh-Hant-TW", "zh-Hant", "zh", "en"]
    );

    assert_eq!(
        locale_candidates(Some("de-DE-1901"), "en-GB"),
        vec!["de-DE-1901", "de-DE", "de", "en-GB", "en"]
    );
}

#[test]
fn locale_contract_is_language_identifier_not_extension_preserving_locale() {
    assert_eq!(normalize_locale_tag("zh_hant_tw"), Some("zh-Hant-TW".to_string()));
    assert_eq!(normalize_locale_tag("de-DE-1901"), Some("de-DE-1901".to_string()));

    // `unic_langid::LanguageIdentifier` intentionally models language/script/
    // region/variants rather than a full extension-preserving BCP-47 locale.
    assert_eq!(normalize_locale_tag("en-US-u-ca-gregory"), None);
}

#[test]
fn strict_catalog_rejects_duplicate_normalized_locales() {
    let error = match try_build_fluent_catalog(&[
        ("en_US", "title = First\n"),
        ("en-US", "title = Second\n"),
    ]) {
        Ok(_) => panic!("normalized duplicate locales must fail closed"),
        Err(error) => error,
    };

    match error {
        BundleBuildError::DuplicateLocale { locale } => assert_eq!(locale, "en-US"),
        other => panic!("expected DuplicateLocale, got {other:?}"),
    }
}

#[test]
fn lenient_catalog_keeps_first_duplicate_locale() {
    let catalog = build_fluent_catalog(&[
        ("en_US", "title = First\n"),
        ("en-US", "title = Second\n"),
    ]);
    let translator = UiTranslator::new(&catalog, "en-US");

    assert_eq!(translator.resolve(Some("en-US"), "title").as_deref(), Some("First"));
}

#[test]
fn strict_catalog_rejects_malformed_ftl() {
    let error = match try_build_fluent_catalog(&[("en", "this is not valid fluent")]) {
        Ok(_) => panic!("malformed FTL must fail strict construction"),
        Err(error) => error,
    };

    assert!(matches!(error, BundleBuildError::FluentParse { .. }));
}

#[test]
fn ui_messages_validate_fails_closed_on_invalid_embedded_catalog() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "title = Valid\n"),
            ("ru", "broken fluent resource"),
        ],
    );

    assert!(matches!(MESSAGES.validate(), Err(BundleBuildError::FluentParse { .. })));
}

#[test]
fn strict_format_reports_missing_variable_as_formatting_error() {
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", "welcome = Welcome, { $name }!\n")]);

    let error = MESSAGES
        .try_format(Some("en"), "welcome", None)
        .expect_err("missing Fluent variables must not return partial output in strict mode");

    match error {
        I18nError::FormattingFailed { locale, key, errors } => {
            assert_eq!(locale, "en");
            assert_eq!(key, "welcome");
            assert!(!errors.is_empty());
        }
        other => panic!("expected FormattingFailed, got {other:?}"),
    }
}

#[test]
fn lenient_format_uses_literal_fallback_instead_of_partial_output() {
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", "welcome = Welcome, { $name }!\n")]);

    assert_eq!(
        MESSAGES.format(Some("en"), "welcome", None, "Safe fallback"),
        "Safe fallback"
    );
}

#[test]
fn strict_resolution_reports_missing_key() {
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", "title = Title\n")]);

    let error = MESSAGES
        .try_format(Some("en-US"), "missing.key", None)
        .expect_err("strict lookup must surface missing messages");

    assert!(matches!(
        error,
        I18nError::MessageNotFound { ref key, .. } if key == "missing.key"
    ));
}

#[test]
fn module_macros_work_from_external_integration_test_context() {
    mod consumer {
        const EN: &str = "hello = Hello, { $name }!\n";
        const RU: &str = "hello = Привет, { $name }!\n";

        rustok_ui_i18n::declare_module_i18n!("en", &[("en", EN), ("ru", RU)]);

        pub fn translated(locale: Option<&str>, name: &str) -> String {
            rustok_ui_i18n::module_t!(locale, "hello", "fallback", name = name)
        }
    }

    assert_eq!(
        strip_bidi_isolates(&consumer::translated(Some("ru"), "Иван")),
        "Привет, Иван!"
    );
    assert_eq!(
        strip_bidi_isolates(&consumer::translated(Some("en"), "Alice")),
        "Hello, Alice!"
    );
}

#[test]
fn fluent_args_string_and_identifier_forms_remain_equivalent() {
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", "value = { $count }\n")]);

    let identifier = fluent_args!(count = 7);
    let string_key = fluent_args!("count" => 7);

    assert_eq!(
        strip_bidi_isolates(&MESSAGES.format(Some("en"), "value", Some(&identifier), "fallback")),
        "7"
    );
    assert_eq!(
        strip_bidi_isolates(&MESSAGES.format(Some("en"), "value", Some(&string_key), "fallback")),
        "7"
    );
}
