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

pub fn with_count(template: String, count: u64) -> String {
    let count = count.to_string();
    template.replace("{count}", count.as_str())
}

#[cfg(test)]
mod tests {
    use super::{t, with_count};

    #[test]
    fn resolves_regional_russian_navigation_copy() {
        assert_eq!(
            t(
                Some("ru-RU"),
                "notifications.navigation.label",
                "Notifications"
            ),
            "Уведомления"
        );
        assert_eq!(
            with_count(
                t(
                    Some("ru-RU"),
                    "notifications.navigation.unread",
                    "{count} unread notifications"
                ),
                4
            ),
            "Непрочитанных уведомлений: 4"
        );
    }
}
