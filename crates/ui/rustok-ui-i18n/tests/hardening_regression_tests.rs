/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::bundle::{build_fluent_bundle, try_build_fluent_catalog};
use rustok_ui_i18n::{
    declare_module_i18n, locale_candidates, module_t, normalize_admin_locale, BundleBuildError,
    I18nError, UiMessages,
};

#[test]
fn admin_locale_normalization_is_case_insensitive_for_regional_tags() {
    assert_eq!(normalize_admin_locale(Some("RU-RU")), "ru");
    assert_eq!(normalize_admin_locale(Some("RU_RU")), "ru");
    assert_eq!(normalize_admin_locale(Some("  ru-BY  ")), "ru");
    assert_eq!(normalize_admin_locale(Some("EN-US")), "en");
}

#[test]
fn locale_candidates_preserve_intermediate_specificity_levels() {
    assert_eq!(
        locale_candidates(Some("zh-Hans-CN"), "en-US"),
        vec!["zh-Hans-CN", "zh-Hans", "zh", "en-US", "en"]
    );
    assert_eq!(
        locale_candidates(Some("sr_Latn_RS"), "en"),
        vec!["sr-Latn-RS", "sr-Latn", "sr", "en"]
    );
}

#[test]
fn bundle_builder_accepts_normalized_underscore_tags() {
    let bundle = build_fluent_bundle("ru_RU", "key = Значение\n").unwrap();
    assert!(bundle.has_message("key"));
}

#[test]
fn strict_catalog_builder_rejects_invalid_resources_and_duplicates() {
    let invalid = try_build_fluent_catalog(&[("en", "broken message")]).unwrap_err();
    assert!(matches!(invalid, BundleBuildError::FluentParse { .. }));

    let duplicate = try_build_fluent_catalog(&[
        ("ru_RU", "key = one\n"),
        ("ru-RU", "key = two\n"),
    ])
    .unwrap_err();
    assert!(matches!(
        duplicate,
        BundleBuildError::DuplicateLocale { ref locale } if locale == "ru-RU"
    ));
}

#[test]
fn ui_messages_validation_fails_closed() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[("en", "key = one\n"), ("EN", "key = duplicate\n")],
    );

    assert!(matches!(
        MESSAGES.validate(),
        Err(BundleBuildError::DuplicateLocale { .. })
    ));
}

#[test]
fn strict_formatting_surfaces_fluent_errors_and_lenient_api_uses_fallback() {
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", "welcome = Hello, { $name }!\n")]);

    assert!(matches!(
        MESSAGES.try_format(Some("en"), "welcome", None),
        Err(I18nError::FormattingFailed { .. })
    ));
    assert_eq!(
        MESSAGES.format(Some("en"), "welcome", None, "fallback"),
        "fallback"
    );
}

mod module_macro_contract {
    const EN_FTL: &str = "title = Title\ngreet = Hello, { $name }!\n";
    const RU_FTL: &str = "title = Заголовок\ngreet = Привет, { $name }!\n";

    declare_module_i18n!("en", &[("en", EN_FTL), ("ru", RU_FTL)]);

    #[test]
    fn module_t_resolves_against_declared_module_catalog() {
        assert_eq!(module_t!(Some("ru"), "title", "fallback"), "Заголовок");
        assert_eq!(
            module_t!(Some("ru"), "greet", "fallback", name = "Иван"),
            "Привет, Иван!"
        );
        assert_eq!(
            module_t!(Some("en"), "greet", "fallback", "name" => "Alice"),
            "Hello, Alice!"
        );
    }
}
