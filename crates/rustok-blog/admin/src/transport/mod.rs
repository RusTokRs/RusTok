mod graphql_adapter;
mod moderation_adapter;

use crate::model::{
    BlogModerationCommentList, BlogModerationStatus, BlogPostDetail, BlogPostDraft, BlogPostList,
};
pub use graphql_adapter::ApiError;

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<BlogPostList, ApiError> {
    graphql_adapter::fetch_posts(token, tenant_slug, locale).await
}

pub fn is_posts_contract_unavailable(error: &ApiError) -> bool {
    graphql_adapter::is_posts_contract_unavailable(error)
}

pub async fn fetch_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, ApiError> {
    graphql_adapter::fetch_post(token, tenant_slug, id, locale).await
}

pub async fn create_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ApiError> {
    graphql_adapter::create_post(token, tenant_slug, draft).await
}

pub async fn update_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ApiError> {
    graphql_adapter::update_post(token, tenant_slug, id, draft).await
}

pub async fn publish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    graphql_adapter::publish_post(token, tenant_slug, id, locale).await
}

pub async fn unpublish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    graphql_adapter::unpublish_post(token, tenant_slug, id, locale).await
}

pub async fn archive_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    graphql_adapter::archive_post(token, tenant_slug, id, locale).await
}

pub async fn delete_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    graphql_adapter::delete_post(token, tenant_slug, id).await
}

pub async fn fetch_moderation_comments(
    token: Option<String>,
    tenant_slug: Option<String>,
    post_id: String,
    locale: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<BlogModerationCommentList, ApiError> {
    moderation_adapter::fetch_comments(
        token,
        tenant_slug,
        post_id,
        locale,
        page,
        per_page,
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
    moderation_adapter::moderate_comment(token, tenant_slug, comment_id, status, locale).await
}

pub fn is_moderation_contract_unavailable(error: &ApiError) -> bool {
    moderation_adapter::is_contract_unavailable(error)
}
