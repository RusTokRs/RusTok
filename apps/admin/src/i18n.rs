//! Framework-neutral admin host message catalog.
//!
//! Locale selection and Leptos reactivity remain host concerns. Message
//! parsing, fallback, and formatting are owned by `rustok-ui-i18n`.

rustok_ui_i18n::declare_module_i18n!();

#[cfg(test)]
mod tests {
    use super::{MESSAGES, t};

    #[test]
    fn host_catalogs_are_valid_and_locale_specific() {
        assert!(MESSAGES.initialization_diagnostics().is_empty());
        assert_eq!(t(Some("en"), "app.nav.dashboard", "fallback"), "Dashboard");
        assert_eq!(t(Some("ru"), "app.nav.dashboard", "fallback"), "Дашборд");
    }
}
