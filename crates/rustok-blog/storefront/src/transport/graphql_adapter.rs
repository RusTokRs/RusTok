use crate::comments_pagination::{COMMENTS_PAGE_SIZE, bounded_comments_request_page};
use crate::core::BlogStorefrontFetchRequest;
use crate::model::{
    BlogCommentCreateRequest, BlogCommentDetail, BlogPostDetail, BlogPostList, StorefrontBlogData,
};
use rustok_api::RichTextDocument;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};

const STOREFRONT_BLOG_QUERY: &str = "query StorefrontBlog($postSlug: String!, $filter: PostsFilter, $locale: String, $commentsPage: Int!, $commentsPerPage: Int!) { selectedPost: postBySlug(slug: $postSlug, locale: $locale) { id effectiveLocale title slug excerpt content { document html } contentPlainText status publishedAt tags featuredImageUrl publicComments(locale: $locale, page: $commentsPage, perPage: $commentsPerPage) { availability cachedSnapshot total items { id effectiveLocale authorId contentPreview parentCommentId createdAt } } } posts(filter: $filter) { total items { id title effectiveLocale slug excerpt status publishedAt } } }";
const CREATE_BLOG_COMMENT_MUTATION: &str = "mutation CreateBlogComment($postId: UUID!, $input: CreateBlogCommentInput!) { createBlogComment(postId: $postId, input: $input) { id requestedLocale effectiveLocale postId authorId content { document html } contentPlainText status parentCommentId createdAt updatedAt } }";

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

#[derive(Debug, Deserialize)]
struct CreateBlogCommentResponse {
    #[serde(rename = "createBlogComment")]
    create_blog_comment: BlogCommentDetail,
}

#[derive(Debug, Serialize)]
struct CreateBlogCommentVariables {
    #[serde(rename = "postId")]
    post_id: String,
    input: CreateBlogCommentInput,
}

#[derive(Debug, Serialize)]
struct CreateBlogCommentInput {
    locale: String,
    content: RichTextDocument,
    #[serde(rename = "parentCommentId")]
    parent_comment_id: Option<String>,
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

async fn request<V, T>(query: &str, variables: V, token: Option<String>) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
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
            comments_page: bounded_comments_request_page(comments_page),
            comments_per_page: COMMENTS_PAGE_SIZE,
        },
        None,
    )
    .await?;

    Ok(StorefrontBlogData {
        selected_post: response.selected_post,
        posts: response.posts,
    })
}

pub async fn create_comment(
    token: Option<String>,
    request_data: BlogCommentCreateRequest,
) -> Result<BlogCommentDetail, ApiError> {
    let response: CreateBlogCommentResponse = request(
        CREATE_BLOG_COMMENT_MUTATION,
        CreateBlogCommentVariables {
            post_id: request_data.post_id,
            input: CreateBlogCommentInput {
                locale: request_data.locale,
                content: request_data.content,
                parent_comment_id: request_data.parent_comment_id,
            },
        },
        token,
    )
    .await?;

    Ok(response.create_blog_comment)
}
