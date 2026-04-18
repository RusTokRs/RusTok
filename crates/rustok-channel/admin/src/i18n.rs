use std::sync::OnceLock;

use rustok_api::{UiMessageCatalog, build_ui_message_catalog, resolve_ui_message_or_fallback};

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
