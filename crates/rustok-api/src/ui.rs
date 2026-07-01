use std::collections::BTreeMap;

use crate::{build_locale_candidates, normalize_locale_tag};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiRouteContext {
    pub locale: Option<String>,
    pub route_segment: Option<String>,
    pub subpath: Option<String>,
    pub query: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiRouteQueryUpdate {
    Clear,
    Replace(String),
}

impl UiRouteQueryUpdate {
    pub fn into_query_value(self) -> Option<String> {
        match self {
            Self::Clear => None,
            Self::Replace(value) => Some(value),
        }
    }
}

pub fn normalize_ui_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn parse_ui_csv(value: &str) -> Vec<String> {
    value.split(',').filter_map(normalize_ui_text).collect()
}

pub fn route_query_update_for_text(value: &str) -> UiRouteQueryUpdate {
    if value.trim().is_empty() {
        UiRouteQueryUpdate::Clear
    } else {
        UiRouteQueryUpdate::Replace(value.to_string())
    }
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
        build_ui_message_catalog, normalize_ui_text, parse_ui_csv, resolve_ui_message,
        resolve_ui_message_or_fallback, route_query_update_for_text, UiRouteContext,
        UiRouteQueryUpdate,
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

    #[test]
    fn shared_ui_input_helpers_normalize_text_and_csv() {
        assert_eq!(
            normalize_ui_text("  catalog  "),
            Some("catalog".to_string())
        );
        assert_eq!(normalize_ui_text("   "), None);
        assert_eq!(
            parse_ui_csv(" product, blog ,, pages "),
            vec![
                "product".to_string(),
                "blog".to_string(),
                "pages".to_string()
            ]
        );
    }

    #[test]
    fn route_query_update_for_text_preserves_replacement_value() {
        assert_eq!(
            route_query_update_for_text("   "),
            UiRouteQueryUpdate::Clear
        );
        assert_eq!(
            route_query_update_for_text("  botas "),
            UiRouteQueryUpdate::Replace("  botas ".to_string())
        );
        assert_eq!(
            UiRouteQueryUpdate::Replace("value".to_string()).into_query_value(),
            Some("value".to_string())
        );
        assert_eq!(UiRouteQueryUpdate::Clear.into_query_value(), None);
    }
}
