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
use rustok_ui_i18n::{locale_candidates, normalize_locale_tag, with_kebab_key};

proptest! {
    #[test]
    fn normalized_locale_is_idempotent(input in any::<String>()) {
        if let Some(normalized) = normalize_locale_tag(&input) {
            prop_assert!(!normalized.contains('_'));
            prop_assert!(normalized.len() <= 64);

            let normalized_again = normalize_locale_tag(&normalized);
            prop_assert_eq!(normalized_again.as_deref(), Some(normalized.as_str()));
        }
    }

    #[test]
    fn locale_candidates_are_unique_and_keep_platform_fallback(
        requested in proptest::option::of(any::<String>()),
        default_locale in any::<String>(),
    ) {
        let candidates = locale_candidates(requested.as_deref(), &default_locale);

        prop_assert!(!candidates.is_empty());
        prop_assert!(candidates.iter().any(|candidate| candidate == "en"));
        prop_assert!(candidates.iter().all(|candidate| !candidate.contains('_')));

        let unique = candidates.iter().collect::<HashSet<_>>();
        prop_assert_eq!(unique.len(), candidates.len());
    }

    #[test]
    fn kebab_conversion_matches_safe_reference(key in any::<String>()) {
        let expected = key.replace('.', "-");
        let actual = with_kebab_key(&key, str::to_owned);

        prop_assert_eq!(actual, expected);
    }
}
