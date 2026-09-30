use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

pub use rustok_ui_i18n::preferred_catalog_locale_from_accept_language as extract_locale_tag_from_header;

pub const PLATFORM_FALLBACK_LOCALE: &str = "en";
pub const UNKNOWN_PROVENANCE_LOCALE: &str = "und";

/// Maximum length of the canonical, extension-free locale identity exposed by
/// the host/API layer. This matches the platform's locale storage width.
pub const MAX_LOCALE_TAG_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LocaleTypeError {
    #[error("invalid locale tag '{value}'")]
    InvalidTag { value: String },
    #[error("storage-only locale 'und' cannot be used as a {kind} locale")]
    UnknownProvenanceNotAllowed { kind: &'static str },
}

fn normalize_typed_locale(
    raw: &str,
    kind: &'static str,
    allow_unknown_provenance: bool,
) -> Result<String, LocaleTypeError> {
    let normalized = normalize_locale_tag(raw).ok_or_else(|| LocaleTypeError::InvalidTag {
        value: raw.to_string(),
    })?;
    if !allow_unknown_provenance && normalized == UNKNOWN_PROVENANCE_LOCALE {
        return Err(LocaleTypeError::UnknownProvenanceNotAllowed { kind });
    }
    Ok(normalized)
}

macro_rules! define_locale_type {
    ($name:ident, $kind:literal, $allow_unknown_provenance:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(raw: impl AsRef<str>) -> Result<Self, LocaleTypeError> {
                normalize_typed_locale(raw.as_ref(), $kind, $allow_unknown_provenance).map(Self)
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = LocaleTypeError;

            fn from_str(raw: &str) -> Result<Self, Self::Err> {
                Self::new(raw)
            }
        }

        impl TryFrom<String> for $name {
            type Error = LocaleTypeError;

            fn try_from(raw: String) -> Result<Self, Self::Error> {
                Self::new(raw)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = LocaleTypeError;

            fn try_from(raw: &str) -> Result<Self, Self::Error> {
                Self::new(raw)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let raw = String::deserialize(deserializer)?;
                Self::new(raw).map_err(D::Error::custom)
            }
        }
    };
}

define_locale_type!(RuntimeLocale, "runtime", false);
define_locale_type!(TenantLocale, "tenant", false);
define_locale_type!(StoredLocale, "stored", true);

impl StoredLocale {
    pub fn is_unknown_provenance(&self) -> bool {
        self.as_str() == UNKNOWN_PROVENANCE_LOCALE
    }
}

impl From<TenantLocale> for RuntimeLocale {
    fn from(locale: TenantLocale) -> Self {
        Self(locale.into_inner())
    }
}

impl From<TenantLocale> for StoredLocale {
    fn from(locale: TenantLocale) -> Self {
        Self(locale.into_inner())
    }
}

impl From<RuntimeLocale> for StoredLocale {
    fn from(locale: RuntimeLocale) -> Self {
        Self(locale.into_inner())
    }
}

pub fn normalize_locale_tag(raw: &str) -> Option<String> {
    let normalized = rustok_ui_i18n::normalize_locale_tag(raw)?;
    (normalized.len() <= MAX_LOCALE_TAG_LEN).then_some(normalized)
}

pub fn is_valid_locale_tag(raw: &str) -> bool {
    normalize_locale_tag(raw).is_some()
}

pub fn locale_primary_language(raw: &str) -> Option<String> {
    normalize_locale_tag(raw).and_then(|value| {
        value
            .split_once('-')
            .map(|(language, _)| language.to_string())
            .or(Some(value))
    })
}

pub fn locale_tags_match(left: &str, right: &str) -> bool {
    normalize_locale_tag(left) == normalize_locale_tag(right)
}

pub fn push_locale_candidate(
    candidates: &mut Vec<String>,
    locale: Option<&str>,
    include_language_fallback: bool,
) {
    let Some(normalized) = locale.and_then(normalize_locale_tag) else {
        return;
    };

    if !candidates.iter().any(|candidate| candidate == &normalized) {
        candidates.push(normalized.clone());
    }

    if include_language_fallback
        && let Some(language) = locale_primary_language(normalized.as_str())
        && language != normalized
        && !candidates.iter().any(|candidate| candidate == &language)
    {
        candidates.push(language);
    }
}

pub fn build_locale_candidates<'a>(
    locales: impl IntoIterator<Item = Option<&'a str>>,
    include_language_fallback: bool,
) -> Vec<String> {
    let mut candidates = Vec::new();
    for locale in locales {
        push_locale_candidate(&mut candidates, locale, include_language_fallback);
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeLocale, StoredLocale, TenantLocale, UNKNOWN_PROVENANCE_LOCALE,
        build_locale_candidates, extract_locale_tag_from_header, is_valid_locale_tag,
        locale_primary_language, locale_tags_match, normalize_locale_tag,
    };

    #[test]
    fn normalize_locale_tag_uses_canonical_unicode_i18n_identity() {
        assert_eq!(normalize_locale_tag("ru"), Some("ru".to_string()));
        assert_eq!(normalize_locale_tag("ru_ru"), Some("ru-RU".to_string()));
        assert_eq!(normalize_locale_tag("pt_br"), Some("pt-BR".to_string()));
        assert_eq!(normalize_locale_tag("zh-hant"), Some("zh-Hant".to_string()));
        assert_eq!(normalize_locale_tag("es-419"), Some("es-419".to_string()));
        assert_eq!(
            normalize_locale_tag("iw-IL-u-ca-hebrew"),
            Some("he-IL".to_string())
        );
    }

    #[test]
    fn normalize_locale_tag_rejects_invalid_values() {
        assert_eq!(normalize_locale_tag(""), None);
        assert_eq!(normalize_locale_tag("e"), None);
        assert_eq!(normalize_locale_tag("en-*"), None);
        assert_eq!(normalize_locale_tag("12"), None);
    }

    #[test]
    fn locale_candidates_preserve_priority_and_add_language_fallback() {
        let candidates = build_locale_candidates([Some("pt-BR"), Some("en"), Some("pt_br")], true);

        assert_eq!(candidates, vec!["pt-BR", "pt", "en"]);
    }

    #[test]
    fn locale_primary_language_uses_normalized_language() {
        assert_eq!(locale_primary_language("zh-hant").as_deref(), Some("zh"));
    }

    #[test]
    fn locale_tags_match_compares_canonical_forms() {
        assert!(locale_tags_match("pt_br", "pt-BR"));
        assert!(is_valid_locale_tag("zh-Hant"));
    }

    #[test]
    fn accept_language_prefers_highest_quality_valid_tag() {
        assert_eq!(
            extract_locale_tag_from_header(Some("en-US;q=0.5, ru-RU;q=0.9")),
            Some("ru-RU".to_string())
        );
        assert_eq!(
            extract_locale_tag_from_header(Some("iw-IL-u-ca-hebrew;q=0.9, en;q=0.5")),
            Some("he-IL".to_string())
        );
        assert_eq!(
            extract_locale_tag_from_header(Some("fr;q=NaN, en;q=0.5")),
            Some("en".to_string())
        );
        assert_eq!(extract_locale_tag_from_header(Some("en;q=0")), None);
    }

    #[test]
    fn typed_locales_separate_runtime_policy_from_storage_provenance() {
        assert_eq!(RuntimeLocale::new("pt_br").unwrap().as_str(), "pt-BR");
        assert_eq!(TenantLocale::new("zh-hant").unwrap().as_str(), "zh-Hant");
        assert_eq!(
            RuntimeLocale::new("iw-IL-u-ca-hebrew").unwrap().as_str(),
            "he-IL"
        );
        assert!(RuntimeLocale::new(UNKNOWN_PROVENANCE_LOCALE).is_err());
        assert!(TenantLocale::new(UNKNOWN_PROVENANCE_LOCALE).is_err());

        let stored = StoredLocale::new(UNKNOWN_PROVENANCE_LOCALE).unwrap();
        assert!(stored.is_unknown_provenance());
    }

    #[test]
    fn typed_locale_deserialization_canonicalizes_and_rejects_und_for_runtime() {
        let runtime: RuntimeLocale = serde_json::from_str("\"pt_br\"").unwrap();
        assert_eq!(runtime.as_str(), "pt-BR");
        assert!(serde_json::from_str::<RuntimeLocale>("\"und\"").is_err());

        let stored: StoredLocale = serde_json::from_str("\"und\"").unwrap();
        assert!(stored.is_unknown_provenance());
    }
}
