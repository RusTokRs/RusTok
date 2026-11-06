#[cfg(feature = "ssr")]
pub mod feed;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod graphql_adapter;
pub mod native_server_adapter;

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::core::BlogStorefrontFetchRequest;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::model::StorefrontBlogData;
use crate::model::{BlogCommentCreateRequest, BlogCommentDetail, BlogPostDetail};
use leptos::prelude::ServerFnError;
use rustok_ui_transport::UiTransportError;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use rustok_ui_transport::UiTransportPath;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use rustok_ui_transport::execute_selected_transport;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub type BlogTransportError = UiTransportError;

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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

#[cfg(feature = "ssr")]
fn configured_fallback_tenant_slug(requested: Option<&str>) -> Result<String, ServerFnError> {
    let configured = configured_tenant_slug().ok_or_else(|| {
        ServerFnError::new("Blog storefront server function requires a configured tenant fallback")
    })?;
    let requested = requested.map(str::trim).filter(|value| !value.is_empty());
    if let Some(requested) = requested
        && requested != configured
    {
        return Err(ServerFnError::new(
            "Blog storefront tenant fallback does not match the configured host tenant",
        ));
    }
    Ok(configured)
}

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub(crate) fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub async fn fetch_blog(
    request: BlogStorefrontFetchRequest,
    comments_page: u64,
) -> Result<StorefrontBlogData, BlogTransportError> {
    let native_request = request.clone();
    execute_selected_transport(
        "blog",
        selected_transport_path(),
        move || native_server_adapter::fetch_blog(native_request, comments_page),
        move || graphql_adapter::fetch_blog(request, comments_page),
    )
    .await
}

pub async fn create_comment(
    token: Option<String>,
    request: BlogCommentCreateRequest,
) -> Result<BlogCommentDetail, BlogTransportError> {
    #[cfg(all(feature = "comment-island", not(feature = "ssr")))]
    {
        let _ = token;
        return native_server_adapter::create_comment(request)
            .await
            .map_err(|error| UiTransportError::native("blog_comment_create", error));
    }
    #[cfg(any(feature = "ssr", not(feature = "comment-island")))]
    {
        let native_request = request.clone();
        execute_selected_transport(
            "blog_comment_create",
            selected_transport_path(),
            move || native_server_adapter::create_comment(native_request),
            move || graphql_adapter::create_comment(token, request),
        )
        .await
    }
}

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub async fn fetch_post_by_preview_token(
    token: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, BlogTransportError> {
    let native_token = token.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "blog_preview_post",
        selected_transport_path(),
        move || native_server_adapter::fetch_post_by_preview_token(native_token, native_locale),
        move || graphql_adapter::fetch_post_by_preview_token(token, locale),
    )
    .await
}

/// Compose owner feed and Search IO behind the module transport boundary.
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub async fn fetch_blog_page(
    request: crate::core::BlogPageRequest,
) -> Result<(StorefrontBlogData, Option<u64>, usize), String> {
    let mut data = fetch_blog(request.feed.clone(), request.comments_page)
        .await
        .map_err(|error| error.to_string())?;
    let Some(query) = request.query else {
        return Ok((data, None, request.search_page));
    };
    let filters = crate::core::blog_search_filters(&request.feed);
    let result = rustok_search_storefront::transport::fetch_search_page(
        query,
        request.feed.locale,
        None,
        filters,
        request.feed.tag,
        crate::core::BLOG_SEARCH_PAGE_SIZE,
        (request.search_page - 1) * crate::core::BLOG_SEARCH_PAGE_SIZE,
    )
    .await
    .map_err(|error| error.to_string())?;
    let total = result.total;
    data.posts = crate::core::map_blog_search_results(result)?;
    Ok((data, Some(total), request.search_page))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
        #[cfg(any(feature = "ssr", feature = "hydrate"))]
        assert_eq!(selected_transport_path(), UiTransportPath::NativeServer);
    }
}
