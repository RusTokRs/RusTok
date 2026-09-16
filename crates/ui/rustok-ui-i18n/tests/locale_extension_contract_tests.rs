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
    locale_candidates, normalize_admin_locale, normalize_locale_tag, try_build_fluent_catalog,
    BundleBuildError, UiTranslator,
};

#[test]
fn effective_unicode_extension_falls_back_through_base_language_identifier() {
    assert_eq!(
        locale_candidates(Some("zh-Hant-TW-u-nu-hanidec"), "en-GB"),
        vec!["zh-Hant-TW", "zh-Hant", "zh", "en-GB", "en"]
    );
}

#[test]
fn effective_private_use_extension_does_not_change_catalog_identity() {
    assert_eq!(
        locale_candidates(Some("de-DE-x-phonebk"), "en"),
        vec!["de-DE", "de", "en"]
    );
}

#[test]
fn malformed_effective_extension_is_rejected_instead_of_partially_stripped() {
    assert_eq!(
        locale_candidates(Some("de-DE-u"), "en-GB"),
        vec!["en-GB", "en"]
    );
}

#[test]
fn admin_locale_accepts_valid_extensions_but_uses_only_base_language() {
    assert_eq!(normalize_admin_locale(Some("ru-RU-u-nu-latn")), "ru");
    assert_eq!(normalize_admin_locale(Some("en-US-u-nu-latn")), "en");
}

#[test]
fn catalog_normalizer_remains_language_identifier_only() {
    assert_eq!(normalize_locale_tag("en-US-u-nu-latn"), None);
    assert_eq!(normalize_locale_tag("de-DE-x-phonebk"), None);
}

#[test]
fn strict_catalog_rejects_extension_bearing_catalog_keys() {
    let error = match try_build_fluent_catalog(&[("en-US-u-nu-latn", "title = Title\n")]) {
        Ok(_) => panic!("extension-bearing catalog identity must remain unsupported"),
        Err(error) => error,
    };

    assert!(matches!(error, BundleBuildError::InvalidLocale { .. }));
}

#[test]
fn translator_resolves_extension_bearing_effective_locale_against_base_catalog() {
    let catalog = try_build_fluent_catalog(&[
        ("de-DE", "title = Deutsch\n"),
        ("en", "title = English\n"),
    ])
    .expect("base catalogs should build");
    let translator = UiTranslator::new(&catalog, "en");

    assert_eq!(
        translator.t(Some("de-DE-u-nu-latn"), "title", "fallback"),
        "Deutsch"
    );
}
