/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::prelude::*;

const EN_FTL: &str = "hello = Hello\nwelcome = Welcome, { $name }!\n";
static MESSAGES: UiMessages = UiMessages::new("en", &[("en", EN_FTL)]);

#[test]
fn prelude_exposes_the_supported_high_level_consumer_surface() {
    assert_eq!(t!(MESSAGES, Some("en"), "hello", "fallback"), "Hello");

    let _args: FluentArgs<'_> = fluent_args!(name = "Ada");

    let prepared: PreparedUiMessages = MESSAGES
        .prepare()
        .expect("the public API contract fixture must build strictly");
    assert_eq!(prepared.t(Some("en"), "hello", "fallback"), "Hello");

    let translator: UiTranslator<'_> = prepared.translator();
    assert_eq!(translator.t(Some("en"), "hello", "fallback"), "Hello");

    let locale_translator: UiLocaleTranslator<'_> = prepared.for_locale(Some("en"));
    assert_eq!(locale_translator.t("hello", "fallback"), "Hello");
}
