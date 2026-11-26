use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, RequestContext};
use rustok_blog_ports::{
    BLOG_PUBLISHED_POST_READ_CLAIM, BlogPublishedPost, BlogPublishedPostReadPort,
    ReadBlogPublishedPost, published_post_read_scope,
};
use rustok_core::SecurityContext;
use uuid::Uuid;

use crate::services::post::normalize_public_channel_slug;
use crate::{BlogError, BlogPostStatus, BlogPublicError, PostResponse, PostService};

/// Builds the internal public-read context after the host admits the request.
pub fn published_post_read_context(
    tenant_id: Uuid,
    locale: &str,
    request: Option<&RequestContext>,
) -> Result<PortContext, PortError> {
    if request.is_some_and(|request| request.tenant_id != tenant_id) {
        return Err(PortError::forbidden(
            "blog.published_post_tenant_mismatch",
            "Blog public read tenant scope does not match the request",
        ));
    }
    let correlation_id = request
        .map(|request| request.correlation_id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("blog-storefront"),
        locale,
        correlation_id,
    )
    .with_claim(BLOG_PUBLISHED_POST_READ_CLAIM)
    .with_deadline(Duration::from_secs(2));
    context.channel =
        request.and_then(|request| normalize_public_channel_slug(request.channel_slug.as_deref()));
    Ok(context)
}

#[async_trait]
impl BlogPublishedPostReadPort for PostService {
    async fn read_published_post(
        &self,
        context: PortContext,
        request: ReadBlogPublishedPost,
    ) -> Result<Option<BlogPublishedPost>, PortError> {
        let (tenant_id, deadline) = published_post_read_scope(&context, &request)?;
        let post = tokio::time::timeout(
            deadline,
            self.get_post_by_slug_on_channel(
                tenant_id,
                SecurityContext::public_read(),
                &context.locale,
                &request.slug,
                request.fallback_locale.as_deref(),
                context.channel.as_deref(),
            ),
        )
        .await
        .map_err(|_| {
            PortError::timeout("blog.published_post_timeout", "Blog public read timed out")
        })?;
        match post {
            Ok(post) => post.map(published_post).transpose(),
            Err(BlogError::PostNotFound(_))
            | Err(BlogError::Content(rustok_content::ContentError::NodeNotFound(_))) => Ok(None),
            Err(error) => Err(published_post_error(error)),
        }
    }
}

fn published_post(post: PostResponse) -> Result<BlogPublishedPost, PortError> {
    let published_at = post
        .published_at
        .filter(|_| post.status == BlogPostStatus::Published)
        .ok_or_else(|| {
            PortError::invariant_violation(
                "blog.published_post_invariant",
                "Blog returned an unpublished public record",
            )
        })?;
    Ok(BlogPublishedPost {
        tenant_id: post.tenant_id,
        id: post.id,
        author_id: post.author_id,
        slug: post.slug,
        title: post.title,
        requested_locale: post.requested_locale,
        effective_locale: post.effective_locale,
        available_locales: post.available_locales,
        content: post.content,
        content_plain_text: post.content_plain_text,
        excerpt: post.excerpt,
        category_id: post.category_id,
        category_name: post.category_name,
        tags: post.tags,
        featured_image_url: post.featured_image_url,
        seo_title: post.seo_title,
        seo_description: post.seo_description,
        channel_slugs: post.channel_slugs,
        created_at: post.created_at,
        updated_at: post.updated_at,
        published_at,
        is_pinned: post.is_pinned,
        pinned_at: post.pinned_at,
        version: post.version,
    })
}

fn published_post_error(error: BlogError) -> PortError {
    let is_database_failure = matches!(&error, BlogError::Database(_));
    let public = BlogPublicError::from(error);
    if is_database_failure {
        return PortError::unavailable(public.code, public.message);
    }
    match public.status {
        400 | 422 => PortError::validation(public.code, public.message),
        401 | 403 => PortError::forbidden(public.code, public.message),
        404 => PortError::not_found(public.code, public.message),
        409 => PortError::conflict(public.code, public.message),
        504 => PortError::timeout(public.code, public.message),
        503 => PortError::unavailable(public.code, public.message),
        _ => PortError::invariant_violation(public.code, public.message),
    }
}
