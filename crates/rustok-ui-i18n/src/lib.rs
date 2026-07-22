/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::BTreeMap;

use serde_json::Value;

pub type UiMessageCatalog = BTreeMap<String, BTreeMap<String, String>>;

pub struct UiTranslator<'a> {
    catalog: &'a UiMessageCatalog,
    default_locale: &'a str,
}

impl<'a> UiTranslator<'a> {
    pub const fn new(catalog: &'a UiMessageCatalog, default_locale: &'a str) -> Self {
        Self {
            catalog,
            default_locale,
        }
    }

    pub fn resolve(&self, locale: Option<&str>, key: &str) -> Option<String> {
        resolve_ui_message(self.catalog, locale, self.default_locale, key)
    }

    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        resolve_ui_message_or_fallback(self.catalog, locale, self.default_locale, key, fallback)
    }
}

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
    let mut candidates = Vec::new();

    push_locale_candidate(&mut candidates, locale);
    push_locale_candidate(&mut candidates, Some(default_locale));
    push_locale_candidate(&mut candidates, Some("en"));

    candidates
}

fn push_locale_candidate(candidates: &mut Vec<String>, locale: Option<&str>) {
    let Some(locale) = locale.and_then(normalize_locale_tag) else {
        return;
    };

    push_unique(candidates, locale.as_str());

    if let Some((language, _)) = locale.split_once('-') {
        push_unique(candidates, language);
    }
}

fn push_unique(candidates: &mut Vec<String>, locale: &str) {
    if !candidates.iter().any(|candidate| candidate == locale) {
        candidates.push(locale.to_string());
    }
}

fn normalize_locale_tag(locale: &str) -> Option<String> {
    let normalized = locale.trim().replace('_', "-");
    if normalized.is_empty() {
        return None;
    }

    let mut parts = normalized.split('-').filter(|part| !part.is_empty());
    let language = parts.next()?.to_ascii_lowercase();
    if language.len() != 2 || !language.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }

    let mut tag = language;
    for part in parts {
        tag.push('-');
        tag.push_str(&part.to_ascii_uppercase());
    }

    Some(tag)
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
        UiTranslator, build_ui_message_catalog, resolve_ui_message, resolve_ui_message_or_fallback,
    };

    #[test]
    fn resolve_ui_message_falls_back_from_regional_locale_to_language() {
        let catalog = build_ui_message_catalog(&[
            ("en", r#"{ "title": "Workflows" }"#),
            ("fr", r#"{ "title": "Workflows FR" }"#),
        ]);

        let resolved = resolve_ui_message(&catalog, Some("fr-FR"), "en", "title");

        assert_eq!(resolved.as_deref(), Some("Workflows FR"));
    }

    #[test]
    fn resolve_ui_message_uses_default_locale_before_platform_fallback() {
        let catalog = build_ui_message_catalog(&[
            ("en", r#"{ "title": "Workflows" }"#),
            ("de", r#"{ "title": "Workflows DE" }"#),
        ]);

        let resolved = resolve_ui_message(&catalog, Some("fr"), "de", "title");

        assert_eq!(resolved.as_deref(), Some("Workflows DE"));
    }

    #[test]
    fn resolve_ui_message_or_fallback_returns_literal_fallback_when_key_is_missing() {
        let catalog = build_ui_message_catalog(&[("en", r#"{ "title": "Workflows" }"#)]);

        let resolved =
            resolve_ui_message_or_fallback(&catalog, Some("fr"), "en", "missing", "Fallback");

        assert_eq!(resolved, "Fallback");
    }

    #[test]
    fn build_ui_message_catalog_flattens_nested_json() {
        let catalog = build_ui_message_catalog(&[(
            "en",
            r#"{ "blog": { "posts": { "title": "Posts" } }, "ignored": 1 }"#,
        )]);

        assert_eq!(
            catalog
                .get("en")
                .and_then(|messages| messages.get("blog.posts.title"))
                .map(String::as_str),
            Some("Posts")
        );
        assert!(
            !catalog
                .get("en")
                .expect("en catalog")
                .contains_key("ignored")
        );
    }

    #[test]
    fn build_ui_message_catalog_normalizes_locale_tags() {
        let catalog = build_ui_message_catalog(&[("pt_br", r#"{ "title": "Title" }"#)]);

        assert!(catalog.contains_key("pt-BR"));
    }

    #[test]
    fn ui_translator_resolves_with_literal_fallback() {
        let catalog = build_ui_message_catalog(&[("en", r#"{ "title": "Dashboard" }"#)]);
        let translator = UiTranslator::new(&catalog, "en");

        assert_eq!(translator.t(Some("fr"), "title", "Fallback"), "Dashboard");
        assert_eq!(translator.t(Some("fr"), "missing", "Fallback"), "Fallback");
    }
}
