use rustok_comments_storefront_support::CommentComposerCopy;
rustok_ui_i18n::declare_module_i18n!();

pub fn comment_composer_copy(locale: Option<&str>) -> CommentComposerCopy {
    CommentComposerCopy {
        title: t(
            locale,
            "blog.comments.composer.title",
            "Join the discussion",
        ),
        editor_label: t(locale, "blog.comments.composer.editorLabel", "Comment"),
        hint: t(
            locale,
            "blog.comments.composer.hint",
            "Formatting is preserved with the shared richtext editor.",
        ),
        submit: t(locale, "blog.comments.composer.submit", "Submit comment"),
        submitting: t(locale, "blog.comments.composer.submitting", "Submitting..."),
        success: t(
            locale,
            "blog.comments.composer.success",
            "Your comment was submitted for moderation.",
        ),
        sign_in_required: t(
            locale,
            "blog.comments.composer.signInRequired",
            "Sign in to join the discussion.",
        ),
        empty_error: t(
            locale,
            "blog.comments.composer.emptyError",
            "Write a comment before submitting.",
        ),
        richtext: leptos_ui::localized_richtext_frame_copy(|key, fallback| {
            t(locale, key, fallback)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn visible(text: String) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }
    #[test]
    fn reading_time_uses_catalog_plural_rules_and_regional_locale() {
        for (minutes, expected) in [
            (1, "1 минута чтения"),
            (2, "2 минуты чтения"),
            (5, "5 минут чтения"),
            (11, "11 минут чтения"),
            (21, "21 минута чтения"),
        ] {
            assert_eq!(
                visible(crate::core::format_reading_time(minutes, Some("ru-RU"))),
                expected
            );
        }
        let french = rustok_ui_i18n::UiMessages::new(
            "fr",
            &[("fr", "blog-readingTime = { $minutes } minutes de lecture")],
        );
        let mut args = rustok_ui_i18n::FluentArgs::new();
        args.set("minutes", 3);
        assert_eq!(
            visible(french.format(Some("fr"), "blog.readingTime", Some(&args), "")),
            "3 minutes de lecture"
        );
    }
}

pub fn search_results_message(locale: Option<&str>, page: usize, total: u64) -> String {
    let mut args = rustok_ui_i18n::FluentArgs::new();
    args.set("page", page.to_string());
    args.set("total", total as f64);
    format(locale, "blog.search.results", Some(&args), "")
}
