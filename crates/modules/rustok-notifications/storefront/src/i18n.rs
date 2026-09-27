rustok_ui_i18n::declare_module_i18n!(
    default = "en",
    locales = ["en", "ru"],
);

pub fn unread_count_label(locale: Option<&str>, count: u64) -> String {
    let args = rustok_ui_i18n::fluent_args!(count = count);
    format(
        locale,
        "notifications.navigation.unread",
        Some(&args),
        "Unread notifications",
    )
}

#[cfg(test)]
mod tests {
    use super::{t, unread_count_label};

    fn strip_bidi_isolates(value: &str) -> String {
        value.replace(['\u{2068}', '\u{2069}'], "")
    }

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
            strip_bidi_isolates(&unread_count_label(Some("ru-RU"), 4)),
            "Непрочитанных уведомлений: 4"
        );
    }
}
