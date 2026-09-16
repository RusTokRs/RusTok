/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use unic_langid::LanguageIdentifier;

const MAX_LOCALE_TAG_LEN: usize = 64;

/// Normalizes the admin UI effective locale to either "ru" or "en".
///
/// If `locale` is absent, invalid, or not Russian, defaults to `"en"`.
pub fn normalize_admin_locale(locale: Option<&str>) -> &'static str {
    let Some(locale) = locale.and_then(normalize_locale_tag) else {
        return "en";
    };

    if locale
        .split('-')
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("ru"))
    {
        "ru"
    } else {
        "en"
    }
}

/// Parses and normalizes a BCP 47 language identifier, replacing underscores with hyphens.
///
/// Inputs longer than 64 bytes are rejected before normalization allocates. BCP 47
/// language identifiers are ASCII, so the byte limit matches the shared Next.js
/// locale policy while keeping request-scope lookup work bounded.
pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    let trimmed = locale.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_LOCALE_TAG_LEN {
        return None;
    }

    let normalized = trimmed.replace('_', "-");
    let langid: LanguageIdentifier = normalized.parse().ok()?;
    Some(langid.to_string())
}

/// Generates a deduplicated ordered list of locale fallback candidates.
///
/// Order of precedence:
/// 1. Requested locale from most-specific to least-specific
///    (e.g. `zh-Hans-CN` -> `zh-Hans` -> `zh`)
/// 2. Default locale from most-specific to least-specific
/// 3. Canonical platform fallback (`"en"`)
pub fn locale_candidates(locale: Option<&str>, default_locale: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    push_locale_candidate(&mut candidates, locale);
    push_locale_candidate(&mut candidates, Some(default_locale));
    push_locale_candidate(&mut candidates, Some("en"));

    candidates
}

/// Pushes a normalized locale and all of its progressively less-specific
/// parents to the candidate list.
pub fn push_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(locale) = locale.and_then(normalize_locale_tag) else {
        return;
    };

    let mut current = locale.as_str();
    loop {
        push_unique(candidates, current);
        let Some((parent, _)) = current.rsplit_once('-') else {
            break;
        };
        current = parent;
    }
}

/// Appends `locale` to `candidates` only if not already present.
pub fn push_unique(candidates: &mut Vec<String>, locale: &str) {
    if !candidates.iter().any(|candidate| candidate == locale) {
        candidates.push(locale.to_string());
    }
}
