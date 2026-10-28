/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{UiMessages, fluent_args};

fn strip_bidi_isolates(value: &str) -> String {
    value.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn catalogs_are_not_limited_to_english_and_russian() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "hello = Hello\n"),
            ("ar", "hello = مرحبًا\n"),
            ("he", "hello = שלום\n"),
            ("fa", "hello = درود\n"),
            ("ur", "hello = سلام\n"),
            ("hi", "hello = नमस्ते\n"),
            ("bn", "hello = নমস্কার\n"),
            ("ta", "hello = வணக்கம்\n"),
            ("th", "hello = สวัสดี\n"),
            ("ja", "hello = こんにちは\n"),
            ("ko", "hello = 안녕하세요\n"),
            ("zh-Hant", "hello = 你好\n"),
            ("es-419", "hello = Hola\n"),
            ("fil", "hello = Kumusta\n"),
            ("sw", "hello = Habari\n"),
        ],
    );

    let prepared = MESSAGES
        .prepare()
        .expect("representative world-language catalog must validate");

    assert_eq!(prepared.available_locales().count(), 15);
    assert_eq!(prepared.t(Some("ar-EG"), "hello", "fallback"), "مرحبًا");
    assert_eq!(prepared.t(Some("hi-IN"), "hello", "fallback"), "नमस्ते");
    assert_eq!(prepared.t(Some("es-MX"), "hello", "fallback"), "Hello");
    assert_eq!(prepared.t(Some("es-419"), "hello", "fallback"), "Hola");
    assert_eq!(prepared.t(Some("fil-PH"), "hello", "fallback"), "Kumusta");

    // Deprecated `iw` canonicalizes to the modern Hebrew identity.
    assert_eq!(prepared.t(Some("iw-IL"), "hello", "fallback"), "שלום");
    // Formatting extensions remain available to host formatters but do not
    // prevent selection of the matching message catalog.
    assert_eq!(
        prepared.t(Some("ar-EG-u-nu-arab"), "hello", "fallback"),
        "مرحبًا"
    );
    // CLDR likely-script inference maps a region-only request to zh-Hant.
    assert_eq!(
        prepared.t(Some("zh-TW-u-ca-chinese"), "hello", "fallback"),
        "你好"
    );
}

#[test]
fn arabic_six_category_plural_rules_are_available() {
    const AR: &str = r#"
items = { $count ->
    [zero] صفر
    [one] واحد
    [two] اثنان
    [few] قليل
    [many] كثير
   *[other] آخر
}
"#;
    static MESSAGES: UiMessages = UiMessages::new("ar", &[("ar", AR)]);

    for (count, expected) in [
        (0, "صفر"),
        (1, "واحد"),
        (2, "اثنان"),
        (3, "قليل"),
        (11, "كثير"),
        (100, "آخر"),
    ] {
        let args = fluent_args!(count = count);
        assert_eq!(
            strip_bidi_isolates(&MESSAGES.format(Some("ar"), "items", Some(&args), "fallback")),
            expected
        );
    }
}

#[test]
fn languages_without_cardinal_inflection_use_other() {
    const JA: &str = r#"
items = { $count ->
   *[other] { $count } 個
}
"#;
    static MESSAGES: UiMessages = UiMessages::new("ja", &[("ja", JA)]);

    for count in [0, 1, 2, 42] {
        let args = fluent_args!(count = count);
        assert_eq!(
            strip_bidi_isolates(&MESSAGES.format(Some("ja"), "items", Some(&args), "fallback")),
            format!("{count} 個")
        );
    }
}
