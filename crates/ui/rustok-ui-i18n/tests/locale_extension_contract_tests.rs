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
    build_fluent_bundle, locale_candidates, normalize_locale_tag, BundleBuildError,
};

const UNICODE_EXTENSION: &str = "en-US-u-ca-gregory";
const PRIVATE_USE_EXTENSION: &str = "de-DE-x-rustok";

#[test]
fn rust_locale_identity_rejects_extension_bearing_tags() {
    assert_eq!(normalize_locale_tag(UNICODE_EXTENSION), None);
    assert_eq!(normalize_locale_tag(PRIVATE_USE_EXTENSION), None);
}

#[test]
fn extension_bearing_requested_locale_is_not_silently_stripped() {
    assert_eq!(
        locale_candidates(Some(UNICODE_EXTENSION), "fr-CA"),
        vec!["fr-CA", "fr", "en"]
    );
}

#[test]
fn extension_bearing_catalog_locale_is_a_typed_error() {
    let error = build_fluent_bundle(UNICODE_EXTENSION, "hello = Hello")
        .expect_err("extension-bearing locale must not become Rust catalog identity");

    assert!(matches!(error, BundleBuildError::InvalidLocale { .. }));
}
