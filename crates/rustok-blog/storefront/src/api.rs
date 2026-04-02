use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{BlogPostDetail, BlogPostList, BlogPostListItem, StorefrontBlogData};

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

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_BLOG_QUERY: &str = "query StorefrontBlog($postSlug: String!, $filter: PostsFilter, $locale: String) { selectedPost: postBySlug(slug: $postSlug, locale: $locale) { id effectiveLocale title slug excerpt body bodyFormat status publishedAt tags featuredImageUrl } posts(filter: $filter) { total items { id title effectiveLocale slug excerpt status publishedAt } } }";
const MODULE_SLUG: &str = "blog";
const PLATFORM_FALLBACK_LOCALE: &str = "en";

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
}

#[derive(Clone, Debug, Serialize)]
struct PostsFilter {
    status: Option<String>,
    locale: Option<String>,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

fn configured_tenant_slug() -> Option<String> {
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
    .map_err(ApiError::from)
}

pub async fn fetch_storefront_blog(
    post_slug: String,
    locale: Option<String>,
) -> Result<StorefrontBlogData, ApiError> {
    match fetch_storefront_blog_server(configured_tenant_slug(), post_slug.clone(), locale.clone())
        .await
    {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_blog_graphql(post_slug, locale).await,
    }
}

pub async fn fetch_storefront_blog_server(
    tenant_slug: Option<String>,
    post_slug: String,
    locale: Option<String>,
) -> Result<StorefrontBlogData, ApiError> {
    storefront_blog_native(tenant_slug, post_slug, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_blog_graphql(
    post_slug: String,
    locale: Option<String>,
) -> Result<StorefrontBlogData, ApiError> {
    let response: StorefrontBlogResponse = request(
        STOREFRONT_BLOG_QUERY,
        StorefrontBlogVariables {
            post_slug,
            filter: PostsFilter {
                status: Some("PUBLISHED".to_string()),
                locale: locale.clone(),
                page: 1,
                per_page: 6,
            },
            locale,
        },
    )
    .await?;

    Ok(StorefrontBlogData {
        selected_post: response.selected_post,
        posts: response.posts,
    })
}

#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]
async fn storefront_blog_native(
    tenant_slug: Option<String>,
    post_slug: String,
    locale: Option<String>,
) -> Result<StorefrontBlogData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_blog::{BlogPostStatus, PostListQuery, PostService};
        use rustok_channel::ChannelService;
        use rustok_core::SecurityContext;
        use rustok_tenant::TenantService;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();

        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (tenant.id, tenant.default_locale.clone())
        } else {
            let slug = tenant_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ServerFnError::new(
                        "blog/storefront-data requires tenant context or tenant slug",
                    )
                })?;
            let tenant = TenantService::new(app_ctx.db.clone())
                .get_tenant_by_slug(slug)
                .await
                .map_err(ServerFnError::new)?;
            let fallback = request_context
                .as_ref()
                .map(|ctx| ctx.locale.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
            (tenant.id, fallback)
        };

        if let Some(request_context) = request_context.as_ref() {
            if let Some(channel_id) = request_context.channel_id {
                let enabled = ChannelService::new(app_ctx.db.clone())
                    .is_module_enabled(channel_id, MODULE_SLUG)
                    .await
                    .map_err(ServerFnError::new)?;
                if !enabled {
                    return Err(ServerFnError::new(format!(
                        "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
                        request_context.channel_slug.as_deref().unwrap_or("current"),
                    )));
                }
            }
        }

        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or_else(|| fallback_locale.clone());
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|ctx| normalize_channel_slug(ctx.channel_slug.as_deref()));

        let service = PostService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(&app_ctx),
        );

        let selected_post = service
            .get_post_by_slug_with_locale_fallback(
                tenant_id,
                SecurityContext::system(),
                requested_locale.as_str(),
                post_slug.as_str(),
                Some(fallback_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .filter(|post| {
                is_visible_for_public_channel(&post.channel_slugs, public_channel_slug.as_deref())
            })
            .map(map_post_detail);

        let posts = service
            .list_public_visible_with_locale_fallback(
                tenant_id,
                PostListQuery {
                    status: Some(BlogPostStatus::Published),
                    category_id: None,
                    tag: None,
                    author_id: None,
                    search: None,
                    locale: Some(requested_locale),
                    page: Some(1),
                    per_page: Some(6),
                    sort_by: Some("published_at".to_string()),
                    sort_order: Some("desc".to_string()),
                },
                Some(fallback_locale.as_str()),
                public_channel_slug.as_deref(),
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(StorefrontBlogData {
            selected_post,
            posts: BlogPostList {
                items: posts.items.into_iter().map(map_post_list_item).collect(),
                total: posts.total,
            },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, post_slug, locale);
        Err(ServerFnError::new(
            "blog/storefront-data requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

#[cfg(feature = "ssr")]
fn is_visible_for_public_channel(
    channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }

    let Some(public_channel_slug) = public_channel_slug else {
        return false;
    };

    channel_slugs
        .iter()
        .any(|slug| slug.eq_ignore_ascii_case(public_channel_slug))
}

#[cfg(feature = "ssr")]
fn map_post_detail(post: rustok_blog::PostResponse) -> BlogPostDetail {
    BlogPostDetail {
        id: post.id.to_string(),
        effective_locale: post.effective_locale,
        title: post.title,
        slug: Some(post.slug),
        excerpt: post.excerpt,
        body: Some(post.body),
        body_format: post.body_format,
        status: match post.status {
            rustok_blog::BlogPostStatus::Draft => "draft",
            rustok_blog::BlogPostStatus::Published => "published",
            rustok_blog::BlogPostStatus::Archived => "archived",
        }
        .to_string(),
        published_at: post.published_at.map(|value| value.to_string()),
        tags: post.tags,
        featured_image_url: post.featured_image_url,
    }
}

#[cfg(feature = "ssr")]
fn map_post_list_item(post: rustok_blog::PostSummary) -> BlogPostListItem {
    BlogPostListItem {
        id: post.id.to_string(),
        title: post.title,
        effective_locale: post.effective_locale,
        slug: Some(post.slug),
        excerpt: post.excerpt,
        status: match post.status {
            rustok_blog::BlogPostStatus::Draft => "draft",
            rustok_blog::BlogPostStatus::Published => "published",
            rustok_blog::BlogPostStatus::Archived => "archived",
        }
        .to_string(),
        published_at: post.published_at.map(|value| value.to_string()),
    }
}
