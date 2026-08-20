use leptos::prelude::*;
use rustok_api::normalize_locale_tag;
use rustok_ui_core::UiRouteContext;

use super::topic_fork::ForumTopicForkAdmin;
use super::topic_merge::ForumTopicMergeAdmin;
use super::topic_reply_range::ForumTopicReplyRangeAdmin;
use super::topic_slug_rename::ForumTopicSlugRenameAdmin;
use super::topic_split::ForumTopicSplitAdmin;

fn forum_admin_ui_locale(locale: Option<&str>) -> String {
    locale
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| "und".to_string())
}

#[component]
pub fn ForumAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = forum_admin_ui_locale(route_context.locale.as_deref());
    let content = if route_context.subpath_matches("reply-range") {
        view! { <ForumTopicReplyRangeAdmin /> }.into_any()
    } else if route_context.subpath_matches("rename-slug") {
        view! { <ForumTopicSlugRenameAdmin /> }.into_any()
    } else if route_context.subpath_matches("fork") {
        view! { <ForumTopicForkAdmin /> }.into_any()
    } else if route_context.subpath_matches("split") {
        view! { <ForumTopicSplitAdmin /> }.into_any()
    } else if route_context.subpath_matches("merge") {
        view! { <ForumTopicMergeAdmin /> }.into_any()
    } else {
        view! { <super::leptos::ForumAdmin /> }.into_any()
    };

    view! {
        <div data-forum-admin-locale="" lang=ui_locale dir="auto">
            {content}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::forum_admin_ui_locale;

    #[test]
    fn admin_ui_locale_uses_the_shared_canonical_locale_contract() {
        assert_eq!(forum_admin_ui_locale(Some(" ar_sa ")), "ar-SA");
        assert_eq!(forum_admin_ui_locale(Some("pt_br")), "pt-BR");
    }

    #[test]
    fn admin_ui_locale_falls_back_to_unknown_for_missing_or_invalid_input() {
        assert_eq!(forum_admin_ui_locale(None), "und");
        assert_eq!(forum_admin_ui_locale(Some("not a locale")), "und");
    }
}
