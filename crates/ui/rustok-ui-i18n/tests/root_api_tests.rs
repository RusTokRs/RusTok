/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{PreparedUiMessages, UiLocaleTranslator, UiMessages};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn prepared_runtime_types_are_available_from_crate_root() {
    assert_send_sync::<PreparedUiMessages>();
    assert_send_sync::<UiLocaleTranslator<'static>>();
}

#[test]
fn root_api_supports_prepare_then_prepare_locale_flow() {
    static MESSAGES: UiMessages = UiMessages::new(
        "en",
        &[
            ("en", "title = English\n"),
            ("ru", "title = Русский\n"),
        ],
    );

    let prepared: PreparedUiMessages = MESSAGES.prepare().expect("catalog should prepare");
    let locale: UiLocaleTranslator<'_> = prepared.for_locale(Some("ru-RU"));

    assert_eq!(locale.t("title", "fallback"), "Русский");
}
