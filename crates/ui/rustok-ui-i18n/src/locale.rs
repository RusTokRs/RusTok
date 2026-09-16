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
use unic_locale::Locale;

pub(crate) const MAX_LOCALE_TAG_LEN: usize = 64;

fn normalize_locale_input(locale: &str) -> Option<String> {
    if locale.is_empty() || locale.len() > MAX_LOCALE_TAG_LEN {
        return None;
    }

    let trimmed = locale.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.replace('_', "-"))
}

fn parse_locale_tag(locale: &str) -> Option<LanguageIdentifier> {
    normalize_locale_input(locale)?.parse().ok()
}

/// Parses a host-selected effective locale for message lookup.
///
/// Catalog identity intentionally remains `LanguageIdentifier`-only, but effective
/// locale input may be a full Unicode Locale Identifier carrying extensions such as
/// `u-nu-latn` or private-use data. Ordinary language identifiers stay on the fast
/// path; extension-bearing inputs are validated as `Locale` and reduced to their
/// base language identifier before catalog fallback is constructed.
fn parse_effective_locale_tag(locale: &str) -> Option<LanguageIdentifier> {
    let normalized = normalize_locale_input(locale)?;

    if let Ok(langid) = normalized.parse::<LanguageIdentifier>() {
        return Some(langid);
    }

    normalized.parse::<Locale>().ok().map(|locale| locale.id)
}

/// Normalizes the admin UI effective locale to either "ru" or "en".
///
/// Full Unicode Locale Identifiers are accepted for effective-locale selection;
/// their extensions do not participate in message catalog identity. If `locale`
/// is absent, malformed, or not Russian, this defaults to `"en"`.
pub fn normalize_admin_locale(locale: Option<&str>) -> &'static str {
    let Some(langid) = locale.and_then(parse_effective_locale_tag) else {
        return "en";
    };

    if langid.language.as_str().eq_ignore_ascii_case("ru") {
        "ru"
    } else {
        "en"
    }
}

/// Parses and normalizes a catalog-safe Unicode language identifier, replacing
/// underscores with hyphens.
///
/// This intentionally does not retain or strip Unicode/private-use extensions:
/// callers configuring catalog/default locale identity must supply a plain
/// `LanguageIdentifier`. Effective request/render locales are handled separately
/// by [`locale_candidates`], which validates full Unicode locale identifiers and
/// resolves them through their base language identifier.
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
/// 1. Requested effective locale's base language identifier from most-specific
///    to least-specific (e.g. `zh-Hant-TW-u-nu-hanidec` -> `zh-Hant-TW` ->
///    `zh-Hant` -> `zh`)
/// 2. Default catalog locale from most-specific to least-specific
/// 3. Canonical platform fallback (`"en"`)
///
/// Unicode/private-use extensions on the requested effective locale are validated
/// but do not become Rust catalog identity. Default/catalog locales remain strict
/// `LanguageIdentifier`s. Variant subtags are treated as one unordered specificity
/// layer because `unic_langid` canonicalizes variants as an ordered set; peeling a
/// serialized tag one hyphen at a time can manufacture an arbitrary partial-variant
/// parent.
pub fn locale_candidates(locale: Option<&str>, default_locale: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    push_effective_locale_candidate(&mut candidates, locale);
    push_locale_candidate(&mut candidates, Some(default_locale));
    push_locale_candidate(&mut candidates, Some("en"));

    candidates
}

fn push_effective_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(langid) = locale.and_then(parse_effective_locale_tag) else {
        return;
    };

    push_structural_candidates(candidates, langid);
}

/// Pushes a normalized catalog/default locale and its progressively less-specific
/// structural parents to the candidate list.
pub fn push_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(langid) = locale.and_then(parse_locale_tag) else {
        return;
    };

    push_structural_candidates(candidates, langid);
}

fn push_structural_candidates(candidates: &mut Vec<String>, mut langid: LanguageIdentifier) {
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
