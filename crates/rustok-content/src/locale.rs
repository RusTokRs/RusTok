use std::collections::BTreeSet;

use rustok_api::{PLATFORM_FALLBACK_LOCALE, locale_tags_match, normalize_locale_tag};

pub struct ResolvedLocale<'a, T> {
    pub item: Option<&'a T>,
    pub effective_locale: String,
}

pub fn resolve_by_locale<'a, T, F>(
    items: &'a [T],
    requested: &str,
    locale_of: F,
) -> ResolvedLocale<'a, T>
where
    F: Fn(&T) -> &str,
{
    resolve_by_locale_with_fallback(items, requested, None, locale_of)
}

pub fn resolve_by_locale_with_fallback<'a, T, F>(
    items: &'a [T],
    requested: &str,
    fallback_locale: Option<&str>,
    locale_of: F,
) -> ResolvedLocale<'a, T>
where
    F: Fn(&T) -> &str,
{
    if let Some(item) = items
        .iter()
        .find(|item| locale_tags_match(locale_of(item), requested))
    {
        return ResolvedLocale {
            item: Some(item),
            effective_locale: normalized_locale_or_raw(requested),
        };
    }

    if let Some(fallback_locale) = fallback_locale
        && fallback_locale != requested
        && let Some(item) = items
            .iter()
            .find(|item| locale_tags_match(locale_of(item), fallback_locale))
    {
        return ResolvedLocale {
            item: Some(item),
            effective_locale: normalized_locale_or_raw(fallback_locale),
        };
    }

    if let Some(item) = items
        .iter()
        .find(|item| locale_tags_match(locale_of(item), PLATFORM_FALLBACK_LOCALE))
    {
        return ResolvedLocale {
            item: Some(item),
            effective_locale: PLATFORM_FALLBACK_LOCALE.to_string(),
        };
    }

    // Database queries are not ordered unless an ORDER BY is explicit. The
    // final fallback therefore cannot depend on slice insertion/query order.
    // Locale identity is the only owner-neutral ordering key available here,
    // so choose the lexicographically smallest normalized locale.
    if let Some(item) = items
        .iter()
        .min_by_key(|item| normalized_locale_or_raw(locale_of(item)))
    {
        return ResolvedLocale {
            item: Some(item),
            effective_locale: normalized_locale_or_raw(locale_of(item)),
        };
    }

    ResolvedLocale {
        item: None,
        effective_locale: normalized_locale_or_raw(requested),
    }
}

pub fn available_locales_from<T, F>(items: &[T], locale_of: F) -> Vec<String>
where
    F: Fn(&T) -> &str,
{
    items
        .iter()
        .map(|item| normalized_locale_or_raw(locale_of(item)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn normalize_locale_code(locale: &str) -> Option<String> {
    normalize_locale_tag(locale)
}

fn normalized_locale_or_raw(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| locale.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct LocalizedItem {
        locale: &'static str,
    }

    #[test]
    fn resolves_requested_locale_first() {
        let items = [
            LocalizedItem { locale: "en" },
            LocalizedItem { locale: "ru" },
        ];

        let resolved = resolve_by_locale(&items, "ru", |item| item.locale);

        assert_eq!(resolved.item.map(|item| item.locale), Some("ru"));
        assert_eq!(resolved.effective_locale, "ru");
    }

    #[test]
    fn resolves_platform_fallback_before_first_available() {
        let items = [
            LocalizedItem { locale: "de" },
            LocalizedItem { locale: "en" },
        ];

        let resolved = resolve_by_locale(&items, "ru", |item| item.locale);

        assert_eq!(resolved.item.map(|item| item.locale), Some("en"));
        assert_eq!(resolved.effective_locale, "en");
    }

    #[test]
    fn resolves_tenant_fallback_before_platform_fallback() {
        let items = [
            LocalizedItem { locale: "ru" },
            LocalizedItem { locale: "en" },
        ];

        let resolved =
            resolve_by_locale_with_fallback(&items, "de", Some("ru"), |item| item.locale);

        assert_eq!(resolved.item.map(|item| item.locale), Some("ru"));
        assert_eq!(resolved.effective_locale, "ru");
    }

    #[test]
    fn final_fallback_is_deterministic_by_normalized_locale() {
        let items = [
            LocalizedItem { locale: "fr" },
            LocalizedItem { locale: "de" },
        ];
        let reversed = [
            LocalizedItem { locale: "de" },
            LocalizedItem { locale: "fr" },
        ];

        let resolved = resolve_by_locale(&items, "ru", |item| item.locale);
        let reversed_resolved = resolve_by_locale(&reversed, "ru", |item| item.locale);

        assert_eq!(resolved.item.map(|item| item.locale), Some("de"));
        assert_eq!(resolved.effective_locale, "de");
        assert_eq!(reversed_resolved.item.map(|item| item.locale), Some("de"));
        assert_eq!(reversed_resolved.effective_locale, "de");
    }

    #[test]
    fn available_locales_are_normalized_unique_and_sorted() {
        let items = [
            LocalizedItem { locale: "fr" },
            LocalizedItem { locale: " EN_us " },
            LocalizedItem { locale: "de" },
            LocalizedItem { locale: "fr" },
        ];

        assert_eq!(
            available_locales_from(&items, |item| item.locale),
            vec!["de".to_string(), "en-US".to_string(), "fr".to_string()]
        );
    }

    #[test]
    fn normalizes_locale_code() {
        assert_eq!(normalize_locale_code(" EN_us "), Some("en-US".to_string()));
        assert_eq!(normalize_locale_code(""), None);
    }
}
