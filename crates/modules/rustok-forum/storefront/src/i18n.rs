use rustok_ui_i18n_leptos::LeptosUiMessages;

static MESSAGES: LeptosUiMessages = LeptosUiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.json")),
        ("ru", include_str!("../locales/ru.json")),
        ("ar", include_str!("../locales/ar.json")),
    ],
);

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    MESSAGES.t_for_locale(locale, key, fallback)
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn arabic_catalog_is_selected_for_arabic_runtime_locale() {
        assert_eq!(
            t(Some("ar"), "forum.categories.label", "Categories"),
            "التصنيفات"
        );
        assert_eq!(
            t(Some("ar-SA"), "forum.thread.repliesTitle", "Replies"),
            "الردود"
        );
    }
}
