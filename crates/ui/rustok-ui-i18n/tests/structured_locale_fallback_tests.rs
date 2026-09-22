/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_i18n::{locale_candidates, normalize_locale_tag};

#[test]
fn multi_variant_fallback_drops_variant_set_instead_of_sorted_suffixes() {
    // unic_langid canonicalizes variants as an ordered set, so the normalized
    // spelling differs from the input order.
    assert_eq!(
        normalize_locale_tag("sl-rozaj-biske"),
        Some("sl-biske-rozaj".to_string())
    );

    assert_eq!(
        locale_candidates(Some("sl-rozaj-biske"), "en"),
        vec!["sl-biske-rozaj", "sl", "en"]
    );
}

#[test]
fn structural_fallback_preserves_script_region_order() {
    assert_eq!(
        locale_candidates(Some("zh-Hant-TW"), "en-GB"),
        vec!["zh-Hant-TW", "zh-Hant", "zh", "en-GB", "en"]
    );
    assert_eq!(
        locale_candidates(Some("sr-Latn-RS-revised"), "en"),
        vec!["sr-Latn-RS-revised", "sr-Latn-RS", "sr-Latn", "sr", "en"]
    );
}

#[test]
fn single_variant_fallback_remains_compatible() {
    assert_eq!(
        locale_candidates(Some("de-DE-1901"), "en-GB"),
        vec!["de-DE-1901", "de-DE", "de", "en-GB", "en"]
    );
}
