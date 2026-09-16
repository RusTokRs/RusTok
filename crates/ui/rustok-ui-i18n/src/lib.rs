/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod bundle;
pub mod error;
pub mod locale;
#[macro_use]
pub mod macros;
pub mod messages;

pub use fluent_bundle::{FluentArgs, FluentValue};
pub use unic_langid::LanguageIdentifier;

pub use bundle::{build_fluent_bundle, build_fluent_catalog, FluentCatalog};
pub use error::{BundleBuildError, I18nError};
pub use locale::{
    locale_candidates, normalize_admin_locale, normalize_locale_tag, push_locale_candidate,
    push_unique,
};
pub use messages::{
    resolve_fluent_message, with_kebab_key, UiMessages, UiTranslator,
};

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
