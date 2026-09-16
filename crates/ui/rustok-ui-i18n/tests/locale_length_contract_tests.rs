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
    build_fluent_bundle, build_fluent_catalog, normalize_locale_tag, try_build_fluent_catalog,
    BundleBuildError, UiMessages,
};

// Syntactically LanguageIdentifier-shaped, but deliberately above the shared
// 64-byte runtime input bound.
const OVERSIZED_LOCALE: &str =
    "en-abcde-fghij-klmno-pqrst-uvwxy-zabcd-efghi-jklmn-opqrs-tuvwx-yzabc";
const FTL: &str = "title = Title\n";

#[test]
fn oversized_locale_is_rejected_consistently_by_lookup_and_strict_builders() {
    assert!(OVERSIZED_LOCALE.len() > 64);
    assert_eq!(normalize_locale_tag(OVERSIZED_LOCALE), None);

    let bundle_error = build_fluent_bundle(OVERSIZED_LOCALE, FTL)
        .expect_err("bundle construction must share the runtime locale length bound");
    assert!(matches!(
        bundle_error,
        BundleBuildError::LocaleTooLong { max_len: 64, .. }
    ));

    let catalog_error = try_build_fluent_catalog(&[(OVERSIZED_LOCALE, FTL)])
        .expect_err("strict catalogs must not contain locales runtime lookup rejects");
    assert!(matches!(
        catalog_error,
        BundleBuildError::LocaleTooLong { max_len: 64, .. }
    ));
}

#[test]
fn lenient_catalog_skips_oversized_unreachable_locale() {
    let catalog = build_fluent_catalog(&[(OVERSIZED_LOCALE, FTL), ("en", FTL)]);

    assert_eq!(catalog.len(), 1);
    assert!(catalog.contains_key("en"));
    assert!(!catalog.contains_key(OVERSIZED_LOCALE));
}

#[test]
fn prepared_runtime_cannot_validate_an_unreachable_default_catalog() {
    static MESSAGES: UiMessages =
        UiMessages::new(OVERSIZED_LOCALE, &[(OVERSIZED_LOCALE, FTL), ("en", FTL)]);

    let error = MESSAGES
        .prepare()
        .expect_err("prepared runtime must fail closed before caching an unreachable default");
    assert!(matches!(
        error,
        BundleBuildError::LocaleTooLong { max_len: 64, .. }
    ));
}
