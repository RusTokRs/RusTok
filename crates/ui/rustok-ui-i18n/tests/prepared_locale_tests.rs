/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::messages::UiLocaleTranslator;
use rustok_ui_i18n::{fluent_args, I18nError, UiMessages};

fn assert_send_sync<T: Send + Sync>() {}

fn strip_bidi_isolates(value: &str) -> String {
    value.replace('\u{2068}', "").replace('\u{2069}', "")
}

#[test]
fn prepared_locale_translator_is_send_and_sync() {
    assert_send_sync::<UiLocaleTranslator<'static>>();
}

#[test]
fn prepared_locale_keeps_the_expected_fallback_chain() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en-GB",
        &[
            ("zh", "title = 中文\n"),
            ("zh-Hant", "title = 繁體中文\n"),
            ("en-GB", "title = English\n"),
        ],
    );

    let translator = MESSAGES.for_locale(Some("zh-Hant-TW"));
    assert_eq!(
        translator.candidates(),
        ["zh-Hant-TW", "zh-Hant", "zh", "en-GB", "en"]
    );
    assert_eq!(translator.t("title", "fallback"), "繁體中文");
}

#[test]
fn prepared_and_per_lookup_paths_are_semantically_equivalent() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            (
                "en",
                "title = English title\nwelcome = Welcome, { $name }!\nonly-default = Default only\n",
            ),
            (
                "ru",
                "title = Русский заголовок\nwelcome = Привет, { $name }!\n",
            ),
        ],
    );

    let prepared = MESSAGES.for_locale(Some("ru-RU"));
    let args = fluent_args!(name = "Иван");

    assert_eq!(
        prepared.t("title", "fallback"),
        MESSAGES.t(Some("ru-RU"), "title", "fallback")
    );
    assert_eq!(
        prepared.t("only.default", "fallback"),
        MESSAGES.t(Some("ru-RU"), "only.default", "fallback")
    );
    assert_eq!(
        strip_bidi_isolates(&prepared.format("welcome", Some(&args), "fallback")),
        strip_bidi_isolates(&MESSAGES.format(
            Some("ru-RU"),
            "welcome",
            Some(&args),
            "fallback",
        ))
    );
    assert_eq!(prepared.t("missing", "fallback"), "fallback");
}

#[test]
fn prepared_locale_strict_path_reports_actual_formatting_locale() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "welcome = Welcome, { $name }!\n"),
            ("ru", "welcome = Привет, { $name }!\n"),
        ],
    );

    let prepared = MESSAGES.for_locale(Some("ru-RU"));
    let error = prepared
        .try_format("welcome", None)
        .expect_err("missing variable must remain a strict formatting error");

    assert!(matches!(
        error,
        I18nError::FormattingFailed { ref locale, .. } if locale == "ru"
    ));
}

#[test]
fn prepared_catalog_can_prepare_a_locale_without_revalidating_bundles() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "title = English\n"),
            ("ru", "title = Русский\n"),
        ],
    );

    let catalog = MESSAGES.prepare().expect("catalog should validate");
    let ru = catalog.for_locale(Some("ru-RU"));
    let en = catalog.for_locale(Some("en-US"));

    assert_eq!(ru.t("title", "fallback"), "Русский");
    assert_eq!(en.t("title", "fallback"), "English");
}
