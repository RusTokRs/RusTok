use rustok_ui_i18n::UiMessages;

static MESSAGES: UiMessages = UiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
    ],
);

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    MESSAGES.t_for_locale(locale, key, fallback)
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn resolves_regional_russian_locale() {
        assert_eq!(
            t(Some("ru-RU"), "fulfillment.shipping.badge", "Shipping"),
            "Доставка"
        );
    }
}
