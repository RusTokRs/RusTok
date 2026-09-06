use async_graphql::{ComplexObject, Context, Enum, FieldError, InputObject, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, RichTextDocument, RichTextView, TenantContext, graphql::GraphQLError,
    has_any_effective_permission,
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::graphql::GqlProfileSummary;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    BlogPostStatus, CommentListItem as DomainCommentListItem, CommentResponse,
    CreateCommentInput as DomainCreateCommentInput, CreatePostInput as DomainCreatePostInput,
    ListCommentsFilter, ModerateCommentStatus as DomainModerateCommentStatus, PostResponse,
    PostSummary, PublicCommentsAvailability, UpdatePostInput as DomainUpdatePostInput,
    list_public_comments_with_snapshot,
};

use super::runtime_data::BlogGraphqlRuntimeData;

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

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(
    name = "BlogCommentsAvailability",
    rename_items = "SCREAMING_SNAKE_CASE"
)]
pub enum GqlBlogCommentsAvailability {
    Available,
    Unavailable,
    Timeout,
}

impl From<PublicCommentsAvailability> for GqlBlogCommentsAvailability {
    fn from(availability: PublicCommentsAvailability) -> Self {
        match availability {
            PublicCommentsAvailability::Available => Self::Available,
            PublicCommentsAvailability::Unavailable => Self::Unavailable,
            PublicCommentsAvailability::Timeout => Self::Timeout,
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
    pub content: RichTextView,
    pub content_plain_text: String,
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
#[graphql(name = "BlogComment")]
pub struct GqlBlogComment {
    pub id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub post_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: RichTextView,
    pub content_plain_text: String,
    pub status: String,
    pub parent_comment_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlPublicCommentList {
    pub availability: GqlBlogCommentsAvailability,
    pub cached_snapshot: bool,
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
        let runtime = ctx.data::<BlogGraphqlRuntimeData>()?;
        let request_tenant = ctx.data::<TenantContext>()?;
        let requested_locale = comment_locale(locale.as_deref(), &self.effective_locale);
        let fallback_locale = post_comment_fallback_locale(request_tenant, self);
        let service = runtime.comment_service(db.clone(), event_bus.clone());
        let read = list_public_comments_with_snapshot(
            &service,
            runtime.public_comments_snapshot_store(),
            self.tenant_id,
            self.id,
            requested_locale.as_str(),
            Some(fallback_locale),
            page.unwrap_or(1),
            per_page.unwrap_or(20),
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(GqlPublicCommentList {
            availability: read.availability.into(),
            cached_snapshot: read.cached_snapshot,
            items: read.items.into_iter().map(Into::into).collect(),
            total: read.total,
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
        let runtime = ctx.data::<BlogGraphqlRuntimeData>()?;
        let request_tenant = ctx.data::<TenantContext>()?;
        ensure_comment_tenant_binding(request_tenant, &auth, self.tenant_id)?;
        let requested_locale = comment_locale(locale.as_deref(), &self.effective_locale);
        let service = runtime.comment_service(db.clone(), event_bus.clone());

        let (items, total) = service
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
    pub content: RichTextDocument,
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
#[graphql(name = "CreateBlogCommentInput")]
pub struct GqlCreateBlogCommentInput {
    pub locale: String,
    pub content: RichTextDocument,
    pub parent_comment_id: Option<Uuid>,
}

#[derive(InputObject)]
pub struct UpdatePostInput {
    pub locale: Option<String>,
    pub title: Option<String>,
    pub content: Option<RichTextDocument>,
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
            content: post.content,
            content_plain_text: post.content_plain_text,
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

impl From<CommentResponse> for GqlBlogComment {
    fn from(comment: CommentResponse) -> Self {
        Self {
            id: comment.id,
            requested_locale: comment.requested_locale,
            effective_locale: comment.effective_locale,
            post_id: comment.post_id,
            author_id: comment.author_id,
            content: comment.content,
            content_plain_text: comment.content_text,
            status: comment.status,
            parent_comment_id: comment.parent_comment_id,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

impl From<GqlCreateBlogCommentInput> for DomainCreateCommentInput {
    fn from(input: GqlCreateBlogCommentInput) -> Self {
        Self {
            locale: input.locale,
            content: input.content,
            parent_comment_id: input.parent_comment_id,
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
            content: input.content,
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

impl From<UpdatePostInput> for DomainUpdatePostInput {
    fn from(input: UpdatePostInput) -> Self {
        Self {
            locale: input.locale,
            title: input.title,
            content: input.content,
            excerpt: input.excerpt,
            slug: input.slug,
            tags: input.tags,
            category_id: input.category_id,
            featured_image_url: input.featured_image_url,
            seo_title: input.seo_title,
            seo_description: input.seo_description,
            channel_slugs: input.channel_slugs,
            metadata: None,
            version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DomainUpdatePostInput, UpdatePostInput};
    use rustok_api::RichTextDocument;
    use uuid::Uuid;

    #[test]
    fn update_post_input_conversion_preserves_canonical_content() {
        let canonical = RichTextDocument::single_paragraph("canonical update");
        let input = UpdatePostInput {
            locale: Some("ru".to_string()),
            title: Some("Заголовок".to_string()),
            content: Some(canonical.clone()),
            excerpt: Some("excerpt".to_string()),
            slug: Some("post".to_string()),
            status: None,
            tags: Some(vec!["tag".to_string()]),
            category_id: Some(Uuid::nil()),
            featured_image_url: Some("https://example.test/image.png".to_string()),
            seo_title: Some("SEO".to_string()),
            seo_description: Some("description".to_string()),
            channel_slugs: Some(vec!["web".to_string()]),
        };
        let domain: DomainUpdatePostInput = input.into();
        assert_eq!(domain.content, Some(canonical));
        assert_eq!(domain.category_id, Some(Uuid::nil()));
        assert!(domain.metadata.is_none());
        assert!(domain.version.is_none());
    }
}
