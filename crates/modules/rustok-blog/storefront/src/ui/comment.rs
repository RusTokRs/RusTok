use crate::{
    i18n::{comment_composer_copy, t},
    transport,
};
use leptos::prelude::*;
use leptos_ui::{Button, ButtonVariant, Size};
use rustok_auth_ui::ui::leptos::hooks::use_token;
use rustok_comments_storefront_support::CommentComposer;
use rustok_reactions_storefront::ReactionBar;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn BlogDiscussion(props: crate::model::BlogDiscussionProps) -> impl IntoView {
    let mut route = use_context::<UiRouteContext>().unwrap_or_default();
    route.locale = props.ui_locale.clone();
    provide_context(route);
    let reaction = crate::core::discussion::reaction_subject(&props);
    let (parent, set_parent) = signal(None::<String>);
    let token = use_token();
    let locale = props.ui_locale.clone();
    let copy = comment_composer_copy(locale.as_deref());
    let post_id = props.post_id;
    let content_locale = props.content_locale.clone();
    let comments_available =
        crate::core::discussion::comments_view(&props.comments, props.comments_page).can_reply;
    let submit = Action::new_local(move |content: &rustok_api::RichTextDocument| {
        let request = crate::core::discussion::prepare_comment(
            &post_id,
            &content_locale,
            parent.get_untracked(),
            content.clone(),
        );
        let token = token.get_untracked();
        async move {
            transport::create_comment(token, request)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
    });
    Effect::new(move |_| {
        if matches!(submit.value().get(), Some(Ok(()))) {
            set_parent.set(None);
        }
    });
    let cancel_label = t(
        locale.as_deref(),
        "blog.comments.cancelReply",
        "Cancel reply",
    );
    let reply_label = t(
        locale.as_deref(),
        "blog.comments.replying",
        "Replying to a comment",
    );
    let on_reply = Callback::new(move |id: String| set_parent.set(Some(id)));
    view! {
        <section class="space-y-6">
            {match reaction {
                Ok(subject) => view! { <ReactionBar subject /> }.into_any(),
                Err(_) => view! { <p role="status">{t(locale.as_deref(), "blog.reactions.invalid", "Reactions are unavailable for this article.")}</p> }.into_any(),
            }}
            {comments_available.then(|| view! {
                <section id="comment-composer-box" class="space-y-3">
                    {move || parent.get().map(|_| {
                        let cancel_label = cancel_label.clone();
                        view! {
                        <div role="status" class="flex items-center gap-2">
                            <span>{reply_label.clone()}</span>
                            <Button size=Size::Sm variant=ButtonVariant::Outline on_click=Box::new(move || set_parent.set(None))>{cancel_label.clone()}</Button>
                        </div>
                        }
                    })}
                    <CommentComposer content_locale=props.content_locale submit_action=submit copy />
                </section>
            })}
            <super::comments_list::PublicCommentsList comments=props.comments comments_page=props.comments_page on_reply=on_reply />
        </section>
    }
}
