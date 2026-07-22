use async_graphql::{ComplexObject, Context, Enum, FieldError, InputObject, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::GraphQLError, has_any_effective_permission,
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::graphql::GqlProfileSummary;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    BlogPostStatus, CommentListItem as DomainCommentListItem, CommentService,
    CreatePostInput as DomainCreatePostInput, ListCommentsFilter,
    ModerateCommentStatus as DomainModerateCommentStatus, PostResponse, PostSummary,
};

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(name = "BlogPostStatus", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlContentStatus {
    Draft,
    Published,
    Archived,
}

impl From<BlogPostStatus> for GqlContentStatus {
    fn from(status: BlogPostStatus) -> Self {
        match status {
            BlogPostStatus::Draft => Self::Draft,
            BlogPostStatus::Published => Self::Published,
            BlogPostStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlContentStatus> for BlogPostStatus {
    fn from(status: GqlContentStatus) -> Self {
        match status {
            GqlContentStatus::Draft => BlogPostStatus::Draft,
            GqlContentStatus::Published => BlogPostStatus::Published,
            GqlContentStatus::Archived => BlogPostStatus::Archived,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(
    name = "BlogCommentModerationStatus",
    rename_items = "SCREAMING_SNAKE_CASE"
)]
pub enum GqlModerateCommentStatus {
    Approved,
    Spam,
    Trash,
}

impl From<GqlModerateCommentStatus> for DomainModerateCommentStatus {
    fn from(status: GqlModerateCommentStatus) -> Self {
        match status {
            GqlModerateCommentStatus::Approved => Self::Approved,
            GqlModerateCommentStatus::Spam => Self::Spam,
            GqlModerateCommentStatus::Trash => Self::Trash,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct GqlPost {
    #[graphql(skip)]
    pub tenant_id: Uuid,
    pub id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub body: Option<String>,
    pub body_format: String,
    pub content_json: Option<Value>,
    pub status: GqlContentStatus,
    pub author_id: Option<Uuid>,
    pub author_profile: Option<GqlProfileSummary>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Vec<String>,
}

#[derive(SimpleObject)]
pub struct GqlPublicCommentListItem {
    pub id: Uuid,
    pub effective_locale: String,
    pub author_id: Option<Uuid>,
    pub content_preview: String,
    pub parent_comment_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct GqlPublicCommentList {
    pub items: Vec<GqlPublicCommentListItem>,
    pub total: u64,
}

#[derive(SimpleObject)]
pub struct GqlModerationCommentListItem {
    pub id: Uuid,
    pub effective_locale: String,
    pub author_id: Option<Uuid>,
    pub content_preview: String,
    pub status: String,
    pub parent_comment_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct GqlModerationCommentList {
    pub items: Vec<GqlModerationCommentListItem>,
    pub total: u64,
}

#[ComplexObject]
impl GqlPost {
    /// Comments safe for public storefront rendering. The Comments owner applies
    /// approved-only visibility and pagination bounds before returning data.
    async fn public_comments(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> Result<GqlPublicCommentList> {
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let request_tenant = ctx.data::<TenantContext>()?;
        let requested_locale = comment_locale(locale.as_deref(), &self.effective_locale);
        let fallback_locale = post_comment_fallback_locale(request_tenant, self);

        let (items, total) = CommentService::new(db.clone(), event_bus.clone())
            .list_for_post_with_locale_fallback(
                self.tenant_id,
                SecurityContext::public_read(),
                self.id,
                ListCommentsFilter {
                    locale: Some(requested_locale),
                    page: page.unwrap_or(1).max(1),
                    per_page: per_page.unwrap_or(20).clamp(1, 100),
                },
                Some(fallback_locale),
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(GqlPublicCommentList {
            items: items.into_iter().map(Into::into).collect(),
            total,
        })
    }

    /// Full non-deleted comment queue for Blog moderators. Access is checked on
    /// the nested field so ordinary post readers cannot inspect pending/spam data.
    async fn moderation_comments(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> Result<GqlModerationCommentList> {
        let auth = require_comment_moderator(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let request_tenant = ctx.data::<TenantContext>()?;
        ensure_comment_tenant_binding(request_tenant, &auth, self.tenant_id)?;
        let requested_locale = comment_locale(locale.as_deref(), &self.effective_locale);

        let (items, total) = CommentService::new(db.clone(), event_bus.clone())
            .list_for_post_with_locale_fallback(
                self.tenant_id,
                SecurityContext::system(),
                self.id,
                ListCommentsFilter {
                    locale: Some(requested_locale),
                    page: page.unwrap_or(1).max(1),
                    per_page: per_page.unwrap_or(50).clamp(1, 100),
                },
                Some(request_tenant.default_locale.as_str()),
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(GqlModerationCommentList {
            items: items.into_iter().map(Into::into).collect(),
            total,
        })
    }
}

fn comment_locale(requested: Option<&str>, effective_locale: &str) -> String {
    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| effective_locale.to_string())
}

fn post_comment_fallback_locale<'a>(tenant: &'a TenantContext, post: &'a GqlPost) -> &'a str {
    if tenant.id == post.tenant_id {
        tenant.default_locale.as_str()
    } else {
        post.effective_locale.as_str()
    }
}

fn require_comment_moderator(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, &[Permission::BLOG_POSTS_MANAGE]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: blog_posts:manage required",
        ));
    }
    Ok(auth)
}

fn ensure_comment_tenant_binding(
    tenant: &TenantContext,
    auth: &AuthContext,
    post_tenant_id: Uuid,
) -> Result<()> {
    if tenant.id != post_tenant_id || auth.tenant_id != post_tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Blog comment moderation must use the current authenticated tenant",
        ));
    }
    Ok(())
}

#[derive(SimpleObject)]
pub struct GqlPostListItem {
    pub id: Uuid,
    pub title: String,
    pub effective_locale: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub status: GqlContentStatus,
    pub author_id: Option<Uuid>,
    pub author_profile: Option<GqlProfileSummary>,
    pub created_at: String,
    pub published_at: Option<String>,
    pub channel_slugs: Vec<String>,
}

#[derive(SimpleObject)]
pub struct GqlPostList {
    pub items: Vec<GqlPostListItem>,
    pub total: u64,
}

#[derive(InputObject)]
pub struct CreatePostInput {
    pub locale: String,
    pub title: String,
    pub body: String,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub excerpt: Option<String>,
    pub slug: Option<String>,
    pub publish: bool,
    pub tags: Vec<String>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct UpdatePostInput {
    pub locale: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub excerpt: Option<String>,
    pub slug: Option<String>,
    pub status: Option<GqlContentStatus>,
    pub tags: Option<Vec<String>>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct PostsFilter {
    pub status: Option<GqlContentStatus>,
    pub author_id: Option<Uuid>,
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl From<PostResponse> for GqlPost {
    fn from(post: PostResponse) -> Self {
        Self {
            tenant_id: post.tenant_id,
            id: post.id,
            requested_locale: post.requested_locale,
            effective_locale: post.effective_locale,
            available_locales: post.available_locales,
            title: post.title,
            slug: Some(post.slug),
            excerpt: post.excerpt,
            body: Some(post.body),
            body_format: post.body_format,
            content_json: post.content_json,
            status: match post.status {
                BlogPostStatus::Draft => GqlContentStatus::Draft,
                BlogPostStatus::Published => GqlContentStatus::Published,
                BlogPostStatus::Archived => GqlContentStatus::Archived,
            },
            author_id: Some(post.author_id),
            author_profile: None,
            created_at: post.created_at.to_rfc3339(),
            updated_at: post.updated_at.to_rfc3339(),
            published_at: post.published_at.map(|value| value.to_rfc3339()),
            tags: post.tags,
            featured_image_url: post.featured_image_url,
            seo_title: post.seo_title,
            seo_description: post.seo_description,
            channel_slugs: post.channel_slugs,
        }
    }
}

impl From<DomainCommentListItem> for GqlPublicCommentListItem {
    fn from(comment: DomainCommentListItem) -> Self {
        Self {
            id: comment.id,
            effective_locale: comment.effective_locale,
            author_id: comment.author_id,
            content_preview: comment.content_preview,
            parent_comment_id: comment.parent_comment_id,
            created_at: comment.created_at,
        }
    }
}

impl From<DomainCommentListItem> for GqlModerationCommentListItem {
    fn from(comment: DomainCommentListItem) -> Self {
        Self {
            id: comment.id,
            effective_locale: comment.effective_locale,
            author_id: comment.author_id,
            content_preview: comment.content_preview,
            status: comment.status,
            parent_comment_id: comment.parent_comment_id,
            created_at: comment.created_at,
        }
    }
}

impl From<PostSummary> for GqlPostListItem {
    fn from(item: PostSummary) -> Self {
        Self {
            id: item.id,
            title: item.title,
            effective_locale: item.effective_locale,
            slug: Some(item.slug),
            excerpt: item.excerpt,
            status: item.status.into(),
            author_id: Some(item.author_id),
            author_profile: None,
            created_at: item.created_at.to_rfc3339(),
            published_at: item.published_at.map(|value| value.to_rfc3339()),
            channel_slugs: item.channel_slugs,
        }
    }
}

impl From<CreatePostInput> for DomainCreatePostInput {
    fn from(input: CreatePostInput) -> Self {
        Self {
            locale: input.locale,
            title: input.title,
            body: input.body,
            body_format: input
                .body_format
                .unwrap_or_else(|| rustok_core::CONTENT_FORMAT_MARKDOWN.to_string()),
            content_json: input.content_json,
            excerpt: input.excerpt,
            slug: input.slug,
            publish: input.publish,
            tags: input.tags,
            category_id: input.category_id,
            featured_image_url: input.featured_image_url,
            seo_title: input.seo_title,
            seo_description: input.seo_description,
            channel_slugs: input.channel_slugs,
            metadata: None,
        }
    }
}
