use leptos::prelude::*;

#[cfg(feature = "ssr")]
use std::sync::Arc;

#[cfg(feature = "ssr")]
use crate::model::{BlogModerationComment, BlogPostListItem};
use crate::model::{
    BlogModerationCommentList, BlogModerationStatus, BlogPostDetail, BlogPostDraft, BlogPostList,
};

pub(super) async fn fetch_posts(locale: Option<String>) -> Result<BlogPostList, ServerFnError> {
    blog_admin_posts_native(locale).await
}

pub(super) async fn fetch_post(
    id: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, ServerFnError> {
    blog_admin_post_native(id, locale).await
}

pub(super) async fn create_post(draft: BlogPostDraft) -> Result<BlogPostDetail, ServerFnError> {
    blog_admin_create_post_native(draft).await
}

pub(super) async fn update_post(
    id: String,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ServerFnError> {
    blog_admin_update_post_native(id, draft).await
}

pub(super) async fn publish_post(
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ServerFnError> {
    blog_admin_publish_post_native(id, locale).await
}

pub(super) async fn unpublish_post(
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ServerFnError> {
    blog_admin_unpublish_post_native(id, locale).await
}

pub(super) async fn archive_post(
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ServerFnError> {
    blog_admin_archive_post_native(id, locale).await
}

pub(super) async fn delete_post(id: String) -> Result<bool, ServerFnError> {
    blog_admin_delete_post_native(id).await
}

pub(super) async fn fetch_moderation_comments(
    post_id: String,
    locale: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<BlogModerationCommentList, ServerFnError> {
    blog_admin_moderation_comments_native(post_id, locale, page, per_page).await
}

pub(super) async fn moderate_comment(
    comment_id: String,
    status: BlogModerationStatus,
    locale: Option<String>,
) -> Result<bool, ServerFnError> {
    blog_admin_moderate_comment_native(comment_id, status, locale).await
}

#[cfg(feature = "ssr")]
struct NativeContext {
    db: sea_orm::DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
    comments_thread_port: Option<Arc<dyn rustok_blog::CommentsThreadPort>>,
    auth: rustok_api::AuthContext,
    tenant: rustok_api::TenantContext,
}

#[cfg(feature = "ssr")]
async fn native_context() -> Result<NativeContext, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;
    use rustok_outbox::TransactionalEventBus;

    let runtime = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    if auth.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Authenticated actor is not bound to the current tenant",
        ));
    }
    let event_bus = runtime
        .shared_get::<TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new("blog/admin requires TransactionalEventBus in host runtime context")
        })?;
    let comments_thread_port = runtime.shared_get::<Arc<dyn rustok_blog::CommentsThreadPort>>();

    Ok(NativeContext {
        db: runtime.db_clone(),
        event_bus,
        comments_thread_port,
        auth,
        tenant,
    })
}

#[cfg(feature = "ssr")]
fn comment_service(context: &NativeContext) -> rustok_blog::CommentService {
    if let Some(comments_thread_port) = context.comments_thread_port.clone() {
        rustok_blog::CommentService::with_comments_thread_port(
            context.db.clone(),
            comments_thread_port,
        )
    } else {
        rustok_blog::CommentService::new(context.db.clone(), context.event_bus.clone())
    }
}

#[cfg(feature = "ssr")]
fn security_context(auth: &rustok_api::AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}

#[cfg(feature = "ssr")]
fn require_manage_permission(auth: &rustok_api::AuthContext) -> Result<(), ServerFnError> {
    if rustok_api::has_any_effective_permission(
        &auth.permissions,
        &[rustok_api::Permission::BLOG_POSTS_MANAGE],
    ) {
        Ok(())
    } else {
        Err(ServerFnError::new(
            "Permission denied: blog_posts:manage required",
        ))
    }
}

#[cfg(feature = "ssr")]
fn requested_locale(locale: Option<String>, fallback: &str) -> String {
    locale
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/posts")]
async fn blog_admin_posts_native(locale: Option<String>) -> Result<BlogPostList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{PostListQuery, PostService};

        let context = native_context().await?;
        let locale = requested_locale(locale, context.tenant.default_locale.as_str());
        let result = PostService::new(context.db, context.event_bus)
            .list_posts_with_locale_fallback(
                context.tenant.id,
                security_context(&context.auth),
                PostListQuery {
                    locale: Some(locale),
                    page: Some(1),
                    per_page: Some(20),
                    sort_by: Some("created_at".to_string()),
                    sort_order: Some("desc".to_string()),
                    ..Default::default()
                },
                Some(context.tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(BlogPostList {
            items: result.items.into_iter().map(map_post_list_item).collect(),
            total: result.total,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "blog/admin/posts requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/post")]
async fn blog_admin_post_native(
    id: String,
    locale: Option<String>,
) -> Result<Option<BlogPostDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{BlogError, PostService};

        let context = native_context().await?;
        let post_id = parse_uuid(&id, "post_id")?;
        let locale = requested_locale(locale, context.tenant.default_locale.as_str());
        match PostService::new(context.db, context.event_bus)
            .get_post_with_locale_fallback(
                context.tenant.id,
                security_context(&context.auth),
                post_id,
                locale.as_str(),
                Some(context.tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(post) => Ok(Some(map_post_detail(post))),
            Err(BlogError::PostNotFound(_)) => Ok(None),
            Err(error) => Err(ServerFnError::new(error)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, locale);
        Err(ServerFnError::new(
            "blog/admin/post requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/create-post")]
async fn blog_admin_create_post_native(
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{CreatePostInput, PostService};

        let context = native_context().await?;
        let locale = draft.locale.clone();
        let service = PostService::new(context.db, context.event_bus);
        let post_id = service
            .create_post(
                context.tenant.id,
                security_context(&context.auth),
                CreatePostInput {
                    locale: draft.locale,
                    title: draft.title,
                    content: draft.content,
                    excerpt: optional_text(draft.excerpt),
                    slug: optional_text(draft.slug),
                    publish: draft.publish,
                    tags: draft.tags,
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        let post = service
            .get_post_with_locale_fallback(
                context.tenant.id,
                security_context(&context.auth),
                post_id,
                locale.as_str(),
                Some(context.tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(map_post_detail(post))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = draft;
        Err(ServerFnError::new(
            "blog/admin/create-post requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/update-post")]
async fn blog_admin_update_post_native(
    id: String,
    draft: BlogPostDraft,
) -> Result<BlogPostDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{PostService, UpdatePostInput};

        let context = native_context().await?;
        let post_id = parse_uuid(&id, "post_id")?;
        let locale = draft.locale.clone();
        let service = PostService::new(context.db, context.event_bus);
        service
            .update_post(
                context.tenant.id,
                post_id,
                security_context(&context.auth),
                UpdatePostInput {
                    locale: Some(draft.locale),
                    title: Some(draft.title),
                    content: Some(draft.content),
                    excerpt: Some(draft.excerpt),
                    slug: Some(draft.slug),
                    tags: Some(draft.tags),
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                    version: None,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        let post = service
            .get_post_with_locale_fallback(
                context.tenant.id,
                security_context(&context.auth),
                post_id,
                locale.as_str(),
                Some(context.tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(map_post_detail(post))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, draft);
        Err(ServerFnError::new(
            "blog/admin/update-post requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/publish-post")]
async fn blog_admin_publish_post_native(
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        update_status_and_reload(id, locale, StatusMutation::Publish).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, locale);
        Err(ServerFnError::new(
            "blog/admin/publish-post requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/unpublish-post")]
async fn blog_admin_unpublish_post_native(
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        update_status_and_reload(id, locale, StatusMutation::Unpublish).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, locale);
        Err(ServerFnError::new(
            "blog/admin/unpublish-post requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/archive-post")]
async fn blog_admin_archive_post_native(
    id: String,
    locale: Option<String>,
) -> Result<BlogPostDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        update_status_and_reload(id, locale, StatusMutation::Archive).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, locale);
        Err(ServerFnError::new(
            "blog/admin/archive-post requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
enum StatusMutation {
    Publish,
    Unpublish,
    Archive,
}

#[cfg(feature = "ssr")]
async fn update_status_and_reload(
    id: String,
    locale: Option<String>,
    mutation: StatusMutation,
) -> Result<BlogPostDetail, ServerFnError> {
    use rustok_blog::PostService;

    let context = native_context().await?;
    let post_id = parse_uuid(&id, "post_id")?;
    let locale = requested_locale(locale, context.tenant.default_locale.as_str());
    let service = PostService::new(context.db, context.event_bus);
    let security = security_context(&context.auth);
    match mutation {
        StatusMutation::Publish => {
            service
                .publish_post(context.tenant.id, post_id, security)
                .await
        }
        StatusMutation::Unpublish => {
            service
                .unpublish_post(context.tenant.id, post_id, security)
                .await
        }
        StatusMutation::Archive => {
            service
                .archive_post(
                    context.tenant.id,
                    post_id,
                    security,
                    Some("Archived from module admin package".to_string()),
                )
                .await
        }
    }
    .map_err(ServerFnError::new)?;

    let post = service
        .get_post_with_locale_fallback(
            context.tenant.id,
            security_context(&context.auth),
            post_id,
            locale.as_str(),
            Some(context.tenant.default_locale.as_str()),
        )
        .await
        .map_err(ServerFnError::new)?;
    Ok(map_post_detail(post))
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/delete-post")]
async fn blog_admin_delete_post_native(id: String) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::PostService;

        let context = native_context().await?;
        let post_id = parse_uuid(&id, "post_id")?;
        PostService::new(context.db, context.event_bus)
            .delete_post(context.tenant.id, post_id, security_context(&context.auth))
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "blog/admin/delete-post requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/moderation-comments")]
async fn blog_admin_moderation_comments_native(
    post_id: String,
    locale: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<BlogModerationCommentList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::ListCommentsFilter;

        let context = native_context().await?;
        require_manage_permission(&context.auth)?;
        let post_id = parse_uuid(&post_id, "post_id")?;
        let locale = requested_locale(locale, context.tenant.default_locale.as_str());
        let (items, total) = comment_service(&context)
            .list_for_post_with_locale_fallback(
                context.tenant.id,
                security_context(&context.auth),
                post_id,
                ListCommentsFilter {
                    locale: Some(locale),
                    page: page.max(1),
                    per_page: per_page.clamp(1, 100),
                },
                Some(context.tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(BlogModerationCommentList {
            items: items.into_iter().map(map_moderation_comment).collect(),
            total,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (post_id, locale, page, per_page);
        Err(ServerFnError::new(
            "blog/admin/moderation-comments requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/moderate-comment")]
async fn blog_admin_moderate_comment_native(
    comment_id: String,
    status: BlogModerationStatus,
    locale: Option<String>,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{ModerateCommentInput, ModerateCommentStatus};

        let context = native_context().await?;
        require_manage_permission(&context.auth)?;
        let comment_id = parse_uuid(&comment_id, "comment_id")?;
        let status = match status {
            BlogModerationStatus::Approved => ModerateCommentStatus::Approved,
            BlogModerationStatus::Spam => ModerateCommentStatus::Spam,
            BlogModerationStatus::Trash => ModerateCommentStatus::Trash,
        };
        comment_service(&context)
            .moderate_comment(
                context.tenant.id,
                comment_id,
                security_context(&context.auth),
                ModerateCommentInput { status, locale },
                Some(context.tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (comment_id, status, locale);
        Err(ServerFnError::new(
            "blog/admin/moderate-comment requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(feature = "ssr")]
fn status_label(status: rustok_blog::BlogPostStatus) -> String {
    match status {
        rustok_blog::BlogPostStatus::Draft => "draft",
        rustok_blog::BlogPostStatus::Published => "published",
        rustok_blog::BlogPostStatus::Archived => "archived",
    }
    .to_string()
}

#[cfg(feature = "ssr")]
fn map_post_list_item(post: rustok_blog::PostSummary) -> BlogPostListItem {
    BlogPostListItem {
        id: post.id.to_string(),
        title: post.title,
        effective_locale: post.effective_locale,
        slug: Some(post.slug),
        excerpt: post.excerpt,
        status: status_label(post.status),
        created_at: post.created_at.to_rfc3339(),
        published_at: post.published_at.map(|value| value.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
fn map_post_detail(post: rustok_blog::PostResponse) -> BlogPostDetail {
    BlogPostDetail {
        id: post.id.to_string(),
        requested_locale: post.requested_locale,
        effective_locale: post.effective_locale,
        available_locales: post.available_locales,
        title: post.title,
        slug: Some(post.slug),
        excerpt: post.excerpt,
        content: Some(post.content),
        content_plain_text: Some(post.content_plain_text),
        status: status_label(post.status),
        created_at: post.created_at.to_rfc3339(),
        updated_at: post.updated_at.to_rfc3339(),
        published_at: post.published_at.map(|value| value.to_rfc3339()),
        tags: post.tags,
        featured_image_url: post.featured_image_url,
        seo_title: post.seo_title,
        seo_description: post.seo_description,
    }
}

#[cfg(feature = "ssr")]
fn map_moderation_comment(comment: rustok_blog::CommentListItem) -> BlogModerationComment {
    BlogModerationComment {
        id: comment.id.to_string(),
        effective_locale: comment.effective_locale,
        author_id: comment.author_id.map(|value| value.to_string()),
        content_preview: comment.content_preview,
        status: comment.status,
        parent_comment_id: comment.parent_comment_id.map(|value| value.to_string()),
        created_at: comment.created_at,
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[test]
    fn admin_native_runtime_exposes_comments_port_selection() {
        let selector: fn(&NativeContext) -> rustok_blog::CommentService = comment_service;
        let _ = selector;
    }
}
