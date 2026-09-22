rustok_ui_i18n::declare_module_i18n!();

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
