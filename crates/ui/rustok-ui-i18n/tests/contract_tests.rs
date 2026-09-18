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

use rustok_ui_i18n::{
    build_fluent_bundle, fluent_args, locale_candidates, normalize_admin_locale,
    normalize_locale_tag, BundleBuildError, UiMessages,
};

fn assert_send_sync<T: Send + Sync>() {}

fn without_bidi_isolates(value: &str) -> String {
    value.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn ui_messages_implements_send_and_sync() {
    assert_send_sync::<UiMessages>();
}

#[test]
fn locale_candidates_fallback_order_matches_specification() {
    // 1. Regional locale falls back to language base, then default, then "en"
    let ru_ru = locale_candidates(Some("ru-RU"), "en");
    assert_eq!(ru_ru, vec!["ru-RU", "ru", "en"]);

    let ru_by = locale_candidates(Some("ru-BY"), "en");
    assert_eq!(ru_by, vec!["ru-BY", "ru", "en"]);

    let es_mx = locale_candidates(Some("es-MX"), "en");
    assert_eq!(es_mx, vec!["es-MX", "es", "en"]);

    // 2. Base language locale
    let ru = locale_candidates(Some("ru"), "en");
    assert_eq!(ru, vec!["ru", "en"]);

    // 3. Default locale and English
    let en_us = locale_candidates(Some("en-US"), "en");
    assert_eq!(en_us, vec!["en-US", "en"]);

    let en = locale_candidates(Some("en"), "en");
    assert_eq!(en, vec!["en"]);

    // 4. None falls back to default locale and "en"
    let none = locale_candidates(None, "en");
    assert_eq!(none, vec!["en"]);

    // 5. Underscore-separated locale normalized
    let ru_under = locale_candidates(Some("ru_RU"), "en");
    assert_eq!(ru_under, vec!["ru-RU", "ru", "en"]);
}

#[test]
fn normalize_admin_locale_contract() {
    assert_eq!(normalize_admin_locale(Some("ru")), "ru");
    assert_eq!(normalize_admin_locale(Some("RU")), "ru");
    assert_eq!(normalize_admin_locale(Some("ru-RU")), "ru");
    assert_eq!(normalize_admin_locale(Some("ru_RU")), "ru");
    assert_eq!(normalize_admin_locale(Some("ru-BY")), "ru");
    assert_eq!(normalize_admin_locale(Some("en")), "en");
    assert_eq!(normalize_admin_locale(Some("en-US")), "en");
    assert_eq!(normalize_admin_locale(Some("es")), "en");
    assert_eq!(normalize_admin_locale(Some("de")), "en");
    assert_eq!(normalize_admin_locale(None), "en");
}

#[test]
fn normalize_locale_tag_contract() {
    assert_eq!(normalize_locale_tag("en"), Some("en".to_string()));
    assert_eq!(normalize_locale_tag("ru_RU"), Some("ru-RU".to_string()));
    assert_eq!(normalize_locale_tag("es-ES"), Some("es-ES".to_string()));
    assert_eq!(normalize_locale_tag("zh_Hans_CN"), Some("zh-Hans-CN".to_string()));
    assert_eq!(normalize_locale_tag("   "), None);
    assert_eq!(normalize_locale_tag(""), None);
    assert_eq!(normalize_locale_tag("invalid!tag!"), None);
}

#[test]
fn russian_pluralization_cardinal_matrix() {
    const FTL_RU: &str = r#"
items = { $count ->
    [one] { $count } элемент
    [few] { $count } элемента
   *[other] { $count } элементов
}
"#;
    static MESSAGES: UiMessages = UiMessages::new("ru", &[("ru", FTL_RU)]);

    let cases: &[(i64, &str)] = &[
        (1, "1 элемент"),
        (2, "2 элемента"),
        (3, "3 элемента"),
        (4, "4 элемента"),
        (5, "5 элементов"),
        (6, "6 элементов"),
        (10, "10 элементов"),
        (11, "11 элементов"),
        (12, "12 элементов"),
        (14, "14 элементов"),
        (19, "19 элементов"),
        (20, "20 элементов"),
        (21, "21 элемент"),
        (22, "22 элемента"),
        (24, "24 элемента"),
        (25, "25 элементов"),
        (101, "101 элемент"),
        (102, "102 элемента"),
        (105, "105 элементов"),
        (111, "111 элементов"),
    ];

    for &(count, expected) in cases {
        let args = fluent_args!(count = count);
        let actual = MESSAGES.format(Some("ru"), "items", Some(&args), "fallback");
        assert_eq!(
            without_bidi_isolates(&actual),
            expected,
            "Failed for Russian count {count}: expected '{expected}', got '{actual}'"
        );
    }
}

#[test]
fn english_pluralization_matrix() {
    const FTL_EN: &str = r#"
items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
"#;
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", FTL_EN)]);

    let cases: &[(i64, &str)] = &[
        (0, "0 items"),
        (1, "1 item"),
        (2, "2 items"),
        (5, "5 items"),
        (21, "21 items"),
    ];

    for &(count, expected) in cases {
        let args = fluent_args!(count = count);
        let actual = MESSAGES.format(Some("en"), "items", Some(&args), "fallback");
        assert_eq!(
            without_bidi_isolates(&actual),
            expected,
            "Failed for English count {count}: expected '{expected}', got '{actual}'"
        );
    }
}

#[test]
fn multi_bundle_fallback_chain() {
    const FTL_RU_RU: &str = "action-save = Сохранить (RU-RU)\n";
    const FTL_RU: &str = "action-save = Сохранить\naction-cancel = Отмена\n";
    const FTL_EN: &str = "action-save = Save\naction-cancel = Cancel\naction-delete = Delete\n";

    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("ru-RU", FTL_RU_RU),
            ("ru", FTL_RU),
            ("en", FTL_EN),
        ],
    );

    // 1. Regional override resolves to regional bundle
    assert_eq!(
        MESSAGES.t(Some("ru-RU"), "action.save", "fallback"),
        "Сохранить (RU-RU)"
    );

    // 2. Regional missing key falls back to base language bundle
    assert_eq!(
        MESSAGES.t(Some("ru-RU"), "action.cancel", "fallback"),
        "Отмена"
    );

    // 3. Regional + base missing key falls back to default English bundle
    assert_eq!(
        MESSAGES.t(Some("ru-RU"), "action.delete", "fallback"),
        "Delete"
    );

    // 4. Missing key everywhere falls back to explicit fallback parameter
    assert_eq!(
        MESSAGES.t(Some("ru-RU"), "action.nonexistent", "DefaultFallback"),
        "DefaultFallback"
    );
}

#[test]
fn formatted_arguments_are_wrapped_with_bidi_isolates() {
    const FTL: &str = "greeting = مرحبًا، { $name }!\n";
    static MESSAGES: UiMessages = UiMessages::new("ar", &[("ar", FTL)]);

    let args = fluent_args!(name = "Alice");
    let result = MESSAGES.format(Some("ar"), "greeting", Some(&args), "fallback");

    assert!(
        result.contains("\u{2068}Alice\u{2069}"),
        "Interpolated LTR text inside an RTL message must be directionally isolated: {result:?}"
    );
    assert_eq!(without_bidi_isolates(&result), "مرحبًا، Alice!");
}

#[test]
fn typed_bundle_build_errors() {
    // 1. Invalid locale tag
    let err = match build_fluent_bundle("not@a@valid@locale", "key = val") {
        Err(e) => e,
        Ok(_) => panic!("Expected error for invalid locale"),
    };
    match err {
        BundleBuildError::InvalidLocale { locale, .. } => {
            assert_eq!(locale, "not@a@valid@locale");
        }
        _ => panic!("Expected InvalidLocale error"),
    }

    // 2. Syntax error in FTL
    let err = match build_fluent_bundle("en", "broken key without equals") {
        Err(e) => e,
        Ok(_) => panic!("Expected error for broken syntax"),
    };
    match err {
        BundleBuildError::FluentParse { locale, errors } => {
            assert_eq!(locale, "en");
            assert!(!errors.is_empty());
        }
        _ => panic!("Expected FluentParse error"),
    }
}

#[test]
fn concurrent_multithreaded_message_resolution() {
    const FTL_EN: &str = "greet = Hello, { $user }!\ncount = Count: { $n }\n";
    const FTL_RU: &str = "greet = Привет, { $user }!\ncount = Число: { $n }\n";

    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", FTL_EN), ("ru", FTL_RU)]);
    let messages = Arc::new(&MESSAGES);

    let mut handles = Vec::new();
    for thread_idx in 0..10 {
        let msg = Arc::clone(&messages);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let user = format!("user_{thread_idx}_{i}");
                let args = fluent_args!(user = user.as_str());
                if i % 2 == 0 {
                    let res = msg.format(Some("ru"), "greet", Some(&args), "fallback");
                    assert_eq!(without_bidi_isolates(&res), format!("Привет, {user}!"));
                } else {
                    let res = msg.format(Some("en"), "greet", Some(&args), "fallback");
                    assert_eq!(without_bidi_isolates(&res), format!("Hello, {user}!"));
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
