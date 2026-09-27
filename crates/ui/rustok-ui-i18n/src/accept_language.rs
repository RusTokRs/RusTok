/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::fmt;

use crate::{normalize_locale_tag, normalize_unicode_locale};

/// Maximum accepted raw `Accept-Language` field-value length in bytes.
pub const MAX_ACCEPT_LANGUAGE_LEN: usize = 4_096;
/// Maximum accepted number of comma-separated language ranges.
pub const MAX_ACCEPT_LANGUAGE_RANGES: usize = 64;

/// A bounded `Accept-Language` parsing failure.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptLanguageError {
    /// The raw field value exceeded [`MAX_ACCEPT_LANGUAGE_LEN`].
    HeaderTooLong { length: usize, max_len: usize },
    /// The field contained more than [`MAX_ACCEPT_LANGUAGE_RANGES`] ranges.
    TooManyRanges { count: usize, max_count: usize },
}

impl fmt::Display for AcceptLanguageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooLong { length, max_len } => write!(
                formatter,
                "Accept-Language length {length} bytes exceeds the supported maximum of {max_len} bytes"
            ),
            Self::TooManyRanges { count, max_count } => write!(
                formatter,
                "Accept-Language range count {count} exceeds the supported maximum of {max_count}"
            ),
        }
    }
}

impl std::error::Error for AcceptLanguageError {}

/// One canonical, quality-ranked `Accept-Language` preference.
///
/// `locale()` returns `None` for the wildcard range. Quality is represented as
/// an exact integer from 0 through 1000 instead of a floating-point value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptLanguagePreference {
    locale: Option<String>,
    quality_thousandths: u16,
    source_index: usize,
}

impl AcceptLanguagePreference {
    /// Returns the complete canonical Unicode locale, or `None` for `*`.
    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    /// Returns whether this preference is the wildcard language range.
    pub fn is_wildcard(&self) -> bool {
        self.locale.is_none()
    }

    /// Returns the RFC quality value as exact thousandths in `0..=1000`.
    pub fn quality_thousandths(&self) -> u16 {
        self.quality_thousandths
    }

    /// Returns whether this range is explicitly unacceptable (`q=0`).
    pub fn is_rejected(&self) -> bool {
        self.quality_thousandths == 0
    }
}

/// Strictly parses a bounded `Accept-Language` field value.
///
/// Valid preferences are returned by descending quality with stable source order
/// for ties. Malformed individual ranges and malformed/out-of-range q-values are
/// ignored so one bad client preference cannot discard later valid preferences.
/// Wildcards and `q=0` exclusions are retained in the typed result. Complete
/// Unicode locale identity, including extensions, is canonicalized and preserved.
///
/// The function parses only the field value; query/cookie/header precedence,
/// tenant allowlists, and effective-locale fallback remain host/runtime policy.
pub fn try_parse_accept_language(
    header: &str,
) -> Result<Vec<AcceptLanguagePreference>, AcceptLanguageError> {
    if header.len() > MAX_ACCEPT_LANGUAGE_LEN {
        return Err(AcceptLanguageError::HeaderTooLong {
            length: header.len(),
            max_len: MAX_ACCEPT_LANGUAGE_LEN,
        });
    }

    let range_count = header.split(',').count();
    if range_count > MAX_ACCEPT_LANGUAGE_RANGES {
        return Err(AcceptLanguageError::TooManyRanges {
            count: range_count,
            max_count: MAX_ACCEPT_LANGUAGE_RANGES,
        });
    }

    let mut preferences = header
        .split(',')
        .enumerate()
        .filter_map(|(source_index, range)| parse_preference(range, source_index))
        .collect::<Vec<_>>();
    preferences.sort_by(|left, right| {
        right
            .quality_thousandths
            .cmp(&left.quality_thousandths)
            .then_with(|| left.source_index.cmp(&right.source_index))
    });
    Ok(preferences)
}

/// Lenient bounded parser for request paths that use absence/default on failure.
///
/// Oversized headers or range-count overflow return an empty list. Use
/// [`try_parse_accept_language`] when the host needs a typed rejection reason.
pub fn parse_accept_language(header: &str) -> Vec<AcceptLanguagePreference> {
    try_parse_accept_language(header).unwrap_or_default()
}

/// Returns accepted complete Unicode locales in quality order.
///
/// Wildcards and `q=0` exclusions are omitted. Canonical duplicates retain their
/// highest-quality, earliest occurrence.
pub fn accept_language_locales(header: &str) -> Vec<String> {
    collect_locales(header, |locale| Some(locale.to_string()))
}

/// Returns accepted extension-free Fluent catalog identities in quality order.
///
/// This is the compatibility projection for message/domain catalog selection.
/// Use [`accept_language_locales`] when formatting extensions must be retained.
pub fn accept_language_catalog_locales(header: &str) -> Vec<String> {
    collect_locales(header, normalize_locale_tag)
}

/// Returns the highest-quality complete Unicode locale from an optional field value.
pub fn preferred_locale_from_accept_language(header: Option<&str>) -> Option<String> {
    header.and_then(|header| accept_language_locales(header).into_iter().next())
}

/// Returns the highest-quality extension-free catalog locale from an optional field value.
pub fn preferred_catalog_locale_from_accept_language(header: Option<&str>) -> Option<String> {
    header.and_then(|header| {
        accept_language_catalog_locales(header)
            .into_iter()
            .next()
    })
}

fn collect_locales(
    header: &str,
    mut project: impl FnMut(&str) -> Option<String>,
) -> Vec<String> {
    let mut locales = Vec::new();
    for preference in parse_accept_language(header) {
        if preference.is_rejected() {
            continue;
        }
        let Some(raw_locale) = preference.locale() else {
            continue;
        };
        let Some(locale) = project(raw_locale) else {
            continue;
        };
        if !locales.contains(&locale) {
            locales.push(locale);
        }
    }
    locales
}

fn parse_preference(range: &str, source_index: usize) -> Option<AcceptLanguagePreference> {
    let mut segments = range.trim().split(';');
    let raw_range = segments.next()?.trim();
    if raw_range.is_empty() {
        return None;
    }

    let locale = if raw_range == "*" {
        None
    } else {
        Some(normalize_unicode_locale(raw_range)?)
    };
    let mut quality_thousandths = 1_000;
    let mut quality_seen = false;

    for parameter in segments {
        let parameter = parameter.trim();
        let Some((name, value)) = parameter.split_once('=') else {
            if parameter.eq_ignore_ascii_case("q") {
                return None;
            }
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("q") {
            continue;
        }
        if quality_seen {
            return None;
        }
        quality_seen = true;
        quality_thousandths = parse_quality(value.trim())?;
    }

    Some(AcceptLanguagePreference {
        locale,
        quality_thousandths,
        source_index,
    })
}

fn parse_quality(value: &str) -> Option<u16> {
    if value == "0" {
        return Some(0);
    }
    if value == "1" {
        return Some(1_000);
    }

    let (whole, fraction) = value.split_once('.')?;
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    match whole {
        "0" => {
            let mut quality = if fraction.is_empty() {
                0
            } else {
                fraction.parse::<u16>().ok()?
            };
            for _ in fraction.len()..3 {
                quality *= 10;
            }
            Some(quality)
        }
        "1" if fraction.bytes().all(|byte| byte == b'0') => Some(1_000),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_sorting_is_exact_and_source_order_is_stable() {
        let locales = accept_language_catalog_locales(
            "en-US;q=0.7, ru-RU, de;q=0.700, fr;q=0.701, ja;q=0",
        );
        assert_eq!(locales, vec!["ru-RU", "fr", "en-US", "de"]);
    }

    #[test]
    fn aliases_extensions_wildcards_and_exclusions_remain_explicit() {
        let preferences = try_parse_accept_language(
            "iw-IL-u-ca-hebrew;q=0.8, *;q=0.5, en;q=0",
        )
        .expect("bounded header must parse");

        assert_eq!(preferences.len(), 3);
        assert_eq!(preferences[0].locale(), Some("he-IL-u-ca-hebrew"));
        assert_eq!(preferences[0].quality_thousandths(), 800);
        assert!(preferences[1].is_wildcard());
        assert!(preferences[2].is_rejected());
        assert_eq!(
            accept_language_catalog_locales("iw-IL-u-ca-hebrew;q=0.8"),
            vec!["he-IL"]
        );
    }

    #[test]
    fn malformed_quality_is_ignored_without_float_edge_cases() {
        assert_eq!(
            accept_language_locales(
                "de;q=NaN, fr;q=1.001, es;q=-1, it;q=0.1234, pt;q=.5, nl;q=0.6;q=0.7, ru;Q=0.600, en;q=0.5",
            ),
            vec!["ru", "en"]
        );
        assert_eq!(
            accept_language_locales("fr;q=1., de;q=0., en;q=0.010"),
            vec!["fr", "en"]
        );
    }

    #[test]
    fn canonical_duplicates_keep_highest_quality_then_earliest() {
        assert_eq!(
            accept_language_catalog_locales("iw;q=0.3, he;q=0.9, HE;q=0.5"),
            vec!["he"]
        );
    }

    #[test]
    fn preferred_helpers_distinguish_complete_and_catalog_identity() {
        let header = Some("de-DE-u-nu-latn;q=0.9, en;q=0.5");
        assert_eq!(
            preferred_locale_from_accept_language(header),
            Some("de-DE-u-nu-latn".to_string())
        );
        assert_eq!(
            preferred_catalog_locale_from_accept_language(header),
            Some("de-DE".to_string())
        );
    }

    #[test]
    fn raw_header_work_is_bounded_before_parsing() {
        let oversized = "a".repeat(MAX_ACCEPT_LANGUAGE_LEN + 1);
        assert!(matches!(
            try_parse_accept_language(&oversized),
            Err(AcceptLanguageError::HeaderTooLong { length, max_len })
                if length == MAX_ACCEPT_LANGUAGE_LEN + 1 && max_len == MAX_ACCEPT_LANGUAGE_LEN
        ));
        assert!(parse_accept_language(&oversized).is_empty());

        let too_many = std::iter::repeat_n("en", MAX_ACCEPT_LANGUAGE_RANGES + 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(matches!(
            try_parse_accept_language(&too_many),
            Err(AcceptLanguageError::TooManyRanges { count, max_count })
                if count == MAX_ACCEPT_LANGUAGE_RANGES + 1
                    && max_count == MAX_ACCEPT_LANGUAGE_RANGES
        ));
    }
}
