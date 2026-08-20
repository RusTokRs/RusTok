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
    use std::collections::BTreeSet;

    use super::t;

    fn message_keys(source: &str) -> BTreeSet<String> {
        let messages = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(source)
            .expect("Forum locale catalog must be a JSON object");
        messages.into_keys().collect()
    }

    #[test]
    fn arabic_catalog_matches_the_canonical_english_key_set() {
        assert_eq!(
            message_keys(include_str!("../locales/ar.json")),
            message_keys(include_str!("../locales/en.json"))
        );
    }

    #[test]
    fn arabic_catalog_is_selected_for_arabic_runtime_locale() {
        assert_eq!(t(Some("ar"), "forum.form.name", "Name"), "الاسم");
        assert_eq!(
            t(Some("ar-SA"), "forum.topics.previewTitle", "Replies"),
            "الردود"
        );
    }
}
