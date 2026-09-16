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
fn oversized_locale_is_rejected_before_entering_fallback_chain() {
    let oversized = format!("en-{}", "a".repeat(62));
    assert!(oversized.len() > 64);

    assert_eq!(normalize_locale_tag(&oversized), None);
    assert_eq!(
        locale_candidates(Some(&oversized), "ru"),
        vec!["ru", "en"]
    );
}

#[test]
fn surrounding_whitespace_does_not_count_against_locale_limit() {
    assert_eq!(
        normalize_locale_tag("                    ru_RU                    "),
        Some("ru-RU".to_string())
    );
}
