mod graphql_adapter;
mod moderation_adapter;
mod native_server_adapter;

use crate::model::{
    BlogModerationCommentList, BlogModerationStatus, BlogPostDetail, BlogPostDraft, BlogPostList,
};
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};

pub type ApiError = UiTransportError;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<BlogPostList, ApiError> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/posts",
        selected_transport_path(),
        move || native_server_adapter::fetch_posts(native_locale),
        move || graphql_adapter::fetch_posts(token, tenant_slug, locale),
    )
    .await
}

pub fn is_posts_contract_unavailable(error: &ApiError) -> bool {
    error.failed_path == UiTransportPath::Graphql
        && error
            .graphql_error
            .as_deref()
            .is_some_and(graphql_adapter::is_posts_contract_unavailable_message)
}

pub async fn fetch_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, ApiError> {
    let native_id = id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/post",
        selected_transport_path(),
        move || native_server_adapter::fetch_post(native_id, native_locale),
        move || graphql_adapter::fetch_post(token, tenant_slug, id, locale),
    )
    .await
}

pub async fn create_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ApiError> {
    let native_draft = draft.clone();
    execute_selected_transport(
        "blog/admin/create-post",
        selected_transport_path(),
        move || native_server_adapter::create_post(native_draft),
        move || graphql_adapter::create_post(token, tenant_slug, draft),
    )
    .await
}

pub async fn update_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ApiError> {
    let native_id = id.clone();
    let native_draft = draft.clone();
    execute_selected_transport(
        "blog/admin/update-post",
        selected_transport_path(),
        move || native_server_adapter::update_post(native_id, native_draft),
        move || graphql_adapter::update_post(token, tenant_slug, id, draft),
    )
    .await
}

pub async fn publish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let native_id = id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/publish-post",
        selected_transport_path(),
        move || native_server_adapter::publish_post(native_id, native_locale),
        move || graphql_adapter::publish_post(token, tenant_slug, id, locale),
    )
    .await
}

pub async fn unpublish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let native_id = id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/unpublish-post",
        selected_transport_path(),
        move || native_server_adapter::unpublish_post(native_id, native_locale),
        move || graphql_adapter::unpublish_post(token, tenant_slug, id, locale),
    )
    .await
}

pub async fn archive_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let native_id = id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/archive-post",
        selected_transport_path(),
        move || native_server_adapter::archive_post(native_id, native_locale),
        move || graphql_adapter::archive_post(token, tenant_slug, id, locale),
    )
    .await
}

pub async fn delete_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let native_id = id.clone();
    execute_selected_transport(
        "blog/admin/delete-post",
        selected_transport_path(),
        move || native_server_adapter::delete_post(native_id),
        move || graphql_adapter::delete_post(token, tenant_slug, id),
    )
    .await
}

pub async fn fetch_moderation_comments(
    token: Option<String>,
    tenant_slug: Option<String>,
    post_id: String,
    locale: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<BlogModerationCommentList, ApiError> {
    let native_post_id = post_id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/moderation-comments",
        selected_transport_path(),
        move || {
            native_server_adapter::fetch_moderation_comments(
                native_post_id,
                native_locale,
                page,
                per_page,
            )
        },
        move || {
            moderation_adapter::fetch_comments(token, tenant_slug, post_id, locale, page, per_page)
        },
    )
    .await
}

pub async fn moderate_comment(
    token: Option<String>,
    tenant_slug: Option<String>,
    comment_id: String,
    status: BlogModerationStatus,
    locale: Option<String>,
) -> Result<bool, ApiError> {
    let native_comment_id = comment_id.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog/admin/moderate-comment",
        selected_transport_path(),
        move || native_server_adapter::moderate_comment(native_comment_id, status, native_locale),
        move || {
            moderation_adapter::moderate_comment(token, tenant_slug, comment_id, status, locale)
        },
    )
    .await
}

pub fn is_moderation_contract_unavailable(error: &ApiError) -> bool {
    error.failed_path == UiTransportPath::Graphql
        && error
            .graphql_error
            .as_deref()
            .is_some_and(moderation_adapter::is_contract_unavailable_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_selects_graphql_without_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
