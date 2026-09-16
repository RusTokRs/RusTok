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

pub(crate) const MAX_LOCALE_TAG_LEN: usize = 64;

fn parse_locale_tag(locale: &str) -> Option<LanguageIdentifier> {
    if locale.is_empty() || locale.len() > MAX_LOCALE_TAG_LEN {
        return None;
    }

    let trimmed = locale.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace('_', "-");
    normalized.parse().ok()
}

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
/// Raw inputs longer than 64 bytes are rejected before trimming or normalization work.
/// BCP 47 language identifiers are ASCII, so the byte limit matches the shared Next.js
/// locale policy for valid tags while keeping request-scope lookup work bounded.
pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    parse_locale_tag(locale).map(|langid| langid.to_string())
}

/// Generates a deduplicated ordered list of locale fallback candidates.
///
/// Order of precedence:
/// 1. Requested locale from most-specific to least-specific
///    (e.g. `zh-Hans-CN` -> `zh-Hans` -> `zh`)
/// 2. Default locale from most-specific to least-specific
/// 3. Canonical platform fallback (`"en"`)
///
/// Variant subtags are treated as one unordered specificity layer. `unic_langid`
/// canonicalizes variants as an ordered set, so peeling the serialized tag one
/// hyphen at a time can manufacture an arbitrary partial-variant parent. The
/// fallback therefore removes all variants together before region and script.
pub fn locale_candidates(locale: Option<&str>, default_locale: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    push_locale_candidate(&mut candidates, locale);
    push_locale_candidate(&mut candidates, Some(default_locale));
    push_locale_candidate(&mut candidates, Some("en"));

    candidates
}

/// Pushes a normalized locale and its progressively less-specific structural
/// parents to the candidate list.
pub fn push_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(mut langid) = locale.and_then(parse_locale_tag) else {
        return;
    };

    push_langid_candidate(candidates, &langid);

    if langid.variants().next().is_some() {
        langid.clear_variants();
        push_langid_candidate(candidates, &langid);
    }

    if langid.region.is_some() {
        langid.region = None;
        push_langid_candidate(candidates, &langid);
    }

    if langid.script.is_some() {
        langid.script = None;
        push_langid_candidate(candidates, &langid);
    }
}

fn push_langid_candidate(candidates: &mut Vec<String>, langid: &LanguageIdentifier) {
    let locale = langid.to_string();
    push_unique(candidates, &locale);
}

/// Appends `locale` to `candidates` only if not already present.
pub fn push_unique(candidates: &mut Vec<String>, locale: &str) {
    if !candidates.iter().any(|candidate| candidate == locale) {
        candidates.push(locale.to_string());
    }
}
