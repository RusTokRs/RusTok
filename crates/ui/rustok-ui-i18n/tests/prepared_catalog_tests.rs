/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::messages::PreparedUiMessages;
use rustok_ui_i18n::{fluent_args, BundleBuildError, I18nError, UiMessages};

fn assert_send_sync<T: Send + Sync>() {}

fn strip_bidi_isolates(value: &str) -> String {
    value.replace('\u{2068}', "").replace('\u{2069}', "")
}

#[test]
fn prepared_catalog_is_send_and_sync() {
    assert_send_sync::<PreparedUiMessages>();
}

#[test]
fn validate_rejects_invalid_default_locale() {
    static MESSAGES: UiMessages = UiMessages::new(
        "not@a@locale",
        &[("en", "title = Title\n")],
    );

    let error = match MESSAGES.validate() {
        Ok(()) => panic!("invalid default locale must fail strict validation"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        BundleBuildError::InvalidDefaultLocale { ref locale, .. }
            if locale == "not@a@locale"
    ));
}

#[test]
fn prepare_rejects_malformed_catalog_instead_of_skipping_it() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "title = Title\n"),
            ("ru", "broken fluent resource"),
        ],
    );

    let error = match MESSAGES.prepare() {
        Ok(_) => panic!("prepared catalog must fail closed on malformed FTL"),
        Err(error) => error,
    };

    assert!(matches!(error, BundleBuildError::FluentParse { .. }));
}

#[test]
fn prepare_rejects_duplicate_normalized_locales() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en-US",
        &[
            ("en_US", "title = First\n"),
            ("en-US", "title = Second\n"),
        ],
    );

    let error = match MESSAGES.prepare() {
        Ok(_) => panic!("prepared catalog must reject duplicate normalized locales"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        BundleBuildError::DuplicateLocale { ref locale } if locale == "en-US"
    ));
}

#[test]
fn prepared_catalog_serves_from_the_validated_catalog() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "welcome = Welcome, { $name }!\nonly-en = English fallback\n"),
            ("ru", "welcome = Привет, { $name }!\n"),
        ],
    );

    let prepared = MESSAGES
        .prepare()
        .expect("valid static catalogs should prepare successfully");

    assert_eq!(prepared.default_locale(), "en");
    assert_eq!(prepared.fluent_catalog().len(), 2);

    let args = fluent_args!(name = "Иван");
    assert_eq!(
        strip_bidi_isolates(&prepared.format(
            Some("ru-RU"),
            "welcome",
            Some(&args),
            "fallback",
        )),
        "Привет, Иван!"
    );
    assert_eq!(
        prepared.t(Some("ru-RU"), "only.en", "fallback"),
        "English fallback"
    );
}

#[test]
fn prepared_try_format_preserves_strict_formatting_errors() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[("en", "welcome = Welcome, { $name }!\n")],
    );
    let prepared = MESSAGES.prepare().expect("catalog should be valid");

    let error = prepared
        .try_format(Some("en"), "welcome", None)
        .expect_err("missing required arguments must remain a strict error");

    assert!(matches!(error, I18nError::FormattingFailed { .. }));
}

#[test]
fn lenient_ui_messages_path_remains_backward_compatible() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "title = Title\n"),
            ("ru", "broken fluent resource"),
        ],
    );

    // Existing UI callers can still use the lazy lenient path: the invalid RU
    // bundle is skipped and the valid default bundle remains available.
    assert_eq!(MESSAGES.t(Some("ru"), "title", "fallback"), "Title");
}
