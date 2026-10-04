//! Framework-neutral admin host message catalog.
//!
//! Locale selection and Leptos reactivity remain host concerns. Message
//! parsing, fallback, and formatting are owned by `rustok-ui-i18n`.

// Keep the host fallback explicit: the repository i18n contract verifies this
// declaration independently from the library macro's convenience default.
rustok_ui_i18n::declare_module_i18n!(default = "en", locales = ["en", "ru"],);

#[cfg(test)]
mod tests {
    use super::{MESSAGES, t};

    #[test]
    fn host_catalogs_are_valid_and_locale_specific() {
        assert!(MESSAGES.initialization_diagnostics().is_empty());
        assert_eq!(t(None, "app.nav.dashboard", "fallback"), "Dashboard");
        assert_eq!(
            t(Some("de-DE"), "app.nav.dashboard", "fallback"),
            "Dashboard"
        );
        assert_eq!(t(Some("en"), "app.nav.dashboard", "fallback"), "Dashboard");
        assert_eq!(t(Some("ru"), "app.nav.dashboard", "fallback"), "Дашборд");
        assert_eq!(
            t(Some("en"), "modules.build.progress", "fallback"),
            "Platform build progress"
        );
        assert_eq!(
            t(Some("ru"), "modules.build.progress", "fallback"),
            "Прогресс сборки платформы"
        );
    }
}
