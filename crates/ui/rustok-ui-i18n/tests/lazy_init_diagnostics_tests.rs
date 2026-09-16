/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{BundleBuildError, UiMessages};

const EN: &str = "title = Title\n";
const RU_FIRST: &str = "title = Первый\n";
const RU_DUPLICATE: &str = "title = Второй\n";
const BROKEN_DE: &str = "this is not valid fluent";

static MESSAGES: UiMessages = UiMessages::new(
    "en",
    &[
        ("en", EN),
        ("ru_RU", RU_FIRST),
        ("ru-RU", RU_DUPLICATE),
        ("!", EN),
        ("de", BROKEN_DE),
    ],
);

#[test]
fn lazy_initialization_retains_diagnostics_without_rebuilding_catalog() {
    let first_diagnostics = MESSAGES.initialization_diagnostics();
    assert_eq!(first_diagnostics.len(), 3);
    assert!(matches!(
        &first_diagnostics[0],
        BundleBuildError::DuplicateLocale { locale } if locale == "ru-RU"
    ));
    assert!(matches!(
        &first_diagnostics[1],
        BundleBuildError::InvalidLocale { locale, .. } if locale == "!"
    ));
    assert!(matches!(
        &first_diagnostics[2],
        BundleBuildError::FluentParse { locale, .. } if locale == "de"
    ));

    let diagnostics_ptr = first_diagnostics.as_ptr();
    let catalog = MESSAGES.fluent_catalog();
    let catalog_ptr = catalog as *const _;

    assert_eq!(catalog.len(), 2);
    assert!(catalog.contains_key("en"));
    assert!(catalog.contains_key("ru-RU"));
    assert_eq!(MESSAGES.t(Some("en"), "title", "fallback"), "Title");
    assert_eq!(MESSAGES.t(Some("ru-RU"), "title", "fallback"), "Первый");

    assert_eq!(MESSAGES.initialization_diagnostics().as_ptr(), diagnostics_ptr);
    assert_eq!(MESSAGES.fluent_catalog() as *const _, catalog_ptr);
}

#[test]
fn ui_messages_remains_send_and_sync_with_cached_diagnostics() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<UiMessages>();
}
