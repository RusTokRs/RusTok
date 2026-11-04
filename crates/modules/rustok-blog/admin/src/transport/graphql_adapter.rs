use rustok_api::RichTextDocument;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::core;
use crate::model::{
    BlogCategoryDetail, BlogCategoryDraft, BlogCategoryOption, BlogCategoryUpdate, BlogPostDetail,
    BlogPostDraft, BlogPostList, BlogPreviewToken,
};

pub type ApiError = GraphqlHttpError;

const BLOG_CATEGORIES_QUERY: &str = "query BlogCategoriesAdmin($filter: BlogCategoriesFilter) { blogCategories(filter: $filter) { items { id name } total } }";
const BLOG_POSTS_QUERY: &str = "query BlogPostsAdmin($filter: PostsFilter) { posts(filter: $filter) { nextCursor items { id title effectiveLocale slug excerpt status authorId authorProfile { displayName } createdAt publishedAt scheduledAt isPinned pinnedAt } } }";
const BLOG_POST_QUERY: &str = "query BlogPostAdmin($id: UUID!, $locale: String) { post(id: $id, locale: $locale) { id requestedLocale effectiveLocale availableLocales title slug excerpt content { document html } contentPlainText status createdAt updatedAt publishedAt scheduledAt isPinned pinnedAt tags categoryId channelSlugs featuredImageUrl seoTitle seoDescription version } }";
const CREATE_POST_MUTATION: &str =
    "mutation CreatePost($input: CreatePostInput!) { createPost(input: $input) }";
const UPDATE_POST_MUTATION: &str = "mutation UpdatePost($id: UUID!, $input: UpdatePostInput!) { updatePost(id: $id, input: $input) }";
const PUBLISH_POST_MUTATION: &str = "mutation PublishPost($id: UUID!) { publishPost(id: $id) }";
const UNPUBLISH_POST_MUTATION: &str =
    "mutation UnpublishPost($id: UUID!) { unpublishPost(id: $id) }";
const PIN_POST_MUTATION: &str = "mutation PinPost($id: UUID!) { pinPost(id: $id) }";
const UNPIN_POST_MUTATION: &str = "mutation UnpinPost($id: UUID!) { unpinPost(id: $id) }";
const ARCHIVE_POST_MUTATION: &str =
    "mutation ArchivePost($id: UUID!, $reason: String) { archivePost(id: $id, reason: $reason) }";
const RESTORE_POST_MUTATION: &str = "mutation RestorePost($id: UUID!) { restorePost(id: $id) }";
const DELETE_POST_MUTATION: &str = "mutation DeletePost($id: UUID!) { deletePost(id: $id) }";
const CREATE_PREVIEW_TOKEN_MUTATION: &str = "mutation CreatePreviewToken($input: CreatePreviewTokenInput!) { createPreviewToken(input: $input) { id token postId expiresAt createdAt } }";
const BLOG_CATEGORY_DETAILS_QUERY: &str = "query BlogCategoryDetailsAdmin($filter: BlogCategoriesFilter) { blogCategories(filter: $filter) { items { id locale effectiveLocale name slug description parentId position createdAt updatedAt } total } }";
const CREATE_BLOG_CATEGORY_MUTATION: &str =
    "mutation CreateBlogCategory($input: CreateBlogCategoryInput!) { createBlogCategory(input: $input) }";
const UPDATE_BLOG_CATEGORY_MUTATION: &str = "mutation UpdateBlogCategory($id: UUID!, $input: UpdateBlogCategoryInput!) { updateBlogCategory(id: $id, input: $input) { id locale effectiveLocale name slug description parentId position createdAt updatedAt } }";
const DELETE_BLOG_CATEGORY_MUTATION: &str =
    "mutation DeleteBlogCategory($id: UUID!) { deleteBlogCategory(id: $id) }";

#[derive(Debug, Deserialize)]
struct BlogPostsResponse {
    posts: BlogPostList,
}

#[derive(Debug, Deserialize)]
struct BlogCategoriesResponse {
    #[serde(rename = "blogCategories")]
    blog_categories: BlogCategoryPage,
}
#[derive(Debug, Deserialize)]
struct BlogCategoryPage {
    items: Vec<BlogCategoryOption>,
    total: u64,
}
#[derive(Debug, Serialize)]
struct CategoryFilter {
    locale: Option<String>,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}
#[derive(Debug, Serialize)]
struct CategoryVariables {
    filter: CategoryFilter,
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
    #[serde(default, rename = "pinPost")]
    pin_post: bool,
    #[serde(default, rename = "unpinPost")]
    unpin_post: bool,
    #[serde(default, rename = "archivePost")]
    archive_post: bool,
    #[serde(default, rename = "restorePost")]
    restore_post: bool,
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
    after: Option<String>,
    #[serde(rename = "perPage")]
    per_page: u64,
    /// Optional in the schema, always sent by the admin: the sort is part of the
    /// request, not a client-side reorder of the loaded rows.
    #[serde(rename = "sortBy")]
    sort_by: crate::model::BlogPostSortField,
    #[serde(rename = "sortOrder")]
    sort_order: crate::model::BlogPostSortOrder,
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
    content: RichTextDocument,
    excerpt: Option<String>,
    slug: Option<String>,
    publish: bool,
    tags: Vec<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    #[serde(rename = "channelSlugs")]
    channel_slugs: Vec<String>,
    #[serde(rename = "featuredImageUrl")]
    featured_image_url: Option<String>,
    #[serde(rename = "seoTitle")]
    seo_title: Option<String>,
    #[serde(rename = "seoDescription")]
    seo_description: Option<String>,
    #[serde(rename = "scheduledAt")]
    scheduled_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdatePostInput {
    locale: Option<String>,
    title: Option<String>,
    content: Option<RichTextDocument>,
    excerpt: Option<String>,
    slug: Option<String>,
    tags: Option<Vec<String>>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    #[serde(rename = "channelSlugs")]
    channel_slugs: Vec<String>,
    #[serde(rename = "featuredImageUrl")]
    featured_image_url: Option<String>,
    #[serde(rename = "seoTitle")]
    seo_title: Option<String>,
    #[serde(rename = "seoDescription")]
    seo_description: Option<String>,
    #[serde(rename = "scheduledAt", skip_serializing_if = "Option::is_none")]
    scheduled_at: Option<Option<String>>,
    version: i32,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePreviewTokenInputPayload {
    post_id: String,
    ttl_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreatePreviewTokenVariables {
    input: CreatePreviewTokenInputPayload,
}

#[derive(Debug, Deserialize)]
struct CreatePreviewTokenResponse {
    #[serde(rename = "createPreviewToken")]
    create_preview_token: BlogPreviewToken,
}

#[derive(Debug, Deserialize)]
struct BlogCategoryDetailsResponse {
    #[serde(rename = "blogCategories")]
    blog_categories: BlogCategoryDetailPage,
}

#[derive(Debug, Deserialize)]
struct BlogCategoryDetailPage {
    items: Vec<BlogCategoryDetail>,
    total: u64,
}

#[derive(Debug, Serialize)]
struct CategoryDetailVariables {
    filter: CategoryFilter,
}

#[derive(Debug, Serialize)]
struct CreateBlogCategoryVariables {
    input: BlogCategoryDraft,
}

#[derive(Debug, Serialize)]
struct UpdateBlogCategoryVariables {
    id: String,
    input: BlogCategoryUpdate,
}

#[derive(Debug, Serialize)]
struct DeleteBlogCategoryVariables {
    id: String,
}

#[derive(Debug, Deserialize)]
struct CreateBlogCategoryResponse {
    #[serde(rename = "createBlogCategory")]
    create_blog_category: String,
}

#[derive(Debug, Deserialize)]
struct UpdateBlogCategoryResponse {
    #[serde(rename = "updateBlogCategory")]
    update_blog_category: BlogCategoryDetail,
}

#[derive(Debug, Default, Deserialize)]
struct DeleteBlogCategoryResponse {
    #[serde(default, rename = "deleteBlogCategory")]
    delete_blog_category: bool,
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

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<Vec<BlogCategoryOption>, ApiError> {
    let mut items = Vec::new();
    for page in 1.. {
        let result: BlogCategoriesResponse = request(
            BLOG_CATEGORIES_QUERY,
            CategoryVariables {
                filter: CategoryFilter {
                    locale: locale.clone(),
                    page,
                    per_page: 100,
                },
            },
            token.clone(),
            tenant_slug.clone(),
        )
        .await?;
        items.extend(result.blog_categories.items);
        if items.len() as u64 >= result.blog_categories.total {
            return Ok(items);
        }
        if (items.len() as u64) < page * 100 {
            return Err(GraphqlHttpError::Graphql(
                "Blog categories pagination returned incomplete rows".into(),
            ));
        }
    }
    unreachable!("Blog categories page count is bounded by owner taxonomy")
}

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
    after: Option<String>,
    sort: crate::core::BlogPostAdminSort,
) -> Result<BlogPostList, ApiError> {
    let response: BlogPostsResponse = request(
        BLOG_POSTS_QUERY,
        BlogPostsVariables {
            filter: PostsFilter {
                locale,
                after,
                per_page: 20,
                sort_by: sort.field,
                sort_order: sort.order,
            },
        },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.posts)
}

pub(crate) fn is_posts_contract_unavailable_message(message: &str) -> bool {
    message.contains("CONTRACT_UNAVAILABLE")
        || message.contains("Unknown type \"PostsFilter\"")
        || message.contains("Unknown field \"posts\"")
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
                content: draft.content,
                excerpt: core::optional_text(draft.excerpt.as_str()),
                slug: core::optional_text(draft.slug.as_str()),
                publish: draft.publish,
                tags: draft.tags,
                category_id: draft.category_id,
                channel_slugs: draft.channel_slugs,
                featured_image_url: draft.featured_image_url,
                seo_title: draft.seo_title,
                seo_description: draft.seo_description,
                scheduled_at: draft.scheduled_at,
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
                content: Some(draft.content),
                excerpt: core::optional_text(draft.excerpt.as_str()),
                // An empty slug must not be sent: the owner treats a present but
                // blank slug as a validation error instead of regenerating it
                // from the title, so a cleared field would block the save.
                slug: core::optional_text(draft.slug.as_str()),
                tags: Some(draft.tags),
                category_id: draft.category_id,
                channel_slugs: draft.channel_slugs,
                featured_image_url: draft.featured_image_url,
                seo_title: draft.seo_title,
                seo_description: draft.seo_description,
                scheduled_at: draft.schedule_editable.then_some(draft.scheduled_at),
                version: draft.version.ok_or_else(|| {
                    GraphqlHttpError::Graphql(
                        "Blog post revision is missing; reload before saving".into(),
                    )
                })?,
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

pub async fn pin_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let response: BoolMutationResponse = request(
        PIN_POST_MUTATION,
        PostIdVariables { id: id.clone() },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;
    if !response.pin_post {
        return Err(GraphqlHttpError::Graphql("Pin post returned false".into()));
    }
    fetch_post(token, tenant_slug, id, locale)
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Pinned post could not be reloaded".into()))
}

pub async fn unpin_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let response: BoolMutationResponse = request(
        UNPIN_POST_MUTATION,
        PostIdVariables { id: id.clone() },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;
    if !response.unpin_post {
        return Err(GraphqlHttpError::Graphql(
            "Unpin post returned false".into(),
        ));
    }
    fetch_post(token, tenant_slug, id, locale)
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Unpinned post could not be reloaded".into()))
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
            reason: None,
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

pub async fn restore_post(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ApiError> {
    let response: BoolMutationResponse = request(
        RESTORE_POST_MUTATION,
        PostIdVariables { id: id.clone() },
        token.clone(),
        tenant_slug.clone(),
    )
    .await?;

    if !response.restore_post {
        return Err(GraphqlHttpError::Graphql(
            "Restore post returned false".into(),
        ));
    }

    fetch_post(token, tenant_slug, id, locale)
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Restored post could not be reloaded".into()))
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

pub async fn create_preview_token(
    token: Option<String>,
    tenant_slug: Option<String>,
    post_id: String,
    ttl_hours: Option<i64>,
) -> Result<crate::model::BlogPreviewToken, ApiError> {
    let response: CreatePreviewTokenResponse = request(
        CREATE_PREVIEW_TOKEN_MUTATION,
        CreatePreviewTokenVariables {
            input: CreatePreviewTokenInputPayload {
                post_id,
                ttl_hours,
            },
        },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.create_preview_token)
}

pub async fn fetch_category_details(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<Vec<BlogCategoryDetail>, ApiError> {
    let mut items = Vec::new();
    for page in 1.. {
        let result: BlogCategoryDetailsResponse = request(
            BLOG_CATEGORY_DETAILS_QUERY,
            CategoryDetailVariables {
                filter: CategoryFilter {
                    locale: locale.clone(),
                    page,
                    per_page: 100,
                },
            },
            token.clone(),
            tenant_slug.clone(),
        )
        .await?;
        items.extend(result.blog_categories.items);
        if items.len() as u64 >= result.blog_categories.total {
            return Ok(items);
        }
        if (items.len() as u64) < page * 100 {
            return Err(GraphqlHttpError::Graphql(
                "Incomplete Blog categories projection".into(),
            ));
        }
    }
    unreachable!("Blog categories are bounded by the Taxonomy owner")
}

pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BlogCategoryDraft,
) -> Result<String, ApiError> {
    let response: CreateBlogCategoryResponse = request(
        CREATE_BLOG_CATEGORY_MUTATION,
        CreateBlogCategoryVariables { input: draft },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.create_blog_category)
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    update: BlogCategoryUpdate,
) -> Result<BlogCategoryDetail, ApiError> {
    let response: UpdateBlogCategoryResponse = request(
        UPDATE_BLOG_CATEGORY_MUTATION,
        UpdateBlogCategoryVariables { id, input: update },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.update_blog_category)
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let response: DeleteBlogCategoryResponse = request(
        DELETE_BLOG_CATEGORY_MUTATION,
        DeleteBlogCategoryVariables { id },
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.delete_blog_category)
}
