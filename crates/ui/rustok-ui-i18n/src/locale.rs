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

/// Normalizes the admin UI effective locale to either "ru" or "en".
///
/// If `locale` is absent or not Russian, defaults to `"en"`.
pub fn normalize_admin_locale(locale: Option<&str>) -> &'static str {
    match locale {
        Some(value)
            if value.eq_ignore_ascii_case("ru")
                || value.starts_with("ru-")
                || value.starts_with("ru_") =>
        {
            "ru"
        }
        _ => "en",
    }
}

/// Parses and normalizes a BCP 47 locale tag, replacing underscores with hyphens.
pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    let normalized = locale.trim().replace('_', "-");
    if normalized.is_empty() {
        return None;
    }

    let langid: LanguageIdentifier = normalized.parse().ok()?;
    Some(langid.to_string())
}

/// Generates a deduplicated ordered list of locale fallback candidates.
///
/// Order of precedence:
/// 1. Requested locale (e.g. `ru-RU`)
/// 2. Language base of requested locale (e.g. `ru`)
/// 3. Default locale (e.g. `en-US` and then `en`)
/// 4. Canonical platform fallback (`"en"`)
pub fn locale_candidates(locale: Option<&str>, default_locale: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    push_locale_candidate(&mut candidates, locale);
    push_locale_candidate(&mut candidates, Some(default_locale));
    push_locale_candidate(&mut candidates, Some("en"));

    candidates
}

/// Pushes normalized locale and its language-only base to the candidate list.
pub fn push_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(locale) = locale.and_then(normalize_locale_tag) else {
        return;
    };

    push_unique(candidates, locale.as_str());

    if let Some((language, _)) = locale.split_once('-') {
        push_unique(candidates, language);
    }
}

/// Appends `locale` to `candidates` only if not already present.
pub fn push_unique(candidates: &mut Vec<String>, locale: &str) {
    if !candidates.iter().any(|candidate| candidate == locale) {
        candidates.push(locale.to_string());
    }
}
