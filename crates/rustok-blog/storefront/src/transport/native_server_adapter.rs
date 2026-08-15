#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::comments_pagination::COMMENTS_PAGE_SIZE;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::core::BlogStorefrontFetchRequest;
use crate::model::{BlogCommentCreateRequest, BlogCommentDetail};
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::model::{
    BlogCommentList, BlogCommentListItem, BlogCommentsAvailability, BlogPostDetail, BlogPostList,
    BlogPostListItem, StorefrontBlogData,
};
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use std::sync::Arc;

use super::ApiError;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use super::configured_tenant_slug;

#[cfg(feature = "ssr")]
const MODULE_SLUG: &str = "blog";
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use rustok_api::PLATFORM_FALLBACK_LOCALE;

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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

pub async fn create_comment(
    request: BlogCommentCreateRequest,
) -> Result<BlogCommentDetail, ApiError> {
    create_blog_comment_native(request)
        .await
        .map_err(ApiError::from)
}

#[server(
    prefix = "/api/fn",
    endpoint = "blog/comment-create",
    client = leptos_auth::AuthorizedBrowserClient
)]
async fn create_blog_comment_native(
    request: BlogCommentCreateRequest,
) -> Result<BlogCommentDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{Action, HostRuntimeContext, Permission, Resource};
        use rustok_outbox::TransactionalEventBus;

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "Blog comment creation must use the current authenticated tenant",
            ));
        }
        if !rustok_api::has_any_effective_permission(
            &auth.permissions,
            &[Permission::new(Resource::Comments, Action::Create)],
        ) {
            return Err(ServerFnError::new("comments:create required"));
        }

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        match rustok_api::is_tenant_module_enabled(runtime_ctx.db(), tenant.id, MODULE_SLUG).await {
            Ok(true) => {}
            Ok(false) => return Err(ServerFnError::new("Blog module is not enabled")),
            Err(error) => {
                return Err(ServerFnError::new(format!(
                    "Blog module state is unavailable: {error}"
                )));
            }
        }
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "blog/comment-create requires TransactionalEventBus in host runtime context",
                )
            })?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        require_blog_channel_enabled(&runtime_ctx, request_context.as_ref()).await?;

        let post_id = uuid::Uuid::parse_str(request.post_id.trim())
            .map_err(|_| ServerFnError::new("Invalid post_id"))?;
        let parent_comment_id = request
            .parent_comment_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ServerFnError::new("Invalid parent_comment_id"))?;
        let locale = request.locale.trim();
        let locale = if locale.is_empty() {
            tenant.default_locale.clone()
        } else {
            locale.to_string()
        };
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|context| context.channel_slug.as_deref());

        let comment = comment_service(&runtime_ctx, event_bus)
            .create_public_comment(
                tenant.id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
                post_id,
                public_channel_slug,
                rustok_blog::CreateCommentInput {
                    locale,
                    content: request.content,
                    parent_comment_id,
                },
            )
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(map_comment_detail(comment))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "blog/comment-create requires the `ssr` feature",
        ))
    }
}

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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
            BlogPostStatus, PostListQuery, PostService, PublicCommentsSnapshotStore,
            list_public_comments_with_snapshot,
        };
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

        require_blog_channel_enabled(&runtime_ctx, request_context.as_ref()).await?;

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
            let comments = comment_service(&runtime_ctx, event_bus.clone());
            let snapshot_store = runtime_ctx.shared_get::<Arc<dyn PublicCommentsSnapshotStore>>();
            let public_comments = list_public_comments_with_snapshot(
                &comments,
                snapshot_store.as_ref(),
                tenant_id,
                post.id,
                requested_locale.as_str(),
                Some(fallback_locale.as_str()),
                comments_page,
                COMMENTS_PAGE_SIZE,
            )
            .await
            .map_err(ServerFnError::new)?;
            let public_comments = BlogCommentList {
                availability: map_comments_availability(public_comments.availability),
                cached_snapshot: public_comments.cached_snapshot,
                items: public_comments
                    .items
                    .into_iter()
                    .map(map_comment_list_item)
                    .collect(),
                total: public_comments.total,
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
fn comment_service(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> rustok_blog::CommentService {
    if let Some(comments_thread_port) =
        runtime_ctx.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>()
    {
        rustok_blog::CommentService::with_comments_thread_port(
            runtime_ctx.db_clone(),
            comments_thread_port,
        )
    } else {
        rustok_blog::CommentService::new(runtime_ctx.db_clone(), event_bus)
    }
}

#[cfg(feature = "ssr")]
async fn require_blog_channel_enabled(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    request_context: Option<&rustok_api::RequestContext>,
) -> Result<(), ServerFnError> {
    use rustok_channel::ChannelService;

    let Some(request_context) = request_context else {
        return Ok(());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };
    let enabled = ChannelService::new(runtime_ctx.db_clone())
        .is_module_enabled(channel_id, MODULE_SLUG)
        .await
        .map_err(ServerFnError::new)?;
    if enabled {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
            request_context.channel_slug.as_deref().unwrap_or("current"),
        )))
    }
}

#[cfg(feature = "ssr")]
fn map_comment_detail(comment: rustok_blog::CommentResponse) -> BlogCommentDetail {
    BlogCommentDetail {
        id: comment.id.to_string(),
        requested_locale: comment.requested_locale,
        effective_locale: comment.effective_locale,
        post_id: comment.post_id.to_string(),
        author_id: comment.author_id.map(|value| value.to_string()),
        content: comment.content,
        content_plain_text: comment.content_text,
        status: comment.status,
        parent_comment_id: comment.parent_comment_id.map(|value| value.to_string()),
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    }
}

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
fn map_comments_availability(
    availability: rustok_blog::PublicCommentsAvailability,
) -> BlogCommentsAvailability {
    match availability {
        rustok_blog::PublicCommentsAvailability::Available => BlogCommentsAvailability::Available,
        rustok_blog::PublicCommentsAvailability::Unavailable => {
            BlogCommentsAvailability::Unavailable
        }
        rustok_blog::PublicCommentsAvailability::Timeout => BlogCommentsAvailability::Timeout,
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

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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
        content: Some(post.content),
        content_plain_text: Some(post.content_plain_text),
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

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
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

#[cfg(all(test, any(feature = "ssr", not(feature = "comment-island"))))]
mod tests {
    use super::*;

    #[test]
    fn storefront_native_runtime_exposes_comments_port_selection() {
        let selector: fn(
            &rustok_api::HostRuntimeContext,
            rustok_outbox::TransactionalEventBus,
        ) -> rustok_blog::CommentService = comment_service;
        let mapper: fn(rustok_blog::PublicCommentsAvailability) -> BlogCommentsAvailability =
            map_comments_availability;
        let _ = (selector, mapper);
    }
}
