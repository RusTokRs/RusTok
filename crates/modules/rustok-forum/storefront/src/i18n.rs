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
