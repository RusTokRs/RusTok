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
    AcceptLanguagePreference, PreparedUiMessages, UiLocaleTranslator, UiMessages,
    accept_language_catalog_locales, try_parse_accept_language,
};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn accept_language_types_and_helpers_are_available_from_crate_root() {
    let preferences: Vec<AcceptLanguagePreference> =
        try_parse_accept_language("fr;q=0.7, ar;q=0.9")
            .expect("bounded preferences must parse");
    assert_eq!(preferences[0].locale(), Some("ar"));
    assert_eq!(
        accept_language_catalog_locales("iw-IL-u-ca-hebrew"),
        vec!["he-IL"]
    );
}

#[test]
fn prepared_runtime_types_are_available_from_crate_root() {
    assert_send_sync::<PreparedUiMessages>();
    assert_send_sync::<UiLocaleTranslator<'static>>();
}

#[test]
fn root_api_supports_prepare_then_prepare_locale_flow() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[("en", "title = English\n"), ("ru", "title = Русский\n")],
    );

    let prepared: PreparedUiMessages = MESSAGES.prepare().expect("catalog should prepare");
    let locale: UiLocaleTranslator<'_> = prepared.for_locale(Some("ru-RU"));

    assert_eq!(locale.t("title", "fallback"), "Русский");
}
