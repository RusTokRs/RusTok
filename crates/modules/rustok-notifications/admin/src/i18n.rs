rustok_ui_i18n::declare_module_i18n!();

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
