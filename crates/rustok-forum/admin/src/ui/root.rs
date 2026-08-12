use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use super::topic_fork::ForumTopicForkAdmin;
use super::topic_merge::ForumTopicMergeAdmin;
use super::topic_reply_range::ForumTopicReplyRangeAdmin;
use super::topic_slug_rename::ForumTopicSlugRenameAdmin;
use super::topic_split::ForumTopicSplitAdmin;

#[component]
pub fn ForumAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    if route_context.subpath_matches("reply-range") {
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
    }
}
