rustok_ui_i18n::declare_module_i18n!(
    "en",
    &[
        ("en", include_str!("../locales/en.ftl")),
        ("ru", include_str!("../locales/ru.ftl")),
        ("ar", include_str!("../locales/ar.ftl")),
    ]
);

/// Localized message that interpolates a count, for example `3 shown`.
///
/// The no-argument `t` family cannot serve these keys: `fluent-bundle` records a reference error for
/// every unbound placeable, the shared resolver treats any error as a failed lookup, and the caller
/// literal wins - so the catalog text never renders and every locale shows the English fallback.
/// Binding the value as a Fluent argument is the only way to localize these labels. The trailing
/// placeholder substitution keeps the caller literal usable for a locale that has no such message.
pub fn count_message<Count>(locale: Option<&str>, key: &str, fallback: &str, count: Count) -> String
where
    Count: Into<rustok_ui_i18n::FluentValue<'static>> + ToString,
{
    let count_text = count.to_string();
    // The `'static` annotation is what makes the bound usable: `FluentArgs` is invariant in its
    // argument lifetime, so an inferred region would ask for an obligation no caller bound can meet.
    let mut arguments: rustok_ui_i18n::FluentArgs<'static> = rustok_ui_i18n::FluentArgs::new();
    arguments.set("count", count);
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::forum_admin_count_label(resolved.as_str(), count_text)
}

/// Localized label for one free-form value, for example `icon: chat`.
///
/// Resolved with a bound argument for the same reason as [`count_message`]; the borrowed value is
/// threaded through `FluentArgs<'a>` so nothing is copied just to satisfy a `'static` bound.
pub fn value_message<'a>(
    locale: Option<&str>,
    key: &str,
    fallback: &str,
    value: &'a str,
) -> String {
    let mut arguments: rustok_ui_i18n::FluentArgs<'a> = rustok_ui_i18n::FluentArgs::new();
    arguments.set("value", value);
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::forum_admin_value_label(resolved.as_str(), value)
}

/// Localized thread path, which interpolates a category id and a slug into one message.
///
/// Two variables make a hand-rolled template worse than the other labels here: a caller that
/// substitutes only one of them leaves a literal `{slug}` in the panel.
pub fn thread_path_message<'a>(
    locale: Option<&str>,
    key: &str,
    fallback: &str,
    category: &'a str,
    slug: &'a str,
) -> String {
    let mut arguments: rustok_ui_i18n::FluentArgs<'a> = rustok_ui_i18n::FluentArgs::new();
    arguments.set("category", category);
    arguments.set("slug", slug);
    let resolved = format(locale, key, Some(&arguments), fallback);
    crate::core::forum_admin_thread_path_label(resolved.as_str(), category, slug)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{count_message, t, thread_path_message, value_message};

    /// The shared bundle resolves with bidi isolation on, so an interpolated value is wrapped in
    /// invisible FSI/PDI marks. Assertions compare the visible text.
    fn visible(text: &str) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

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

    #[test]
    fn count_message_reaches_the_catalog_instead_of_the_caller_literal() {
        // `t` on the same key cannot bind `$count`, so it returns the English fallback for a Russian
        // reader; a bound argument is what makes the localized form render.
        assert_eq!(
            visible(
                count_message(Some("en"), "forum.topics.shown", "{count} shown", 1_usize).as_str()
            ),
            "1 shown"
        );
        assert_eq!(
            visible(
                count_message(Some("ru"), "forum.topics.shown", "{count} shown", 7_usize).as_str()
            ),
            "показано: 7"
        );
    }

    #[test]
    fn value_and_path_messages_bind_every_variable() {
        assert_eq!(
            visible(
                value_message(Some("en"), "forum.render.icon", "icon: {value}", " chat ").as_str()
            ),
            "icon:  chat "
        );
        assert_eq!(
            visible(
                thread_path_message(
                    Some("ru"),
                    "forum.render.threadPath",
                    "thread/{category}/{slug}",
                    "category-1",
                    "welcome"
                )
                .as_str()
            ),
            "thread/category-1/welcome"
        );
    }

    #[test]
    fn resolvers_keep_the_caller_literal_usable_for_unknown_keys() {
        assert_eq!(
            count_message(
                Some("en"),
                "forum.topics.absentKey",
                "{count} left",
                2_usize
            ),
            "2 left"
        );
        assert_eq!(
            value_message(
                Some("en"),
                "forum.render.absentKey",
                "icon: {value}",
                "star"
            ),
            "icon: star"
        );
    }
}
