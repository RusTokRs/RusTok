rustok_ui_i18n::declare_module_i18n!(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
        ("ar", include_str!("../locales/ar.ftl")),
    ]
);

/// Localized message that interpolates a count, for example `{ $count } threads`.
///
/// The no-argument `t` family cannot serve these keys: `fluent-bundle` records a reference error for
/// every unbound placeable, the shared resolver treats any error as a failed lookup, and the caller
/// literal wins. That means every locale silently renders the English fallback and the catalog's
/// plural select never runs. Binding the number as a Fluent argument is what makes the Slavic and
/// Arabic categories resolve; it has to be a number, because a string selector only matches an
/// explicit variant key. The trailing placeholder substitution covers a catalog that does not carry
/// the message yet, where the caller literal is what renders.
pub fn count_message<Count>(locale: Option<&str>, key: &str, fallback: &str, count: Count) -> String
where
    Count: Into<rustok_ui_i18n::FluentValue<'static>> + ToString,
{
    let count_text = count.to_string();
    // The `'static` annotation is what makes the bound usable: `FluentArgs` is invariant in its
    // argument lifetime, so an inferred region would ask for `Into<FluentValue<'args>>` for a local
    // `'args` the caller bound cannot satisfy.
    let mut arguments: rustok_ui_i18n::FluentArgs<'static> = rustok_ui_i18n::FluentArgs::new();
    arguments.set("count", count);
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::forum_storefront_count_label(resolved.as_str(), count_text)
}

/// Localized message that interpolates a slug, for example `slug: { $slug }`.
///
/// Resolved with a bound argument for the same reason as [`count_message`]: without one the catalog
/// text never renders. The trailing substitution keeps the caller literal usable when a locale has
/// no such message.
pub fn slug_message<'a>(locale: Option<&str>, key: &str, fallback: &str, slug: &'a str) -> String {
    // Annotated for the same reason as in [`count_message`]: the argument borrows the caller's
    // string, so the `FluentArgs` lifetime is named instead of left to inference.
    let mut arguments: rustok_ui_i18n::FluentArgs<'a> = rustok_ui_i18n::FluentArgs::new();
    arguments.set("slug", slug);
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::forum_storefront_slug_label(resolved.as_str(), slug)
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
    use super::{count_message, slug_message, t};

    /// `rustok-ui-i18n` builds its bundles with bidi isolation on, so an interpolated value is
    /// wrapped in invisible FSI/PDI marks. Assertions compare the visible text.
    fn visible(text: &str) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

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
        // Binding the argument is the only way these variants are reachable: `t` on the same key
        // fails formatting and returns the caller literal in every locale.
        assert_eq!(
            visible(&count_message(
                Some("en"),
                "forum.feed.threads",
                "{count} threads",
                1_u64
            )),
            "1 thread"
        );
        assert_eq!(
            visible(&count_message(
                Some("en"),
                "forum.feed.threads",
                "{count} threads",
                3_u64
            )),
            "3 threads"
        );
        assert_eq!(
            visible(&count_message(
                Some("ru"),
                "forum.feed.threads",
                "{count} threads",
                3_u64
            )),
            "3 треда"
        );
        assert_eq!(
            visible(&count_message(
                Some("ru"),
                "forum.feed.threads",
                "{count} threads",
                5_u64
            )),
            "5 тредов"
        );
    }

    #[test]
    fn count_message_accepts_the_owner_count_types_without_narrowing() {
        // The storefront holds `u64` page sizes, `i64` read-state counts and `i32` counters, and a
        // negative counter must not be silently clamped to zero.
        assert_eq!(
            visible(&count_message(
                Some("en"),
                "forum.thread.repliesTotal",
                "{count} total",
                42_u64
            )),
            "42 total"
        );
        assert_eq!(
            visible(&count_message(
                Some("ru"),
                "forum.thread.repliesTotal",
                "{count} total",
                7_i64
            )),
            "7 всего"
        );
        assert_eq!(
            count_message(Some("en"), "forum.feed.absentKey", "{count} loaded", -1_i64),
            "-1 loaded"
        );
    }

    #[test]
    fn count_message_keeps_the_caller_literal_usable_for_unknown_keys() {
        assert_eq!(
            count_message(Some("en"), "forum.feed.absentKey", "{count} loaded", 2_u64),
            "2 loaded"
        );
    }

    #[test]
    fn slug_message_interpolates_the_slug_in_every_catalog() {
        assert_eq!(
            visible(&slug_message(
                Some("en"),
                "forum.thread.slug",
                "slug: {slug}",
                "welcome"
            )),
            "slug: welcome"
        );
        assert_eq!(
            visible(&slug_message(
                Some("ar"),
                "forum.thread.slug",
                "slug: {slug}",
                "welcome"
            )),
            "المعرّف النصي: welcome"
        );
        assert_eq!(
            slug_message(Some("en"), "forum.topic.absentKey", "{slug}", "welcome"),
            "welcome"
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
