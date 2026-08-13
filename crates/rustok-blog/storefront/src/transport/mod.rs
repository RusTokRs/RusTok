#[cfg(not(feature = "comment-island"))]
pub mod graphql_adapter;
pub mod native_server_adapter;

#[cfg(not(feature = "comment-island"))]
use crate::core::BlogStorefrontFetchRequest;
#[cfg(not(feature = "comment-island"))]
use crate::model::StorefrontBlogData;
use crate::model::{BlogCommentCreateRequest, BlogCommentDetail};
use leptos::prelude::ServerFnError;
use rustok_ui_transport::UiTransportError;
#[cfg(not(feature = "comment-island"))]
use rustok_ui_transport::UiTransportPath;
#[cfg(not(feature = "comment-island"))]
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

#[cfg(not(feature = "comment-island"))]
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

#[cfg(not(feature = "comment-island"))]
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

#[cfg(not(feature = "comment-island"))]
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
    #[cfg(feature = "comment-island")]
    {
        let _ = token;
        return native_server_adapter::create_comment(request)
            .await
            .map_err(|error| UiTransportError::native("blog_comment_create", error));
    }
    #[cfg(not(feature = "comment-island"))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
