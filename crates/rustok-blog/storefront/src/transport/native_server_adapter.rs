use crate::comments_pagination::COMMENTS_PAGE_SIZE;
use crate::core::BlogStorefrontFetchRequest;
#[cfg(feature = "ssr")]
use crate::model::{BlogCommentList, BlogCommentListItem, BlogPostDetail, BlogPostList};
#[cfg(feature = "ssr")]
use crate::model::BlogPostListItem;
use crate::model::StorefrontBlogData;
use leptos::prelude::*;

use super::{configured_tenant_slug, ApiError};

#[cfg(feature = "ssr")]
const MODULE_SLUG: &str = "blog";
#[cfg(feature = "ssr")]
use rustok_api::PLATFORM_FALLBACK_LOCALE;

pub async fn fetch_blog(
    request: BlogStorefrontFetchRequest,
    comments_page: u64,
) -> Result<StorefrontBlogData, ApiError> {
    fetch_storefront_blog_server(
        configured_tenant_slug(),
        request.post_slug,
        request.locale,
        comments_page,
    )
    .await
}

async fn fetch_storefront_blog_server(
    tenant_slug: Option<String>,
    post_slug: String,
    locale: Option<String>,
    comments_page: u64,
) -> Result<StorefrontBlogData, ApiError> {
    storefront_blog_native(tenant_slug, post_slug, locale, comments_page)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]
async fn storefront_blog_native(
    tenant_slug: Option<String>,
    post_slug: String,
    locale: Option<String>,
    comments_page: u64,
) -> Result<StorefrontBlogData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_blog::{
            BlogPostStatus, CommentService, ListCommentsFilter, PostListQuery, PostService,
        };
        use rustok_channel::ChannelService;
        use rustok_core::SecurityContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_tenant::TenantService;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "blog/storefront-data requires TransactionalEventBus in host runtime context",
                )
            })?;
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
            let tenant = TenantService::new(runtime_ctx.db_clone())
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
                let enabled = ChannelService::new(runtime_ctx.db_clone())
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

        let service = PostService::new(runtime_ctx.db_clone(), event_bus.clone());

        let selected_post = service
            .get_post_by_slug_with_locale_fallback(
                tenant_id,
                SecurityContext::public_read(),
                requested_locale.as_str(),
                post_slug.as_str(),
                Some(fallback_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .filter(|post| {
                is_visible_for_public_channel(&post.channel_slugs, public_channel_slug.as_deref())
            });

        let selected_post = if let Some(post) = selected_post {
            let (comments, total) = CommentService::new(runtime_ctx.db_clone(), event_bus.clone())
                .list_for_post_with_locale_fallback(
                    tenant_id,
                    SecurityContext::public_read(),
                    post.id,
                    ListCommentsFilter {
                        locale: Some(requested_locale.clone()),
                        page: comments_page.max(1),
                        per_page: COMMENTS_PAGE_SIZE,
                    },
                    Some(fallback_locale.as_str()),
                )
                .await
                .map_err(ServerFnError::new)?;
            let public_comments = BlogCommentList {
                items: comments.into_iter().map(map_comment_list_item).collect(),
                total,
            };
            Some(map_post_detail(post, public_comments))
        } else {
            None
        };

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
        let _ = (tenant_slug, post_slug, locale, comments_page);
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
fn map_post_detail(
    post: rustok_blog::PostResponse,
    public_comments: BlogCommentList,
) -> BlogPostDetail {
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
        public_comments,
    }
}

#[cfg(feature = "ssr")]
fn map_comment_list_item(comment: rustok_blog::CommentListItem) -> BlogCommentListItem {
    BlogCommentListItem {
        id: comment.id.to_string(),
        effective_locale: comment.effective_locale,
        author_id: comment.author_id.map(|value| value.to_string()),
        content_preview: comment.content_preview,
        parent_comment_id: comment.parent_comment_id.map(|value| value.to_string()),
        created_at: comment.created_at,
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
