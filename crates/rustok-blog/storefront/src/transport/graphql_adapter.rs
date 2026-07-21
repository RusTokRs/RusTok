use crate::comments_pagination::COMMENTS_PAGE_SIZE;
use crate::core::BlogStorefrontFetchRequest;
use crate::model::{BlogPostDetail, BlogPostList, StorefrontBlogData};
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use super::{configured_tenant_slug, ApiError};

const STOREFRONT_BLOG_QUERY: &str = "query StorefrontBlog($postSlug: String!, $filter: PostsFilter, $locale: String, $commentsPage: Int!, $commentsPerPage: Int!) { selectedPost: postBySlug(slug: $postSlug, locale: $locale) { id effectiveLocale title slug excerpt body bodyFormat status publishedAt tags featuredImageUrl publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage) { total items { id effectiveLocale authorId contentPreview parentCommentId createdAt } } } posts(filter: $filter) { total items { id title effectiveLocale slug excerpt status publishedAt } } }";

#[derive(Debug, Deserialize)]
struct StorefrontBlogResponse {
    #[serde(rename = "selectedPost")]
    selected_post: Option<BlogPostDetail>,
    posts: BlogPostList,
}

#[derive(Debug, Serialize)]
struct StorefrontBlogVariables {
    #[serde(rename = "postSlug")]
    post_slug: String,
    filter: PostsFilter,
    locale: Option<String>,
    #[serde(rename = "commentsPage")]
    comments_page: u64,
    #[serde(rename = "commentsPerPage")]
    comments_per_page: u64,
}

#[derive(Clone, Debug, Serialize)]
struct PostsFilter {
    status: Option<String>,
    locale: Option<String>,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn fetch_blog(
    fetch_request: BlogStorefrontFetchRequest,
    comments_page: u64,
) -> Result<StorefrontBlogData, ApiError> {
    let response: StorefrontBlogResponse = request(
        STOREFRONT_BLOG_QUERY,
        StorefrontBlogVariables {
            post_slug: fetch_request.post_slug,
            filter: PostsFilter {
                status: Some("PUBLISHED".to_string()),
                locale: fetch_request.locale.clone(),
                page: 1,
                per_page: 6,
            },
            locale: fetch_request.locale,
            comments_page: comments_page.max(1),
            comments_per_page: COMMENTS_PAGE_SIZE,
        },
    )
    .await?;

    Ok(StorefrontBlogData {
        selected_post: response.selected_post,
        posts: response.posts,
    })
}
