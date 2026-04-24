use std::collections::BTreeMap;

use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiRouteContext {
    pub locale: Option<String>,
    pub route_segment: Option<String>,
    pub subpath: Option<String>,
    pub query: BTreeMap<String, String>,
}

impl UiRouteContext {
    pub fn query_value(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(String::as_str)
    }

    pub fn module_route_base(&self, route_segment: &str) -> String {
        let route_segment = route_segment.trim_matches('/');
        match self
            .locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(locale) if route_segment.is_empty() => format!("/{locale}/modules"),
            Some(locale) => format!("/{locale}/modules/{route_segment}"),
            None if route_segment.is_empty() => "/modules".to_string(),
            None => format!("/modules/{route_segment}"),
        }
    }

    pub fn subpath(&self) -> Option<&str> {
        self.subpath.as_deref()
    }

    pub fn subpath_matches(&self, prefix: &str) -> bool {
        self.subpath()
            .map(|subpath| subpath == prefix || subpath.starts_with(&format!("{prefix}/")))
            .unwrap_or(false)
    }
}

pub type UiMessageCatalog = BTreeMap<String, BTreeMap<String, String>>;

pub fn build_ui_message_catalog(bundles: &[(&str, &str)]) -> UiMessageCatalog {
    let mut catalog = UiMessageCatalog::new();

    for (locale, bundle) in bundles {
        let Some(locale) = normalize_locale_tag(locale) else {
            continue;
        };

        let value = serde_json::from_str::<Value>(bundle).unwrap_or(Value::Null);
        let mut messages = BTreeMap::new();
        flatten_ui_messages(&value, "", &mut messages);
        catalog.insert(locale, messages);
    }

    catalog
}

pub fn resolve_ui_message(
    catalog: &UiMessageCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
) -> Option<String> {
    let candidates = locale_candidates(locale, default_locale);

    for candidate in candidates {
        if let Some(messages) = catalog.get(candidate.as_str()) {
            if let Some(value) = messages.get(key) {
                return Some(value.clone());
            }
        }
    }

    None
}

pub fn resolve_ui_message_or_fallback(
    catalog: &UiMessageCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    fallback: &str,
) -> String {
    resolve_ui_message(catalog, locale, default_locale, key).unwrap_or_else(|| fallback.to_string())
}

fn locale_candidates(locale: Option<&str>, default_locale: &str) -> Vec<String> {
    build_locale_candidates([locale, Some(default_locale), Some("en")], true)
}

fn normalize_locale_tag(raw: &str) -> Option<String> {
    let candidate = raw.trim().replace('_', "-");
    if candidate.is_empty() || candidate.len() > 32 {
        return None;
    }

    if !candidate
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }

    let parts = candidate.split('-').collect::<Vec<_>>();
    let language = parts.first()?.trim();
    if language.len() < 2
        || language.len() > 8
        || !language.chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return None;
    }

    let mut normalized = Vec::with_capacity(parts.len());
    normalized.push(language.to_ascii_lowercase());

    for part in parts.into_iter().skip(1) {
        if part.is_empty() || part.len() > 8 {
            return None;
        }

        let normalized_part = if part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic())
        {
            part.to_ascii_uppercase()
        } else if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let mut chars = part.chars();
            let head = chars
                .next()
                .map(|ch| ch.to_ascii_uppercase().to_string())
                .unwrap_or_default();
            let tail = chars.as_str().to_ascii_lowercase();
            format!("{head}{tail}")
        } else if part.len() == 3 && part.chars().all(|ch| ch.is_ascii_digit()) {
            part.to_string()
        } else if (5..=8).contains(&part.len()) && part.chars().all(|ch| ch.is_ascii_alphanumeric())
        {
            part.to_ascii_lowercase()
        } else {
            return None;
        };

        normalized.push(normalized_part);
    }

    Some(normalized.join("-"))
}

fn locale_primary_language(raw: &str) -> Option<String> {
    normalize_locale_tag(raw).and_then(|value| {
        value
            .split_once('-')
            .map(|(language, _)| language.to_string())
            .or(Some(value))
    })
}

fn push_locale_candidate(
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

    if include_language_fallback {
        if let Some(language) = locale_primary_language(normalized.as_str()) {
            if language != normalized && !candidates.iter().any(|candidate| candidate == &language)
            {
                candidates.push(language);
            }
        }
    }
}

fn build_locale_candidates<'a>(
    locales: impl IntoIterator<Item = Option<&'a str>>,
    include_language_fallback: bool,
) -> Vec<String> {
    let mut candidates = Vec::new();
    for locale in locales {
        push_locale_candidate(&mut candidates, locale, include_language_fallback);
    }
    candidates
}

fn flatten_ui_messages(value: &Value, prefix: &str, target: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next_prefix = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_ui_messages(child, next_prefix.as_str(), target);
            }
        }
        Value::String(text) if !prefix.is_empty() => {
            target.insert(prefix.to_string(), text.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_ui_message_catalog, resolve_ui_message, resolve_ui_message_or_fallback,
        UiRouteContext,
    };

    #[test]
    fn module_route_base_uses_locale_prefix_when_present() {
        let route_context = UiRouteContext {
            locale: Some("ru".to_string()),
            ..Default::default()
        };

        assert_eq!(route_context.module_route_base("blog"), "/ru/modules/blog");
    }

    #[test]
    fn module_route_base_falls_back_to_legacy_path_without_locale() {
        let route_context = UiRouteContext::default();

        assert_eq!(route_context.module_route_base("pages"), "/modules/pages");
    }

    #[test]
    fn resolve_ui_message_falls_back_from_regional_locale_to_language() {
        let catalog = build_ui_message_catalog(&[
            ("en", r#"{ "title": "Workflows" }"#),
            ("ru", r#"{ "title": "Потоки" }"#),
        ]);

        let resolved = resolve_ui_message(&catalog, Some("ru-RU"), "en", "title");

        assert_eq!(resolved.as_deref(), Some("Потоки"));
    }

    #[test]
    fn resolve_ui_message_uses_default_locale_before_platform_fallback() {
        let catalog = build_ui_message_catalog(&[
            ("en", r#"{ "title": "Workflows" }"#),
            ("de", r#"{ "title": "Arbeitsabläufe" }"#),
        ]);

        let resolved = resolve_ui_message(&catalog, Some("fr"), "de", "title");

        assert_eq!(resolved.as_deref(), Some("Arbeitsabläufe"));
    }

    #[test]
    fn resolve_ui_message_or_fallback_returns_literal_fallback_when_key_is_missing() {
        let catalog = build_ui_message_catalog(&[("en", r#"{ "title": "Workflows" }"#)]);

        let resolved =
            resolve_ui_message_or_fallback(&catalog, Some("ru"), "en", "missing", "Fallback");

        assert_eq!(resolved, "Fallback");
    }
}
