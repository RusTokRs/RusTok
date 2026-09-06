use rustok_ui_i18n_leptos::LeptosUiMessages;

static MESSAGES: LeptosUiMessages = LeptosUiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.json")),
        ("ru", include_str!("../locales/ru.json")),
    ],
);

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    MESSAGES.t_for_locale(locale, key, fallback)
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn resolves_regional_russian_admin_copy() {
        assert_eq!(
            t(Some("ru-RU"), "notifications.title", "Notifications"),
            "Уведомления"
        );
        assert_eq!(
            t(
                Some("ru-RU"),
                "notifications.sourceRegistry",
                "Source registry"
            ),
            "Реестр источников"
        );
    }
}
