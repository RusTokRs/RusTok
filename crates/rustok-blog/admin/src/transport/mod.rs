mod graphql_adapter;

use crate::model::{BlogPostDetail, BlogPostDraft, BlogPostList};
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
