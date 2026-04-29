#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{BlogPostDetail, BlogPostDraft, BlogPostList};

pub type ApiError = GraphqlHttpError;

const BLOG_POSTS_QUERY: &str = "query BlogPostsAdmin($filter: PostsFilter) { posts(filter: $filter) { total items { id title effectiveLocale slug excerpt status createdAt publishedAt } } }";
const BLOG_POST_QUERY: &str = "query BlogPostAdmin($id: UUID!, $locale: String) { post(id: $id, locale: $locale) { id requestedLocale effectiveLocale availableLocales title slug excerpt body bodyFormat contentJson status createdAt updatedAt publishedAt tags featuredImageUrl seoTitle seoDescription } }";
const CREATE_POST_MUTATION: &str =
    "mutation CreatePost($input: CreatePostInput!) { createPost(input: $input) }";
const UPDATE_POST_MUTATION: &str = "mutation UpdatePost($id: UUID!, $input: UpdatePostInput!) { updatePost(id: $id, input: $input) }";
const PUBLISH_POST_MUTATION: &str = "mutation PublishPost($id: UUID!) { publishPost(id: $id) }";
const UNPUBLISH_POST_MUTATION: &str =
    "mutation UnpublishPost($id: UUID!) { unpublishPost(id: $id) }";
const ARCHIVE_POST_MUTATION: &str =
    "mutation ArchivePost($id: UUID!, $reason: String) { archivePost(id: $id, reason: $reason) }";
const DELETE_POST_MUTATION: &str = "mutation DeletePost($id: UUID!) { deletePost(id: $id) }";

#[derive(Debug, Deserialize)]
struct BlogPostsResponse {
    posts: BlogPostList,
}

#[derive(Debug, Deserialize)]
struct BlogPostResponse {
    post: Option<BlogPostDetail>,
}

#[derive(Debug, Deserialize)]
struct CreatePostResponse {
    #[serde(rename = "createPost")]
    create_post: String,
}

#[derive(Debug, Deserialize)]
struct BoolMutationResponse {
    #[serde(default, rename = "updatePost")]
    update_post: bool,
    #[serde(default, rename = "publishPost")]
    publish_post: bool,
    #[serde(default, rename = "unpublishPost")]
    unpublish_post: bool,
    #[serde(default, rename = "archivePost")]
    archive_post: bool,
    #[serde(default, rename = "deletePost")]
    delete_post: bool,
}

#[derive(Debug, Serialize)]
struct BlogPostsVariables {
    filter: PostsFilter,
}

#[derive(Debug, Serialize)]
struct PostsFilter {
    locale: Option<String>,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

#[derive(Debug, Serialize)]
struct PostVariables {
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatePostVariables {
    input: CreatePostInput,
}

#[derive(Debug, Serialize)]
struct UpdatePostVariables {
    id: String,
    input: UpdatePostInput,
}

#[derive(Debug, Serialize)]
struct CreatePostInput {
    locale: String,
    title: String,
    body: String,
    #[serde(rename = "bodyFormat")]
    body_format: String,
    excerpt: Option<String>,
    slug: Option<String>,
    publish: bool,
    tags: Vec<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    #[serde(rename = "featuredImageUrl")]
    featured_image_url: Option<String>,
    #[serde(rename = "seoTitle")]
    seo_title: Option<String>,
    #[serde(rename = "seoDescription")]
    seo_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdatePostInput {
    locale: Option<String>,
    title: Option<String>,
    body: Option<String>,
    #[serde(rename = "bodyFormat")]
    body_format: Option<String>,
    excerpt: Option<String>,
    slug: Option<String>,
    tags: Option<Vec<String>>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    #[serde(rename = "featuredImageUrl")]
    featured_image_url: Option<String>,
    #[serde(rename = "seoTitle")]
    seo_title: Option<String>,
    #[serde(rename = "seoDescription")]
    seo_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostIdVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct ArchivePostVariables {
    id: String,
    reason: Option<String>,
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

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<BlogPostList, ApiError> {
    let response: BlogPostsResponse = match request(
        BLOG_POSTS_QUERY,
        BlogPostsVariables {
            filter: PostsFilter {
                locale,
                page: 1,
                per_page: 20,
            },
        },
        token,
        tenant_slug,
    )
    .await
    {
        Ok(response) => response,
        Err(error) if is_posts_contract_unavailable(&error) => {
            return Ok(BlogPostList {
                items: Vec::new(),
                total: 0,
            });
        }
        Err(error) => return Err(error),
    };

    Ok(response.posts)
}

pub(crate) fn is_posts_contract_unavailable(error: &ApiError) -> bool {
    let message = error.to_string();
    message.contains("Unknown type \"PostsFilter\"") || message.contains("Unknown field \"posts\"")
}

pub async fn fetch_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, ApiError> {
    let response: BlogPostResponse = request(
        BLOG_POST_QUERY,
        PostVariables { id, locale },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.post)
}

pub async fn create_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ApiError> {
    let created: CreatePostResponse = request(
        CREATE_POST_MUTATION,
        CreatePostVariables {
            input: CreatePostInput {
                locale: draft.locale.clone(),
                title: draft.title,
                body: draft.body,
                body_format: draft.body_format,
                excerpt: optional_text(draft.excerpt),
                slug: optional_text(draft.slug),
                publish: draft.publish,
                tags: draft.tags,
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
            },
        },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;

    fetch_post(token, tenant_slug, created.create_post, Some(draft.locale))
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Created post could not be reloaded".into()))
}

pub async fn update_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ApiError> {
    let updated: BoolMutationResponse = request(
        UPDATE_POST_MUTATION,
        UpdatePostVariables {
            id: id.clone(),
            input: UpdatePostInput {
                locale: Some(draft.locale.clone()),
                title: Some(draft.title),
                body: Some(draft.body),
                body_format: Some(draft.body_format),
                excerpt: Some(draft.excerpt),
                slug: Some(draft.slug),
                tags: Some(draft.tags),
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
            },
        },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;

    if !updated.update_post {
        return Err(GraphqlHttpError::Graphql(
            "Update post returned false".into(),
        ));
    }

    fetch_post(token, tenant_slug, id, Some(draft.locale))
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Updated post could not be reloaded".into()))
}

pub async fn publish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let response: BoolMutationResponse = request(
        PUBLISH_POST_MUTATION,
        PostIdVariables { id: id.clone() },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;

    if !response.publish_post {
        return Err(GraphqlHttpError::Graphql(
            "Publish post returned false".into(),
        ));
    }

    fetch_post(token, tenant_slug, id, locale)
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Published post could not be reloaded".into()))
}

pub async fn unpublish_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let response: BoolMutationResponse = request(
        UNPUBLISH_POST_MUTATION,
        PostIdVariables { id: id.clone() },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;

    if !response.unpublish_post {
        return Err(GraphqlHttpError::Graphql(
            "Unpublish post returned false".into(),
        ));
    }

    fetch_post(token, tenant_slug, id, locale)
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Unpublished post could not be reloaded".into()))
}

pub async fn archive_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let response: BoolMutationResponse = request(
        ARCHIVE_POST_MUTATION,
        ArchivePostVariables {
            id: id.clone(),
            reason: Some("Archived from module admin package".to_string()),
        },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;

    if !response.archive_post {
        return Err(GraphqlHttpError::Graphql(
            "Archive post returned false".into(),
        ));
    }

    fetch_post(token, tenant_slug, id, locale)
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Archived post could not be reloaded".into()))
}

pub async fn delete_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        DELETE_POST_MUTATION,
        PostIdVariables { id },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.delete_post)
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
