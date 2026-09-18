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
use rustok_ui_i18n::{locale_candidates, normalize_locale_tag};
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
    fn locale_fallback_chain_is_unique_bounded_and_canonical(
        requested in prop::option::of(bounded_text()),
        default_locale in bounded_text(),
    ) {
        let candidates = locale_candidates(requested.as_deref(), &default_locale);

        // One LanguageIdentifier can contribute at most four structural levels:
        // exact, variants-cleared, region-cleared, script-cleared. Requested and
        // default locales therefore contribute at most eight entries, plus "en".
        prop_assert!(candidates.len() <= 9);

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
