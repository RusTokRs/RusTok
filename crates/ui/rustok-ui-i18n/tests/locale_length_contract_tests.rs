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

    let bundle_error = match build_fluent_bundle(OVERSIZED_LOCALE, FTL) {
        Ok(_) => panic!("bundle construction must share the runtime locale length bound"),
        Err(error) => error,
    };
    match &bundle_error {
        BundleBuildError::LocaleTooLong { length, max_len } => {
            assert_eq!(*length, OVERSIZED_LOCALE.len());
            assert_eq!(*max_len, 64);
        }
        other => panic!("expected LocaleTooLong, got {other:?}"),
    }
    assert!(
        !bundle_error.to_string().contains(OVERSIZED_LOCALE),
        "oversized untrusted locale payload must not be retained in diagnostics"
    );

    let catalog_error = match try_build_fluent_catalog(&[(OVERSIZED_LOCALE, FTL)]) {
        Ok(_) => panic!("strict catalogs must not contain locales runtime lookup rejects"),
        Err(error) => error,
    };
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

    let error = match MESSAGES.prepare() {
        Ok(_) => panic!("prepared runtime must fail closed before caching an unreachable default"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        BundleBuildError::LocaleTooLong { max_len: 64, .. }
    ));
}
