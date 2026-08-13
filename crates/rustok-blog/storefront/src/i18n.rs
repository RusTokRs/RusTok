use rustok_comments_storefront_support::CommentComposerCopy;
use rustok_ui_i18n_leptos::LeptosUiMessages;

static MESSAGES: LeptosUiMessages = LeptosUiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.json")),
        ("ru", include_str!("../locales/ru.json")),
    ],
);

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    MESSAGES.t_for_locale(locale, key, fallback)
}

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
