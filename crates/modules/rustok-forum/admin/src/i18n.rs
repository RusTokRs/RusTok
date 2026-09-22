rustok_ui_i18n::declare_module_i18n!(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
        ("ar", include_str!("../locales/ar.ftl")),
    ]
);

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::t;

    fn message_keys(source: &str) -> BTreeSet<String> {
        source
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                line.split_once('=').map(|(k, _)| k.trim().to_string())
            })
            .collect()
    }

    #[test]
    fn arabic_catalog_matches_the_canonical_english_key_set() {
        assert_eq!(
            message_keys(include_str!("../locales/ar.ftl")),
            message_keys(include_str!("../locales/en.ftl"))
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
