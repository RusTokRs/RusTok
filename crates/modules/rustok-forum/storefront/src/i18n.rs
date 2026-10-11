rustok_ui_i18n::declare_module_i18n!(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
        ("ar", include_str!("../locales/ar.ftl")),
    ]
);

/// Localized plural-aware count label resolved with the number bound as a Fluent argument.
///
/// The no-argument `t` family cannot serve these keys: a pattern that references `$count` fails
/// Fluent formatting when no arguments are supplied, every locale then falls back to the caller
/// literal, and the storefront silently renders English copy for a Russian or Arabic reader.
/// Resolving through `format` lets the catalog plural select run, which is the only way Slavic and
/// Arabic plural categories are chosen correctly. Substituting the `{count}` placeholder afterwards
/// is a no-op for a resolved message and keeps the caller literal usable when a catalog lacks a key.
pub fn count_message(locale: Option<&str>, key: &str, fallback: &str, count: u64) -> String {
    let mut arguments = rustok_ui_i18n::FluentArgs::new();
    arguments.set("count", i64::try_from(count).unwrap_or(i64::MAX));
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::forum_storefront_count_label(resolved.as_str(), count)
}

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
    use super::{count_message, t};

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
    fn count_message_runs_the_catalog_plural_select() {
        // A message with a count variable only renders from the catalog when the argument is
        // supplied, so these labels are localized instead of falling back to the caller literal.
        assert_eq!(
            count_message(Some("en"), "forum.thread.repliesTotal", "{count} total", 1),
            "1 total"
        );
        assert_eq!(
            count_message(Some("ru"), "forum.thread.repliesTotal", "{count} total", 42),
            "42 всего"
        );
    }

    #[test]
    fn count_message_keeps_the_caller_literal_usable_for_unknown_keys() {
        assert_eq!(
            count_message(Some("en"), "forum.feed.absentKey", "{count} loaded", 2),
            "2 loaded"
        );
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
