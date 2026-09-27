/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use icu_locale::{
    Direction as IcuDirection, Locale as IcuLocale, LocaleCanonicalizer, LocaleDirectionality,
    LocaleExpander,
};
use unic_langid::LanguageIdentifier;

pub(crate) const MAX_LOCALE_TAG_LEN: usize = 64;

/// The resolved writing direction for a valid locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextDirection {
    /// Left-to-right writing direction.
    LeftToRight,
    /// Right-to-left writing direction.
    RightToLeft,
}

fn parse_unicode_locale(locale: &str) -> Option<IcuLocale> {
    if locale.is_empty() || locale.len() > MAX_LOCALE_TAG_LEN {
        return None;
    }

    let trimmed = locale.trim();
    if trimmed.is_empty() {
        return None;
    }

    // ICU4X 2.x deliberately accepts BCP-47 separators only. Preserve the
    // compatibility policy of this crate by normalizing legacy underscore input
    // before parsing the complete Unicode locale, including extensions.
    let normalized = trimmed.replace('_', "-");
    let mut locale = normalized.parse::<IcuLocale>().ok()?;
    let _ = LocaleCanonicalizer::new_extended().canonicalize(&mut locale);
    Some(locale)
}

fn icu_to_fluent_langid(locale: &IcuLocale) -> Option<LanguageIdentifier> {
    // Fluent 0.16 uses unic-langid. Keep that dependency type behind this module
    // while ICU4X owns full-locale parsing, aliases, and likely-subtag data.
    locale.id.to_string().parse().ok()
}

fn parse_locale_tag(locale: &str) -> Option<LanguageIdentifier> {
    parse_unicode_locale(locale)
        .as_ref()
        .and_then(icu_to_fluent_langid)
}

pub(crate) fn canonicalize_language_identifier(langid: LanguageIdentifier) -> LanguageIdentifier {
    let Ok(mut locale) = langid.to_string().parse::<IcuLocale>() else {
        return langid;
    };
    let _ = LocaleCanonicalizer::new_extended().canonicalize(&mut locale);
    icu_to_fluent_langid(&locale).unwrap_or(langid)
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

/// Parses and canonicalizes a complete Unicode locale identifier.
///
/// Language aliases are resolved with ICU4X/CLDR data and well-formed Unicode,
/// transformed, and private-use extensions are preserved. Underscore separators
/// are accepted for compatibility and serialized as canonical hyphens.
///
/// The input is bounded to 64 bytes before trimming and normalization.
pub fn normalize_unicode_locale(locale: &str) -> Option<String> {
    parse_unicode_locale(locale).map(|locale| locale.to_string())
}

/// Maps a complete Unicode locale to its canonical Fluent catalog identity.
///
/// Fluent 0.16 catalogs are keyed by a `LanguageIdentifier` (language, script,
/// region, and variants), not by formatting preferences. Valid Unicode,
/// transformed, and private-use extensions are therefore parsed and deliberately
/// removed *after* validation instead of causing the whole locale request to be
/// rejected. Deprecated language/region aliases are canonicalized with ICU4X.
/// Use [`normalize_unicode_locale`] when extension preferences must be retained
/// for date, number, calendar, or collation services.
///
/// Raw inputs longer than 64 bytes are rejected before trimming or normalization
/// work. Oversized request work therefore remains bounded.
pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    parse_locale_tag(locale).map(|langid| langid.to_string())
}

/// Returns the CLDR writing direction for a complete Unicode locale.
///
/// Missing scripts are inferred from language/region likely-subtag data. Invalid
/// or unknown locale identities return `None` rather than silently assuming LTR.
pub fn locale_text_direction(locale: &str) -> Option<TextDirection> {
    let locale = parse_unicode_locale(locale)?;
    match LocaleDirectionality::new_extended().get(&locale) {
        Some(IcuDirection::LeftToRight) => Some(TextDirection::LeftToRight),
        Some(IcuDirection::RightToLeft) => Some(TextDirection::RightToLeft),
        _ => None,
    }
}

/// Generates a deduplicated ordered list of locale fallback candidates.
///
/// Order of precedence:
/// 1. Requested catalog locale from most-specific to least-specific.
/// 2. If a request has a region but no script, CLDR likely-subtag data adds the
///    inferred script branch before the base language (`zh-TW` -> `zh-Hant`).
/// 3. Default locale from most-specific to least-specific.
/// 4. Canonical platform fallback (`"en"`).
///
/// Variant subtags are treated as one unordered specificity layer. `unic_langid`
/// canonicalizes variants as an ordered set, so peeling the serialized tag one
/// hyphen at a time can manufacture an arbitrary partial-variant parent. The
/// fallback therefore removes all variants together before region and script.
/// Valid locale extensions are ignored for catalog selection only after the full
/// locale has been parsed and validated.
pub fn locale_candidates(locale: Option<&str>, default_locale: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    push_locale_candidate_internal(&mut candidates, locale);
    push_locale_candidate_internal(&mut candidates, Some(default_locale));
    push_locale_candidate_internal(&mut candidates, Some("en"));

    candidates
}

fn push_locale_candidate_internal(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(mut langid) = locale.and_then(parse_locale_tag) else {
        return;
    };

    push_langid_candidate(candidates, &langid);

    if langid.variants().next().is_some() {
        langid.clear_variants();
        push_langid_candidate(candidates, &langid);
    }

    // A region often determines the writing system (for example zh-TW and
    // sr-RS). Add that CLDR-backed branch without inventing a region for a plain
    // language request such as `en`.
    if langid.script.is_none()
        && langid.region.is_some()
        && let Some(inferred) = infer_script(&langid)
    {
        push_langid_candidate(candidates, &inferred);
        let mut script_parent = inferred;
        script_parent.region = None;
        push_langid_candidate(candidates, &script_parent);
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

fn infer_script(langid: &LanguageIdentifier) -> Option<LanguageIdentifier> {
    let expander = LocaleExpander::new_extended();
    let mut regional = langid
        .to_string()
        .parse::<icu_locale::LanguageIdentifier>()
        .ok()?;
    let mut language = regional.clone();
    language.region = None;

    let _ = expander.maximize(&mut regional);
    let _ = expander.maximize(&mut language);

    // Add an inferred branch only when the region changes the language's usual
    // script. This keeps common chains compact (`ru-RU` -> `ru`) while fixing
    // genuinely ambiguous cases such as `zh-TW` (Hant vs the default Hans).
    if regional.script.is_none() || regional.script == language.script {
        return None;
    }
    regional.to_string().parse().ok()
}

/// Pushes a normalized locale and its progressively less-specific structural
/// parents to the candidate list.
#[deprecated(
    since = "0.1.0",
    note = "Use `locale_candidates` instead. This internal helper will be made private before 1.0."
)]
pub fn push_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    push_locale_candidate_internal(candidates, locale);
}

fn push_langid_candidate(candidates: &mut Vec<String>, langid: &LanguageIdentifier) {
    let locale = langid.to_string();
    push_unique_internal(candidates, &locale);
}

fn push_unique_internal(candidates: &mut Vec<String>, locale: &str) {
    if !candidates.iter().any(|candidate| candidate == locale) {
        candidates.push(locale.to_string());
    }
}

/// Appends `locale` to `candidates` only if not already present.
#[deprecated(
    since = "0.1.0",
    note = "Internal helper; will be made private before 1.0."
)]
pub fn push_unique(candidates: &mut Vec<String>, locale: &str) {
    push_unique_internal(candidates, locale);
}
