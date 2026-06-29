use std::sync::OnceLock;

use rustok_api::{build_ui_message_catalog, resolve_ui_message_or_fallback, UiMessageCatalog};

static CATALOG: OnceLock<UiMessageCatalog> = OnceLock::new();

fn catalog() -> &'static UiMessageCatalog {
    CATALOG.get_or_init(|| {
        build_ui_message_catalog(&[
            ("en", include_str!("../locales/en.json")),
            ("ru", include_str!("../locales/ru.json")),
        ])
    })
}

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    resolve_ui_message_or_fallback(catalog(), locale, "en", key, fallback)
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn resolves_host_locale_and_falls_back_to_english() {
        assert_eq!(t(Some("ru-RU"), "users.title", "Users"), "Пользователи");
        assert_eq!(t(Some("fr"), "users.title", "Users"), "Users");
    }
}
