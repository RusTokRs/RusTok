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

pub use fluent_bundle::{FluentArgs, FluentValue};
pub use unic_langid::LanguageIdentifier;

pub type FluentCatalog = BTreeMap<String, FluentBundle<FluentResource>>;

pub struct UiTranslator<'a> {
    fluent_catalog: &'a FluentCatalog,
    default_locale: &'a str,
}

impl<'a> UiTranslator<'a> {
    pub const fn new(fluent_catalog: &'a FluentCatalog, default_locale: &'a str) -> Self {
        Self {
            fluent_catalog,
            default_locale,
        }
    }

    pub fn resolve(&self, locale: Option<&str>, key: &str) -> Option<String> {
        resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, None)
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
        if let Some(msg) =
            resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, args)
        {
            return msg;
        }
        fallback.to_string()
    }
}

pub struct UiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
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
            fluent_catalog: OnceLock::new(),
        }
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
        if let Some(msg) =
            resolve_fluent_message(self.fluent_catalog(), locale, self.default_locale, key, args)
        {
            return msg;
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

/// Helper macro for constructing `FluentArgs`.
/// Supports both:
/// - `fluent_args!(name = val, count = count)`
/// - `fluent_args!("name" => val, "count" => count)`
#[macro_export]
macro_rules! fluent_args {
    ($($key:ident = $val:expr),* $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $(
            args.set(stringify!($key), $crate::FluentValue::from($val));
        )*
        args
    }};
    ($($key:expr => $val:expr),* $(,)?) => {{
        let mut args = $crate::FluentArgs::new();
        $(
            args.set($key, $crate::FluentValue::from($val));
        )*
        args
    }};
}

/// Global macro for resolving messages against a specified `UiMessages` reference.
#[macro_export]
macro_rules! t {
    ($messages:expr, $locale:expr, $key:expr, $fallback:expr) => {
        $messages.t_for_locale($locale, $key, $fallback)
    };
    ($messages:expr, $locale:expr, $key:expr, $fallback:expr, $($arg_name:ident = $arg_val:expr),+ $(,)?) => {
        $messages.format(
            $locale,
            $key,
            Some(&$crate::fluent_args!($($arg_name = $arg_val),+)),
            $fallback,
        )
    };
    ($messages:expr, $locale:expr, $key:expr, $fallback:expr, $($arg_name:expr => $arg_val:expr),+ $(,)?) => {
        $messages.format(
            $locale,
            $key,
            Some(&$crate::fluent_args!($($arg_name => $arg_val),+)),
            $fallback,
        )
    };
}

/// Macro for resolving module-local messages with parameters.
#[macro_export]
macro_rules! module_t {
    ($locale:expr, $key:expr, $fallback:expr) => {
        $crate::t($locale, $key, $fallback)
    };
    ($locale:expr, $key:expr, $fallback:expr, $($arg_name:ident = $arg_val:expr),+ $(,)?) => {
        $crate::format(
            $locale,
            $key,
            Some(&$crate::fluent_args!($($arg_name = $arg_val),+)),
            $fallback,
        )
    };
}

/// Declares canonical module-owned i18n catalogs and accessor functions.
///
/// Default invocation (loads `../locales/en.ftl` and `../locales/ru.ftl`):
/// ```ignore
/// rustok_ui_i18n::declare_module_i18n!();
/// ```
///
/// Custom invocation (with additional locales):
/// ```ignore
/// rustok_ui_i18n::declare_module_i18n!(
///     "en",
///     &[
///         ("en", include_str!("../locales/en.ftl")),
///         ("ru", include_str!("../locales/ru.ftl")),
///         ("ar", include_str!("../locales/ar.ftl")),
///     ]
/// );
/// ```
#[macro_export]
macro_rules! declare_module_i18n {
    () => {
        $crate::declare_module_i18n!(
            "en",
            &[
                ("en", include_str!("../locales/en.ftl")),
                ("ru", include_str!("../locales/ru.ftl")),
            ]
        );
    };
    ($default_locale:expr, $bundles:expr) => {
        static MESSAGES: $crate::UiMessages = $crate::UiMessages::new($default_locale, $bundles);

        #[inline]
        pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
            MESSAGES.t_for_locale(locale, key, fallback)
        }

        #[inline]
        pub fn format<'args>(
            locale: Option<&str>,
            key: &str,
            args: Option<&$crate::FluentArgs<'args>>,
            fallback: &str,
        ) -> String {
            MESSAGES.format(locale, key, args, fallback)
        }
    };
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

const STACK_KEY_BUF_SIZE: usize = 128;

/// Executes a closure with a kebab-case representation of `key`.
///
/// If `key` contains '.', replaces '.' with '-' using a fixed stack buffer for
/// keys <= 128 bytes, avoiding any heap allocation in the hot rendering path.
#[inline]
pub fn with_kebab_key<R>(key: &str, f: impl FnOnce(&str) -> R) -> R {
    if !key.contains('.') {
        return f(key);
    }

    if key.len() <= STACK_KEY_BUF_SIZE {
        let mut buf = [0u8; STACK_KEY_BUF_SIZE];
        let bytes = key.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            buf[i] = if b == b'.' { b'-' } else { b };
        }
        // SAFETY: '.' (0x2E) and '-' (0x2D) are single-byte ASCII characters.
        // Replacing '.' with '-' in valid UTF-8 maintains valid UTF-8.
        let kebab = unsafe { std::str::from_utf8_unchecked(&buf[..key.len()]) };
        f(kebab)
    } else {
        let kebab = key.replace('.', "-");
        f(&kebab)
    }
}

pub fn resolve_fluent_message<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    let candidates = locale_candidates(locale, default_locale);

    with_kebab_key(key, |lookup_key| {
        for candidate in candidates {
            if let Some(bundle) = catalog.get(candidate.as_str()) {
                if let Some(message) = bundle.get_message(lookup_key) {
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
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_locale_tag_supports_iso_639_three_letter_codes() {
        assert_eq!(normalize_locale_tag("fil"), Some("fil".to_string()));
        assert_eq!(normalize_locale_tag("yue"), Some("yue".to_string()));
        assert_eq!(normalize_locale_tag("ru_RU"), Some("ru-RU".to_string()));
        assert_eq!(normalize_locale_tag("pt_BR"), Some("pt-BR".to_string()));
    }

    #[test]
    fn with_kebab_key_converts_without_heap_allocations() {
        with_kebab_key("workflow.table.name", |k| {
            assert_eq!(k, "workflow-table-name");
        });
        with_kebab_key("already-kebab", |k| {
            assert_eq!(k, "already-kebab");
        });
        // Long key beyond stack buffer
        let long_key = "a.".repeat(100);
        with_kebab_key(&long_key, |k| {
            assert!(!k.contains('.'));
            assert!(k.contains('-'));
        });
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
        let translator = UiTranslator::new(&catalog, "ru");

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
        static MESSAGES: UiMessages = UiMessages::new("en", &[("en", FTL_EN), ("ru", FTL_RU)]);

        let args = fluent_args!["name" => "Alice"];
        assert_eq!(
            MESSAGES.format(Some("en"), "welcome", Some(&args), "Fallback"),
            "Welcome, Alice!"
        );
        assert_eq!(
            MESSAGES.format(Some("ru"), "welcome", Some(&args), "Fallback"),
            "Добро пожаловать, Alice!"
        );

        // Test fluent_args! with identifier syntax
        let args_ident = fluent_args!(name = "Bob");
        assert_eq!(
            MESSAGES.format(Some("en"), "welcome", Some(&args_ident), "Fallback"),
            "Welcome, Bob!"
        );

        let count_args = fluent_args!(count = 3);
        assert_eq!(
            MESSAGES.format(Some("ru"), "items-count", Some(&count_args), "Fallback"),
            "3 товара"
        );

        // Verify dotted key resolves via kebab-case mapping
        assert_eq!(
            MESSAGES.format(Some("ru"), "items.count", Some(&count_args), "Fallback"),
            "3 товара"
        );

        // Verify t! macro with arguments
        assert_eq!(
            t!(MESSAGES, Some("ru"), "items.count", "fallback", count = 5),
            "5 товаров"
        );
        assert_eq!(
            t!(MESSAGES, Some("en"), "welcome", "fallback", name = "Charlie"),
            "Welcome, Charlie!"
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

    #[test]
    fn workspace_module_ftl_files_parse_cleanly() {
        use std::fs;
        use std::path::Path;

        fn visit_dirs(dir: &Path, ftl_files: &mut Vec<std::path::PathBuf>) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dirs(&path, ftl_files);
                    } else if path.extension().and_then(|s| s.to_str()) == Some("ftl") {
                        ftl_files.push(path);
                    }
                }
            }
        }

        let modules_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules");
        let mut ftl_files = Vec::new();
        visit_dirs(&modules_dir, &mut ftl_files);

        assert!(!ftl_files.is_empty(), "Expected to find module .ftl files");

        for file_path in ftl_files {
            let content = fs::read_to_string(&file_path)
                .unwrap_or_else(|e| panic!("Failed to read {file_path:?}: {e}"));
            let filename = file_path.file_name().unwrap().to_str().unwrap();
            let locale = if filename.starts_with("ru") { "ru" } else { "en" };
            build_fluent_bundle(locale, &content)
                .unwrap_or_else(|e| panic!("Failed to parse Fluent resource in {file_path:?}: {e}"));
        }
    }

    #[test]
    fn test_multiline_json_placeholder() {
        let ftl = r#"
page-builder-translations-valuesPlaceholder =
    {"{"}
      "en": "Welcome",
      "ru": "Добро пожаловать"
    {"}"}

page-builder-translations-localizedMetadataValuesPlaceholder =
    {"{"}
      "title": {"{"} "en": "Home", "ru": "Главная" {"}"},
      "description": {"{"} "en": "Welcome", "ru": "Добро пожаловать" {"}"}
    {"}"}
"#;
        let bundle = build_fluent_bundle("en", ftl).unwrap();
        assert!(bundle.has_message("page-builder-translations-valuesPlaceholder"));
        assert!(bundle.has_message("page-builder-translations-localizedMetadataValuesPlaceholder"));
    }

    mod mock_module {
        const EN_FTL: &str = "test-title = Title\ntest-greet = Hello, { $name }!\n";
        const RU_FTL: &str = "test-title = Заголовок\ntest-greet = Привет, { $name }!\n";

        super::declare_module_i18n!("en", &[("en", EN_FTL), ("ru", RU_FTL)]);

        #[test]
        fn declare_module_i18n_expands_and_resolves_correctly() {
            assert_eq!(t(Some("en"), "test.title", "Fallback"), "Title");
            assert_eq!(t(Some("ru"), "test.title", "Fallback"), "Заголовок");
            assert_eq!(t(Some("fr"), "test.title", "Fallback"), "Title");
            assert_eq!(t(Some("en"), "missing.key", "Fallback"), "Fallback");

            let args = fluent_args!(name = "Иван");
            assert_eq!(
                format(Some("ru"), "test.greet", Some(&args), "Fallback"),
                "Привет, Иван!"
            );
            assert_eq!(
                crate::t!(MESSAGES, Some("ru"), "test.greet", "Fallback", name = "Иван"),
                "Привет, Иван!"
            );
        }
    }
}
