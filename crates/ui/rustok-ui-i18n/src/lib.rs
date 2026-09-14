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
use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::FluentResource;
use serde_json::Value;

pub use fluent_bundle::{FluentArgs, FluentValue};
pub use unic_langid::LanguageIdentifier;

pub type UiMessageCatalog = BTreeMap<String, BTreeMap<String, String>>;
pub type FluentCatalog = BTreeMap<String, FluentBundle<FluentResource>>;

pub struct UiTranslator<'a> {
    catalog: Option<&'a UiMessageCatalog>,
    fluent_catalog: Option<&'a FluentCatalog>,
    default_locale: &'a str,
}

impl<'a> UiTranslator<'a> {
    pub const fn new(catalog: &'a UiMessageCatalog, default_locale: &'a str) -> Self {
        Self {
            catalog: Some(catalog),
            fluent_catalog: None,
            default_locale,
        }
    }

    pub const fn with_fluent(fluent_catalog: &'a FluentCatalog, default_locale: &'a str) -> Self {
        Self {
            catalog: None,
            fluent_catalog: Some(fluent_catalog),
            default_locale,
        }
    }

    pub const fn combined(
        catalog: &'a UiMessageCatalog,
        fluent_catalog: &'a FluentCatalog,
        default_locale: &'a str,
    ) -> Self {
        Self {
            catalog: Some(catalog),
            fluent_catalog: Some(fluent_catalog),
            default_locale,
        }
    }

    pub fn resolve(&self, locale: Option<&str>, key: &str) -> Option<String> {
        if let Some(fluent) = self.fluent_catalog {
            if let Some(msg) = resolve_fluent_message(fluent, locale, self.default_locale, key, None) {
                return Some(msg);
            }
        }
        if let Some(catalog) = self.catalog {
            return resolve_ui_message(catalog, locale, self.default_locale, key);
        }
        None
    }

    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format_message(locale, key, None, fallback)
    }

    pub fn format_message<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        if let Some(fluent) = self.fluent_catalog {
            if let Some(msg) = resolve_fluent_message(fluent, locale, self.default_locale, key, args) {
                return msg;
            }
        }
        if let Some(catalog) = self.catalog {
            if let Some(msg) = resolve_ui_message(catalog, locale, self.default_locale, key) {
                return msg;
            }
        }
        fallback.to_string()
    }
}

pub struct UiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
    json_catalog: OnceLock<UiMessageCatalog>,
    fluent_catalog: OnceLock<FluentCatalog>,
}

impl UiMessages {
    pub const fn new(
        default_locale: &'static str,
        bundles: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            default_locale,
            bundles,
            json_catalog: OnceLock::new(),
            fluent_catalog: OnceLock::new(),
        }
    }

    pub fn json_catalog(&self) -> &UiMessageCatalog {
        self.json_catalog
            .get_or_init(|| build_ui_message_catalog(self.bundles))
    }

    pub fn catalog(&self) -> &UiMessageCatalog {
        self.json_catalog()
    }

    pub fn fluent_catalog(&self) -> &FluentCatalog {
        self.fluent_catalog
            .get_or_init(|| build_fluent_catalog(self.bundles))
    }

    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format(locale, key, None, fallback)
    }

    pub fn t_for_locale(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.t(locale, key, fallback)
    }

    pub fn format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        let is_json = self
            .bundles
            .first()
            .map_or(false, |(_, content)| content.trim_start().starts_with('{'));

        if !is_json {
            if let Some(msg) = resolve_fluent_message(
                self.fluent_catalog(),
                locale,
                self.default_locale,
                key,
                args,
            ) {
                return msg;
            }
        }

        if let Some(msg) = resolve_ui_message(self.json_catalog(), locale, self.default_locale, key) {
            return msg;
        }

        if is_json {
            if let Some(msg) = resolve_fluent_message(
                self.fluent_catalog(),
                locale,
                self.default_locale,
                key,
                args,
            ) {
                return msg;
            }
        }

        fallback.to_string()
    }
}

pub fn normalize_admin_locale(locale: Option<&str>) -> &'static str {
    match locale {
        Some(value)
            if value.eq_ignore_ascii_case("ru")
                || value.starts_with("ru-")
                || value.starts_with("ru_") =>
        {
            "ru"
        }
        _ => "en",
    }
}

#[macro_export]
macro_rules! fluent_args {
    ($($key:expr => $val:expr),* $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $(
            args.set($key, $crate::FluentValue::from($val));
        )*
        args
    }};
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

pub fn build_fluent_bundle(
    locale: &str,
    ftl_source: &str,
) -> Result<FluentBundle<FluentResource>, String> {
    let langid: LanguageIdentifier = locale.parse().map_err(|e| format!("Invalid locale: {e}"))?;
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    let resource = FluentResource::try_new(ftl_source.to_string())
        .map_err(|(_, errors)| format!("Fluent parse errors: {errors:?}"))?;
    bundle
        .add_resource(resource)
        .map_err(|errors| format!("Failed to add resource: {errors:?}"))?;
    Ok(bundle)
}

pub fn build_fluent_catalog(bundles: &[(&str, &str)]) -> FluentCatalog {
    let mut catalog = FluentCatalog::new();

    for (locale, ftl_source) in bundles {
        let Some(normalized) = normalize_locale_tag(locale) else {
            continue;
        };

        if let Ok(bundle) = build_fluent_bundle(&normalized, ftl_source) {
            catalog.insert(normalized, bundle);
        }
    }

    catalog
}

pub fn resolve_fluent_message<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    let candidates = locale_candidates(locale, default_locale);

    for candidate in candidates {
        if let Some(bundle) = catalog.get(candidate.as_str()) {
            if let Some(message) = bundle.get_message(key) {
                if let Some(pattern) = message.value() {
                    let mut errors = vec![];
                    let formatted = bundle.format_pattern(pattern, args, &mut errors);
                    if !errors.is_empty() {
                        tracing::warn!(?errors, key, "Fluent message formatting errors");
                    }
                    return Some(formatted.to_string());
                }
            }
        }
    }

    None
}

pub fn resolve_ui_message(
    catalog: &UiMessageCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
) -> Option<String> {
    let candidates = locale_candidates(locale, default_locale);

    for candidate in candidates {
        if let Some(messages) = catalog.get(candidate.as_str())
            && let Some(value) = messages.get(key)
        {
            return Some(value.clone());
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

pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    let normalized = locale.trim().replace('_', "-");
    if normalized.is_empty() {
        return None;
    }

    let langid: LanguageIdentifier = normalized.parse().ok()?;
    Some(langid.to_string())
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
    use super::*;

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
    fn normalize_locale_tag_supports_iso_639_three_letter_codes() {
        assert_eq!(normalize_locale_tag("fil"), Some("fil".to_string()));
        assert_eq!(normalize_locale_tag("yue"), Some("yue".to_string()));
        assert_eq!(normalize_locale_tag("ru_RU"), Some("ru-RU".to_string()));
    }

    #[test]
    fn ui_translator_resolves_with_literal_fallback() {
        let catalog = build_ui_message_catalog(&[("en", r#"{ "title": "Dashboard" }"#)]);
        let translator = UiTranslator::new(&catalog, "en");

        assert_eq!(translator.t(Some("fr"), "title", "Fallback"), "Dashboard");
        assert_eq!(translator.t(Some("fr"), "missing", "Fallback"), "Fallback");
    }

    #[test]
    fn fluent_russian_plurals_evaluate_correctly() {
        let ftl_ru = r#"
cart-items = { $count ->
    [one] { $count } товар
    [few] { $count } товара
   *[other] { $count } товаров
}
"#;
        let catalog = build_fluent_catalog(&[("ru", ftl_ru)]);
        let translator = UiTranslator::with_fluent(&catalog, "ru");

        let mut args1 = FluentArgs::new();
        args1.set("count", 1);
        assert_eq!(
            translator.format_message(Some("ru"), "cart-items", Some(&args1), "fallback"),
            "1 товар"
        );

        let mut args2 = FluentArgs::new();
        args2.set("count", 3);
        assert_eq!(
            translator.format_message(Some("ru"), "cart-items", Some(&args2), "fallback"),
            "3 товара"
        );

        let mut args5 = FluentArgs::new();
        args5.set("count", 5);
        assert_eq!(
            translator.format_message(Some("ru"), "cart-items", Some(&args5), "fallback"),
            "5 товаров"
        );

        let mut args21 = FluentArgs::new();
        args21.set("count", 21);
        assert_eq!(
            translator.format_message(Some("ru"), "cart-items", Some(&args21), "fallback"),
            "21 товар"
        );
    }

    #[test]
    fn ui_messages_resolves_json_bundles() {
        static MESSAGES: UiMessages = UiMessages::new(
            "en",
            &[
                ("en", r#"{ "title": "Dashboard", "hello": "Hello {name}" }"#),
                ("ru", r#"{ "title": "Панель" }"#),
            ],
        );

        assert_eq!(MESSAGES.t(Some("ru"), "title", "Fallback"), "Панель");
        assert_eq!(MESSAGES.t(Some("ru-RU"), "title", "Fallback"), "Панель");
        assert_eq!(MESSAGES.t_for_locale(Some("en"), "title", "Fallback"), "Dashboard");
        assert_eq!(MESSAGES.t(Some("es"), "title", "Fallback"), "Dashboard");
        assert_eq!(MESSAGES.t(Some("ru"), "missing", "Fallback"), "Fallback");
    }

    #[test]
    fn ui_messages_resolves_fluent_bundles_with_macro_args() {
        const FTL_EN: &str = r#"
welcome = Welcome, { $name }!
items-count = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
"#;
        const FTL_RU: &str = r#"
welcome = Добро пожаловать, { $name }!
items-count = { $count ->
    [one] { $count } товар
    [few] { $count } товара
   *[other] { $count } товаров
}
"#;
        static MESSAGES: UiMessages = UiMessages::new(
            "en",
            &[("en", FTL_EN), ("ru", FTL_RU)],
        );

        let args = fluent_args!["name" => "Alice"];
        assert_eq!(
            MESSAGES.format(Some("en"), "welcome", Some(&args), "Fallback"),
            "Welcome, Alice!"
        );
        assert_eq!(
            MESSAGES.format(Some("ru"), "welcome", Some(&args), "Fallback"),
            "Добро пожаловать, Alice!"
        );

        let count_args = fluent_args!["count" => 3];
        assert_eq!(
            MESSAGES.format(Some("ru"), "items-count", Some(&count_args), "Fallback"),
            "3 товара"
        );
    }

    #[test]
    fn normalize_admin_locale_maps_correctly() {
        assert_eq!(normalize_admin_locale(Some("ru")), "ru");
        assert_eq!(normalize_admin_locale(Some("ru-RU")), "ru");
        assert_eq!(normalize_admin_locale(Some("ru_RU")), "ru");
        assert_eq!(normalize_admin_locale(Some("en")), "en");
        assert_eq!(normalize_admin_locale(Some("en-US")), "en");
        assert_eq!(normalize_admin_locale(None), "en");
    }
}
