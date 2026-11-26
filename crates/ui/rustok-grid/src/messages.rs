//! Catalog-resolved grid chrome shared by framework adapters.
//!
//! Hosts pass their already-resolved UI locale. This owner never negotiates it
//! from cookies, routes or browser state.
use rustok_ui_i18n::{FluentArgs, UiMessages};

static MESSAGES: UiMessages = UiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
        ("fr", include_str!("../locales/fr.ftl")),
    ],
);

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GridMessages {
    locale: Option<String>,
}

impl GridMessages {
    pub fn for_locale(locale: Option<&str>) -> Self {
        Self {
            locale: locale.map(str::to_owned),
        }
    }

    /// The locale this catalog resolves against, when the host chose one.
    ///
    /// `None` means "no explicit choice was made": the Leptos adapter then
    /// falls back to the ambient grid locale it provides, so a table cannot end
    /// up English-only by omission.
    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    /// Catalog copy for `key`, resolved against this locale.
    pub fn text(&self, key: &str) -> String {
        MESSAGES.t_for_locale(self.locale.as_deref(), key, key)
    }

    pub fn column_label(&self, key: &str, column: &str) -> String {
        let mut args = FluentArgs::new();
        args.set("column", column);
        MESSAGES.format(self.locale.as_deref(), key, Some(&args), key)
    }

    pub fn range(&self, from: usize, to: usize, total: u64) -> String {
        let mut args = FluentArgs::new();
        args.set("from", from.to_string());
        args.set("to", to.to_string());
        args.set("total", total.to_string());
        MESSAGES.format(self.locale.as_deref(), "grid.range", Some(&args), "")
    }

    pub fn selected(&self, count: usize) -> String {
        let mut args = FluentArgs::new();
        args.set("count", count as f64);
        MESSAGES.format(self.locale.as_deref(), "grid.selected", Some(&args), "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn visible(text: String) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn grid_copy_uses_host_locale_and_fluent_arguments() {
        assert_eq!(
            visible(GridMessages::for_locale(Some("ru-RU")).range(11, 20, 42)),
            "Показано 11–20 из 42"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("fr")).range(11, 20, 42)),
            "11–20 sur 42"
        );
        assert_eq!(
            GridMessages::for_locale(Some("fr")).text("grid.next"),
            "Suivant"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("en")).selected(1)),
            "1 row selected"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("en")).selected(2)),
            "2 rows selected"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("fr")).selected(1)),
            "1 ligne sélectionnée"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("fr")).selected(2)),
            "2 lignes sélectionnées"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("ru")).selected(2)),
            "Выбрано 2 строки"
        );
        assert_eq!(
            visible(GridMessages::for_locale(Some("ru")).selected(5)),
            "Выбрано 5 строк"
        );
        assert!(MESSAGES.validate().is_ok());
    }

    #[test]
    fn unset_catalog_reports_no_locale_so_the_host_can_supply_one() {
        // `DataGrid`'s prop default is the empty catalog; the adapter reads
        // `locale()` to decide whether the ambient context locale should win.
        assert_eq!(GridMessages::default().locale(), None);
        assert_eq!(GridMessages::for_locale(None).locale(), None);
        assert_eq!(GridMessages::for_locale(Some("ru")).locale(), Some("ru"));
        assert_eq!(GridMessages::default().text("grid.next"), "Next");
    }
}
