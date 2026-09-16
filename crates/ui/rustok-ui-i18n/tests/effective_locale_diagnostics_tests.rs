/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{I18nError, UiMessages};

#[test]
fn prepared_and_per_lookup_missing_errors_report_same_canonical_effective_locale() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "title = English\n"),
            ("ru", "title = Русский\n"),
        ],
    );

    let prepared = MESSAGES.for_locale(Some("ru_RU"));
    let prepared_error = prepared
        .try_resolve("missing")
        .expect_err("missing prepared lookup must return a typed error");
    let direct_error = MESSAGES
        .try_format(Some("ru_RU"), "missing", None)
        .expect_err("missing direct lookup must return a typed error");

    for error in [prepared_error, direct_error] {
        assert!(matches!(
            error,
            I18nError::MessageNotFound { ref locale, ref key }
                if locale == "ru-RU" && key == "missing"
        ));
    }
}

#[test]
fn invalid_requested_locale_reports_the_effective_default_locale() {
    static MESSAGES: UiMessages = UiMessages::new("en", &[("en", "title = English\n")]);

    let error = MESSAGES
        .for_locale(Some("not@a@locale"))
        .try_resolve("missing")
        .expect_err("missing lookup must report the effective locale");

    assert!(matches!(
        error,
        I18nError::MessageNotFound { ref locale, .. } if locale == "en"
    ));
}
