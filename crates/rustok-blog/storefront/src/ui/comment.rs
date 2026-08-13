use leptos::prelude::*;
use leptos_auth::hooks::use_token;
use rustok_comments_storefront_support::CommentComposer;

use crate::{i18n::comment_composer_copy, model::BlogCommentCreateRequest, transport};

#[component]
pub fn BlogCommentComposer(post_id: String, content_locale: String) -> impl IntoView {
    let token = use_token();
    let comment_locale = content_locale.clone();
    let copy = comment_composer_copy(Some(content_locale.as_str()));
    let action = Action::new_local(move |content: &rustok_api::RichTextDocument| {
        let request = BlogCommentCreateRequest::for_post(
            post_id.clone(),
            comment_locale.clone(),
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

    view! {
        <CommentComposer
            content_locale
            submit_action=action
            copy
        />
    }
}
