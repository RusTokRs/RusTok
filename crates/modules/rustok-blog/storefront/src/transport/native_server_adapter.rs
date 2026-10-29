#[cfg(feature = "ssr")]
use crate::comments_pagination::COMMENTS_PAGE_SIZE;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::core::BlogStorefrontFetchRequest;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
use crate::model::StorefrontBlogData;
use crate::model::{BlogCommentCreateRequest, BlogCommentDetail};
#[cfg(feature = "ssr")]
use crate::model::{
    BlogCommentList, BlogCommentListItem, BlogCommentsAvailability, BlogPostAuthorProfile,
    BlogPostDetail, BlogPostList, BlogPostListItem,
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
pub async fn fetch_blog(
    request: BlogStorefrontFetchRequest,
    comments_page: u64,
) -> Result<StorefrontBlogData, ApiError> {
    fetch_storefront_blog_server(
        configured_tenant_slug(),
        request.post_slug,
        request.locale,
        request.tag,
        request.category_id,
        request.posts_after,
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
        use leptos::prelude::use_context;
        use rustok_api::{Action, HostRuntimeContext, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|_| ServerFnError::new("Authentication required"))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|_| public_internal_error())?;
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

        let runtime_ctx = use_context::<HostRuntimeContext>().ok_or_else(public_internal_error)?;
        match rustok_api::is_tenant_module_enabled(runtime_ctx.db(), tenant.id, MODULE_SLUG).await {
            Ok(true) => {}
            Ok(false) => return Err(ServerFnError::new("Blog module is not enabled")),
            Err(_error) => {
                return Err(ServerFnError::new(
                    "Blog module state is temporarily unavailable",
                ));
            }
        }
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        require_blog_comment_channel_enabled(&runtime_ctx, tenant.id, request_context.as_ref())
            .await?;

        let command_id = uuid::Uuid::parse_str(request.command_id.trim())
            .map_err(|_| ServerFnError::new("Invalid command_id"))?;
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

        let comment = comment_service(&runtime_ctx)
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
                    command_id,
                    locale,
                    content: request.content,
                    parent_comment_id,
                },
            )
            .await
            .map_err(public_blog_error)?;

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
    tag: Option<String>,
    category_id: Option<String>,
    posts_after: Option<String>,
    comments_page: u64,
) -> Result<StorefrontBlogData, ApiError> {
    storefront_blog_native(
        tenant_slug,
        post_slug,
        locale,
        tag,
        category_id,
        posts_after,
        comments_page,
    )
    .await
    .map_err(ApiError::from)
}

#[cfg(feature = "ssr")]
fn public_blog_error(error: rustok_blog::BlogError) -> ServerFnError {
    let public = rustok_blog::BlogPublicError::from(error);
    ServerFnError::new(format!("{}: {}", public.code, public.message))
}

#[cfg(feature = "ssr")]
fn public_internal_error() -> ServerFnError {
    let public = rustok_blog::BlogPublicError::internal();
    ServerFnError::new(format!("{}: {}", public.code, public.message))
}

#[server(prefix = "/api/fn", endpoint = "blog/storefront-data")]
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
async fn storefront_blog_native(
    tenant_slug: Option<String>,
    post_slug: String,
    locale: Option<String>,
    tag: Option<String>,
    category_id: Option<String>,
    posts_after: Option<String>,
    comments_page: u64,
) -> Result<StorefrontBlogData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::use_context;
        use rustok_api::HostRuntimeContext;
        use rustok_blog::{
            PostService, PublicCommentsSnapshotStore, PublicPostsPageQuery,
            list_public_comments_with_snapshot,
        };
        use rustok_core::SecurityContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_tenant::TenantService;

        let runtime_ctx = use_context::<HostRuntimeContext>().ok_or_else(public_internal_error)?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(public_internal_error)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();

        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (tenant.id, tenant.default_locale.clone())
        } else {
            let slug = super::configured_fallback_tenant_slug(tenant_slug.as_deref())?;
            let tenant = TenantService::new(runtime_ctx.db_clone())
                .get_tenant_by_slug(slug.as_str())
                .await
                .map_err(|_| public_internal_error())?;
            (tenant.id, tenant.default_locale)
        };

        // Native server functions are callable independently of the host UI
        // registry. Enforce the same tenant module gate as the GraphQL query.
        match rustok_api::is_tenant_module_enabled(runtime_ctx.db(), tenant_id, MODULE_SLUG).await {
            Ok(true) => {}
            Ok(false) => return Err(ServerFnError::new("Blog module is not enabled")),
            Err(_) => {
                return Err(ServerFnError::new(
                    "Blog module state is temporarily unavailable",
                ));
            }
        }

        if request_context
            .as_ref()
            .is_some_and(|context| context.tenant_id != tenant_id)
        {
            return Err(public_internal_error());
        }

        let auth_context = leptos_axum::extract::<rustok_api::AuthContext>().await.ok();
        ensure_storefront_tenant_binding(auth_context.as_ref(), tenant_id)?;
        let is_authenticated = auth_context.is_some();
        let profile_audience = auth_context
            .as_ref()
            .map(
                |auth| rustok_profiles::ProfileSummaryAudience::Authenticated {
                    actor_id: auth.user_id,
                },
            )
            .unwrap_or(rustok_profiles::ProfileSummaryAudience::Anonymous);

        require_blog_channel_enabled(
            &runtime_ctx,
            tenant_id,
            request_context.as_ref(),
            is_authenticated,
        )
        .await?;

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
            .map_err(public_blog_error)?
            .filter(|post| {
                is_visible_for_public_channel(&post.channel_slugs, public_channel_slug.as_deref())
            });

        let selected_post = if let Some(post) = selected_post {
            let comments = comment_service(&runtime_ctx);
            let snapshot_store = runtime_ctx.shared_get::<Arc<dyn PublicCommentsSnapshotStore>>();
            let public_comments = list_public_comments_with_snapshot(
                &comments,
                snapshot_store.as_ref(),
                tenant_id,
                post.id,
                requested_locale.as_str(),
                Some(fallback_locale.as_str()),
                public_channel_slug.as_deref(),
                comments_page,
                COMMENTS_PAGE_SIZE,
            )
            .await
            .map_err(public_blog_error)?;
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
            let mut author_profiles = load_author_profiles(
                &runtime_ctx,
                tenant_id,
                &[post.author_id],
                requested_locale.as_str(),
                fallback_locale.as_str(),
                profile_audience,
            )
            .await;
            let author_profile = author_profiles.remove(&post.author_id);
            Some(map_post_detail(post, public_comments, author_profile))
        } else {
            None
        };

        // Match the GraphQL UUID coercion rule: invalid filters are errors, not
        // silently widened queries returning posts from every category.
        let category_uuid = category_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ServerFnError::new("Invalid category_id"))?;
        let tag_normalized = tag
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        let posts_after = posts_after
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(rustok_blog::PublishedPostCursor::decode)
            .transpose()
            .map_err(public_blog_error)?;

        let page = service
            .list_public_visible_keyset(
                tenant_id,
                PublicPostsPageQuery {
                    category_id: category_uuid,
                    tag: tag_normalized,
                    author_id: None,
                    locale: Some(requested_locale.clone()),
                    per_page: Some(6),
                    after: posts_after,
                },
                Some(fallback_locale.as_str()),
                public_channel_slug.as_deref(),
            )
            .await
            .map_err(public_blog_error)?;

        let author_ids = page
            .items
            .iter()
            .map(|post| post.author_id)
            .collect::<Vec<_>>();
        let author_profiles = load_author_profiles(
            &runtime_ctx,
            tenant_id,
            author_ids.as_slice(),
            requested_locale.as_str(),
            fallback_locale.as_str(),
            profile_audience,
        )
        .await;

        Ok(StorefrontBlogData {
            selected_post,
            posts: BlogPostList {
                items: page
                    .items
                    .into_iter()
                    .map(|post| {
                        let author_profile = author_profiles.get(&post.author_id).cloned();
                        map_post_list_item(post, author_profile)
                    })
                    .collect(),
                next_cursor: page.next_cursor.map(|cursor| cursor.encode()),
            },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            tenant_slug,
            post_slug,
            locale,
            tag,
            category_id,
            posts_after,
            comments_page,
        );
        Err(ServerFnError::new(
            "blog/storefront-data requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn load_author_profiles(
    runtime: &rustok_api::HostRuntimeContext,
    tenant_id: uuid::Uuid,
    authors: &[uuid::Uuid],
    locale: &str,
    fallback_locale: &str,
    audience: rustok_profiles::ProfileSummaryAudience,
) -> std::collections::HashMap<uuid::Uuid, BlogPostAuthorProfile> {
    use std::collections::HashMap;

    if authors.is_empty() {
        return HashMap::new();
    }
    // Profiles is optional; missing presentation cannot hide a published post.
    let Some(reader) = runtime.shared_get::<Arc<dyn rustok_profiles::ProfileSummaryReader>>()
    else {
        return HashMap::new();
    };

    let summaries = match reader
        .find_profile_summaries(
            tenant_id,
            authors,
            Some(locale),
            Some(fallback_locale),
            audience,
        )
        .await
    {
        Ok(summaries) => summaries,
        Err(error) => {
            eprintln!(
                "Blog author profile presentation unavailable for tenant {tenant_id}: {}",
                error.code()
            );
            return HashMap::new();
        }
    };

    summaries
        .into_iter()
        .map(|(id, profile)| {
            (
                id,
                BlogPostAuthorProfile {
                    user_id: profile.user_id.to_string(),
                    handle: profile.handle,
                    display_name: profile.display_name,
                    tags: profile.tags,
                    avatar_media_id: profile.avatar_media_id.map(|id| id.to_string()),
                },
            )
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn comment_service(runtime_ctx: &rustok_api::HostRuntimeContext) -> rustok_blog::CommentService {
    rustok_blog::CommentService::from_runtime_capabilities(
        runtime_ctx.db_clone(),
        runtime_ctx.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>(),
        runtime_ctx.shared_get::<rustok_api::SharedStaticModuleSettingsReader>(),
    )
}

#[cfg(feature = "ssr")]
async fn require_blog_comment_channel_enabled(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    tenant_id: uuid::Uuid,
    request_context: Option<&rustok_api::RequestContext>,
) -> Result<(), ServerFnError> {
    let Some(request_context) = request_context else {
        return Err(public_internal_error());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Err(public_internal_error());
    };

    let enabled = rustok_channel::ChannelService::new(runtime_ctx.db_clone())
        .is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)
        .await
        .map_err(|_| ServerFnError::new("Blog channel state is temporarily unavailable"))?;

    if enabled {
        Ok(())
    } else {
        Err(ServerFnError::new(
            "Blog is not available for the current channel",
        ))
    }
}

#[cfg(feature = "ssr")]
fn ensure_storefront_tenant_binding(
    auth_context: Option<&rustok_api::AuthContext>,
    tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    if auth_context.is_some_and(|auth| auth.tenant_id != tenant_id) {
        return Err(public_internal_error());
    }
    Ok(())
}

#[cfg(feature = "ssr")]
async fn require_blog_channel_enabled(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    tenant_id: uuid::Uuid,
    request_context: Option<&rustok_api::RequestContext>,
    is_authenticated: bool,
) -> Result<(), ServerFnError> {
    use rustok_channel::ChannelService;

    if is_authenticated {
        return Ok(());
    }

    let Some(request_context) = request_context else {
        return Ok(());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };
    let enabled = ChannelService::new(runtime_ctx.db_clone())
        .is_module_enabled_for_tenant(tenant_id, channel_id, MODULE_SLUG)
        .await
        .map_err(|_| ServerFnError::new("Blog channel state is temporarily unavailable"))?;
    if enabled {
        Ok(())
    } else {
        Err(ServerFnError::new(
            "Blog is not available for the current channel",
        ))
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

#[cfg(feature = "ssr")]
fn map_comments_availability(
    availability: rustok_blog::PublicCommentsAvailability,
) -> BlogCommentsAvailability {
    match availability {
        rustok_blog::PublicCommentsAvailability::Available => BlogCommentsAvailability::Available,
        rustok_blog::PublicCommentsAvailability::Disabled => BlogCommentsAvailability::Disabled,
        rustok_blog::PublicCommentsAvailability::ReadOnly => BlogCommentsAvailability::ReadOnly,
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

#[cfg(feature = "ssr")]
fn map_post_detail(
    post: rustok_blog::PostResponse,
    public_comments: BlogCommentList,
    author_profile: Option<BlogPostAuthorProfile>,
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
        author_profile,
        category_id: post.category_id.map(|v| v.to_string()),
        category_name: post.category_name,
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
fn map_post_list_item(
    post: rustok_blog::PostSummary,
    author_profile: Option<BlogPostAuthorProfile>,
) -> BlogPostListItem {
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
        tags: post.tags,
        featured_image_url: post.featured_image_url,
        author_profile,
        category_id: post.category_id.map(|v| v.to_string()),
        category_name: post.category_name,
    }
}

#[cfg(feature = "ssr")]
#[test]
fn storefront_tenant_binding_rejects_cross_tenant_authenticated_context() {
    let tenant_id = uuid::Uuid::new_v4();
    let other_tenant_id = uuid::Uuid::new_v4();
    let auth = rustok_api::AuthContext {
        user_id: uuid::Uuid::new_v4(),
        session_id: uuid::Uuid::new_v4(),
        tenant_id: other_tenant_id,
        permissions: Vec::new(),
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    };

    assert!(ensure_storefront_tenant_binding(Some(&auth), tenant_id).is_err());
    assert!(ensure_storefront_tenant_binding(Some(&auth), other_tenant_id).is_ok());
    assert!(ensure_storefront_tenant_binding(None, tenant_id).is_ok());
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[test]
    fn storefront_native_runtime_exposes_comments_port_selection() {
        let selector: fn(&rustok_api::HostRuntimeContext) -> rustok_blog::CommentService =
            comment_service;
        let mapper: fn(rustok_blog::PublicCommentsAvailability) -> BlogCommentsAvailability =
            map_comments_availability;
        let _ = (selector, mapper);
    }
}
