use leptos::prelude::*;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::leptos::ForumView as ForumViewInner;

fn forum_content_locale(locale: Option<&str>) -> String {
    locale
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| "und".to_string())
}

/// Public Forum storefront boundary for localized business content.
///
/// The route locale is normalized once for the HTML language contract while
/// browser-native bidi resolution remains authoritative for mixed-direction
/// category, topic, tag, and reply content. Rich text keeps its narrower
/// per-resource locale boundary inside `RichTextHtml`.
#[component]
pub fn ForumView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let content_locale = forum_content_locale(route_context.locale.as_deref());

    view! {
        <div
            data-forum-content-locale=""
            lang=content_locale
            dir="auto"
        >
            <ForumViewInner />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::forum_content_locale;

    #[test]
    fn content_locale_uses_canonical_route_locale() {
        assert_eq!(forum_content_locale(Some(" ar_sa ")), "ar-SA");
        assert_eq!(forum_content_locale(Some("he")), "he");
    }

    #[test]
    fn missing_or_invalid_route_locale_falls_back_to_unknown_language() {
        assert_eq!(forum_content_locale(None), "und");
        assert_eq!(forum_content_locale(Some("")), "und");
        assert_eq!(forum_content_locale(Some("not a locale")), "und");
    }
}
