/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::HashSet;

use proptest::prelude::*;
use rustok_ui_i18n::{
    MAX_ACCEPT_LANGUAGE_LEN, MAX_ACCEPT_LANGUAGE_RANGES, locale_candidates, normalize_locale_tag,
    normalize_unicode_locale, parse_accept_language,
};
use unic_langid::LanguageIdentifier;

fn bounded_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..=96)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn normalized_locale_is_canonical_parseable_and_idempotent(raw in bounded_text()) {
        let normalized = normalize_locale_tag(&raw);

        if raw.len() > 64 {
            prop_assert_eq!(normalized, None);
            return Ok(());
        }

        if let Some(normalized) = normalized {
            prop_assert!(!normalized.contains('_'));
            prop_assert!(normalized.parse::<LanguageIdentifier>().is_ok());
            prop_assert_eq!(
                normalize_locale_tag(&normalized),
                Some(normalized.clone())
            );
        }
    }

    #[test]
    fn accept_language_parsing_is_bounded_sorted_and_canonical(raw in bounded_text()) {
        let preferences = parse_accept_language(&raw);

        if raw.len() > MAX_ACCEPT_LANGUAGE_LEN
            || raw.split(',').count() > MAX_ACCEPT_LANGUAGE_RANGES
        {
            prop_assert!(preferences.is_empty());
            return Ok(());
        }

        prop_assert!(preferences.len() <= MAX_ACCEPT_LANGUAGE_RANGES);
        for pair in preferences.windows(2) {
            prop_assert!(pair[0].quality_thousandths() >= pair[1].quality_thousandths());
        }
        for preference in preferences {
            prop_assert!(preference.quality_thousandths() <= 1_000);
            if let Some(locale) = preference.locale() {
                prop_assert_eq!(
                    normalize_unicode_locale(locale),
                    Some(locale.to_string())
                );
            }
        }
    }

    #[test]
    fn locale_fallback_chain_is_unique_bounded_and_canonical(
        requested in prop::option::of(bounded_text()),
        default_locale in bounded_text(),
    ) {
        let candidates = locale_candidates(requested.as_deref(), &default_locale);

        // One request can contribute at most five levels: exact,
        // variants-cleared, two CLDR inferred-script levels, and the base
        // language. Requested/default chains therefore contribute at most ten
        // entries, plus the canonical platform fallback.
        prop_assert!(candidates.len() <= 11);

        let unique = candidates.iter().collect::<HashSet<_>>();
        prop_assert_eq!(unique.len(), candidates.len());

        for candidate in &candidates {
            prop_assert!(candidate.parse::<LanguageIdentifier>().is_ok());
            prop_assert_eq!(
                normalize_locale_tag(candidate),
                Some(candidate.clone())
            );
        }

        prop_assert!(candidates.iter().any(|candidate| candidate == "en"));
    }
}
