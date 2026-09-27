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
    BundleBuildError, build_fluent_bundle, locale_candidates, normalize_locale_tag,
    normalize_unicode_locale,
};

const UNICODE_EXTENSION: &str = "en-US-u-ca-gregory";
const TRANSFORM_EXTENSION: &str = "de-t-en-us-h0-hybrid";
const PRIVATE_USE_EXTENSION: &str = "de-DE-x-rustok";

#[test]
fn complete_locale_normalization_preserves_well_formed_extensions() {
    assert_eq!(
        normalize_unicode_locale(UNICODE_EXTENSION),
        Some(UNICODE_EXTENSION.to_string())
    );
    assert_eq!(
        normalize_unicode_locale(TRANSFORM_EXTENSION),
        Some(TRANSFORM_EXTENSION.to_string())
    );
    assert_eq!(
        normalize_unicode_locale(PRIVATE_USE_EXTENSION),
        Some(PRIVATE_USE_EXTENSION.to_string())
    );
}

#[test]
fn request_extensions_are_projected_to_catalog_identity_after_validation() {
    assert_eq!(
        normalize_locale_tag(UNICODE_EXTENSION),
        Some("en-US".to_string())
    );
    assert_eq!(
        normalize_locale_tag(TRANSFORM_EXTENSION),
        Some("de".to_string())
    );
    assert_eq!(
        normalize_locale_tag(PRIVATE_USE_EXTENSION),
        Some("de-DE".to_string())
    );
    assert_eq!(
        locale_candidates(Some(UNICODE_EXTENSION), "fr-CA"),
        vec!["en-US", "en", "fr-CA", "fr"]
    );
}

#[test]
fn extension_bearing_catalog_locale_remains_a_typed_error() {
    let error = match build_fluent_bundle(UNICODE_EXTENSION, "hello = Hello") {
        Err(error) => error,
        Ok(_) => panic!("formatting preferences must not become catalog identity"),
    };

    assert!(matches!(error, BundleBuildError::InvalidLocale { .. }));
}
