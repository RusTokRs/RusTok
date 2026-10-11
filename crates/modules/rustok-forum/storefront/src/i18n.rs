rustok_ui_i18n::declare_module_i18n!(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
        ("ar", include_str!("../locales/ar.ftl")),
    ]
);

/// Localized label for a raw topic or reply status token returned by the owner
/// transports.
///
/// Topics carry `open`, `closed`, `archived`, and `pending`; replies carry the moderation
/// vocabulary. A token this surface does not know is rendered unchanged: inventing a label
/// for a status the owner may add later would misreport the state of the content.
pub fn status_label(locale: Option<&str>, status: &str) -> String {
    let (key, fallback) = match status.to_ascii_lowercase().as_str() {
        "open" | "published" | "active" => ("forum.topic.statusOpen", "Open"),
        "closed" => ("forum.topic.statusClosed", "Closed"),
        "archived" => ("forum.topic.statusArchived", "Archived"),
        "pending" | "draft" => ("forum.topic.statusPending", "Pending"),
        "approved" => ("forum.topic.statusApproved", "Approved"),
        "rejected" => ("forum.topic.statusRejected", "Rejected"),
        "flagged" => ("forum.topic.statusFlagged", "Flagged"),
        "hidden" => ("forum.topic.statusHidden", "Hidden"),
        "deleted" => ("forum.topic.statusDeleted", "Deleted"),
        _ => return status.to_string(),
    };
    t(locale, key, fallback)
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn every_catalog_locale_names_the_topic_statuses() {
        for (
            locale, open, closed, archived, pending, approved, deleted,
        ) in [
            ("en", "Open", "Closed", "Archived", "Pending", "Approved", "Deleted"),
            (
                "ru",
                "Открыта",
                "Закрыта",
                "В архиве",
                "На модерации",
                "Одобрено",
                "Удалено",
            ),
            (
                "ar",
                "مفتوحة",
                "مغلقة",
                "مؤرشفة",
                "قيد المراجعة",
                "مقبول",
                "محذوف",
            ),
        ] {
            assert_eq!(t(Some(locale), "forum.topic.statusOpen", "?"), open);
            assert_eq!(t(Some(locale), "forum.topic.statusClosed", "?"), closed);
            assert_eq!(t(Some(locale), "forum.topic.statusArchived", "?"), archived);
            assert_eq!(t(Some(locale), "forum.topic.statusPending", "?"), pending);
            assert_eq!(t(Some(locale), "forum.topic.statusApproved", "?"), approved);
            assert_eq!(t(Some(locale), "forum.topic.statusDeleted", "?"), deleted);
        }
    }

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
