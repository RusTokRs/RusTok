use rustok_core::{locale_tags_match, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE};

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
            effective_locale: normalize_locale_tag(requested)
                .unwrap_or_else(|| requested.to_string()),
        };
    }

    if let Some(fallback_locale) = fallback_locale {
        if fallback_locale != requested {
            if let Some(item) = items
                .iter()
                .find(|item| locale_tags_match(locale_of(item), fallback_locale))
            {
                return ResolvedLocale {
                    item: Some(item),
                    effective_locale: normalize_locale_tag(fallback_locale)
                        .unwrap_or_else(|| fallback_locale.to_string()),
                };
            }
        }
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

    if let Some(item) = items.first() {
        return ResolvedLocale {
            item: Some(item),
            effective_locale: normalize_locale_tag(locale_of(item))
                .unwrap_or_else(|| locale_of(item).to_string()),
        };
    }

    ResolvedLocale {
        item: None,
        effective_locale: normalize_locale_tag(requested).unwrap_or_else(|| requested.to_string()),
    }
}

pub fn available_locales_from<T, F>(items: &[T], locale_of: F) -> Vec<String>
where
    F: Fn(&T) -> &str,
{
    let mut locales = Vec::new();
    for item in items {
        let locale =
            normalize_locale_tag(locale_of(item)).unwrap_or_else(|| locale_of(item).to_string());
        if !locales.iter().any(|value| value == &locale) {
            locales.push(locale);
        }
    }
    locales
}

pub fn normalize_locale_code(locale: &str) -> Option<String> {
    normalize_locale_tag(locale)
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
    fn resolves_first_available_when_platform_fallback_missing() {
        let items = [
            LocalizedItem { locale: "de" },
            LocalizedItem { locale: "fr" },
        ];

        let resolved = resolve_by_locale(&items, "ru", |item| item.locale);

        assert_eq!(resolved.item.map(|item| item.locale), Some("de"));
        assert_eq!(resolved.effective_locale, "de");
    }

    #[test]
    fn normalizes_locale_code() {
        assert_eq!(normalize_locale_code(" EN_us "), Some("en-US".to_string()));
        assert_eq!(normalize_locale_code(""), None);
    }
}
