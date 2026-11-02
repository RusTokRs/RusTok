use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind,
    SharedStaticModuleSettingsReader,
};
use rustok_comments::{
    CommentListItem as ApiCommentListItem, CommentRecord as ApiCommentRecord,
    CommentStatus as ApiCommentStatus, CommentsThreadPort,
    CreateCommentInput as ApiCreateCommentInput, ListCommentsFilter as ApiListCommentsFilter,
    SetCommentStatusRequest as ApiSetCommentStatusRequest,
    UpdateCommentInput as ApiUpdateCommentInput,
};
use rustok_core::{PermissionScope, SecurityActorKind, SecurityContext};
use std::sync::Arc;

use crate::BlogPostStatus;
use crate::domain::comment_policy::{
    BLOG_COMMENTS_MODE_SETTING, BlogCommentsMode, parse_comments_mode,
};
use crate::dto::{
    CommentListItem, CommentResponse, CreateCommentInput, ListCommentsFilter, ModerateCommentInput,
    UpdateCommentInput, bounded_comment_page,
};
use crate::entities::{blog_comment_projection_delivery, blog_post, blog_post_channel_visibility};
use crate::error::{BlogError, BlogResult};
use crate::services::{
    is_post_visible_for_channel,
    post::{normalize_public_channel_slug, storage_to_status},
    rbac::enforce_scope,
};

mod commands;

const TARGET_TYPE_BLOG_POST: &str = "blog_post";
const PUBLIC_COMMENTS_PORT_ACTOR: &str = "rustok-blog.public-comments";

/// Which target boundary a comment write has to satisfy.
///
/// The public boundary is a *publication* rule: the owning Blog post must be
/// publicly visible in the current channel, and the tenant's public comments mode
/// must allow the write. The owner boundary is a *ownership* rule: the target must
/// be a Blog post of the tenant, and a draft, an archived post or a post that is
/// hidden from the storefront channels are valid targets.
///
/// The two boundaries must not be mixed: a write that satisfies the public rule and
/// then re-validates it would be committed to the Comments owner first and
/// compensated away afterwards, so the comment would never survive the command.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentTargetPolicy {
    /// Browser-facing write: the post must be publicly visible in the channel.
    Public,
    /// Owner-facing write: the post must exist in the tenant.
    Owner,
}

pub struct CommentService {
    db: DatabaseConnection,
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
    settings_reader: Option<SharedStaticModuleSettingsReader>,
}

impl CommentService {
    /// Builds the Blog comment adapter from host-composed capabilities.
    ///
    /// The Comments port is optional because Blog remains useful without the
    /// provider. The settings reader is required only for public Blog comment
    /// operations; if it is unavailable those operations fail closed.
    pub fn from_runtime_capabilities(
        db: DatabaseConnection,
        comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
        settings_reader: Option<SharedStaticModuleSettingsReader>,
    ) -> Self {
        Self {
            db,
            comments_thread_port,
            settings_reader,
        }
    }

    /// Returns the active CommentsThreadPort or fails with COMMENTS_PROVIDER_UNAVAILABLE.
    fn require_comments_thread_port(&self) -> BlogResult<&dyn CommentsThreadPort> {
        self.comments_thread_port
            .as_deref()
            .ok_or(BlogError::CommentsUnavailable)
    }

    pub(crate) async fn public_comments_mode(
        &self,
        tenant_id: Uuid,
    ) -> BlogResult<BlogCommentsMode> {
        let Some(settings_reader) = self.settings_reader.as_ref() else {
            return Err(BlogError::CommentsPolicyUnavailable);
        };
        let snapshot = settings_reader
            .settings(tenant_id, "blog")
            .await
            .map_err(|_| BlogError::CommentsPolicyUnavailable)?;
        let Some(snapshot) = snapshot else {
            return Ok(BlogCommentsMode::Disabled);
        };
        if snapshot.enabled != Some(true) {
            return Ok(BlogCommentsMode::Disabled);
        }

        parse_comments_mode(&snapshot.settings).map_err(|_| {
            tracing::warn!(
                tenant_id = %tenant_id,
                setting = BLOG_COMMENTS_MODE_SETTING,
                "Blog comments policy could not be parsed; failing closed"
            );
            BlogError::CommentsPolicyUnavailable
        })
    }

    async fn ensure_public_comment_creation_allowed(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        public_channel_slug: Option<&str>,
    ) -> BlogResult<()> {
        match self.public_comments_mode(tenant_id).await? {
            BlogCommentsMode::Open => {}
            BlogCommentsMode::Disabled => return Err(BlogError::CommentsDisabled),
            BlogCommentsMode::ReadOnly => return Err(BlogError::CommentsReadOnly),
        }
        self.ensure_public_post_visible(tenant_id, post_id, public_channel_slug)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn get_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
        locale: &str,
    ) -> BlogResult<CommentResponse> {
        self.get_comment_with_locale_fallback(tenant_id, security, comment_id, locale, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn get_comment_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<CommentResponse> {
        self.get_comment_with_locale_fallback_on_channel(
            tenant_id,
            security,
            comment_id,
            locale,
            fallback_locale,
            None,
        )
        .await
    }

    /// Ordinary readers may inspect their own comments; others' unpublished
    /// moderation entries are never exposed by a generic COMMENTS_READ grant.
    #[instrument(skip(self, security))]
    pub async fn get_comment_with_locale_fallback_on_channel(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> BlogResult<CommentResponse> {
        enforce_scope(&security, Resource::Comments, Action::Read)?;
        let record = self
            .require_comments_thread_port()?
            .get_comment(
                // The Comments owner deliberately restricts direct reads of
                // someone else's entry. Blog loads with system authority,
                // then applies its own author/moderator/approved-post policy
                // before returning a byte to the caller.
                comments_read_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    locale,
                    comment_id,
                )?,
                comment_id,
                fallback_locale.map(str::to_owned),
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        let post_id = Self::ensure_blog_target(&record)?;
        self.ensure_post_exists(tenant_id, post_id).await?;
        match comment_detail_read_policy(&security, record.author_id, record.status) {
            CommentDetailReadPolicy::Owner => {}
            CommentDetailReadPolicy::Denied => {
                return Err(BlogError::comment_not_found(comment_id));
            }
            CommentDetailReadPolicy::Public => {
                if self.public_comments_mode(tenant_id).await? == BlogCommentsMode::Disabled {
                    return Err(BlogError::comment_not_found(comment_id));
                }
                match self
                    .ensure_public_post_visible(tenant_id, post_id, public_channel_slug)
                    .await
                {
                    Ok(()) => {}
                    Err(BlogError::PostNotFound(_)) => {
                        return Err(BlogError::comment_not_found(comment_id));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        filter: ListCommentsFilter,
    ) -> BlogResult<(Vec<CommentListItem>, u64)> {
        self.list_for_post_with_locale_fallback(tenant_id, security, post_id, filter, None, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_post_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> BlogResult<(Vec<CommentListItem>, u64)> {
        // COMMENTS_LIST is not a moderation grant. Without COMMENTS_MODERATE
        // an authenticated customer must use exactly the same approved-only,
        // published/channel-visible Comments projection as an anonymous reader.
        let owner_read = may_moderate_comments(&security);
        if owner_read {
            enforce_scope(&security, Resource::Comments, Action::List)?;
            self.ensure_post_exists(tenant_id, post_id).await?;
        } else {
            // Anonymous storefront reads already use the dedicated public
            // Comments port and need no authenticated COMMENTS_LIST grant.
            if !security.is_public_read() {
                enforce_scope(&security, Resource::Comments, Action::List)?;
            }
            if self.public_comments_mode(tenant_id).await? == BlogCommentsMode::Disabled {
                return Err(BlogError::CommentsDisabled);
            }
            // The public boundary lives in the owner service, not only in its
            // transports: a public reader may list comments only for a post
            // that is published and visible in the current channel. Existence
            // alone would leak the comment thread of drafts, archived posts
            // and channel-hidden posts to anyone who learns the post id.
            self.ensure_public_post_visible(tenant_id, post_id, public_channel_slug)
                .await?;
        }

        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let (page, per_page) = bounded_comment_page(filter.page, filter.per_page);
        let domain_filter = ApiListCommentsFilter {
            locale: locale.clone(),
            page,
            per_page,
        };

        let result = if !owner_read {
            self.require_comments_thread_port()?
                .list_public_comments_for_target(
                    comments_public_read_port_context(tenant_id, locale.as_str(), post_id),
                    TARGET_TYPE_BLOG_POST.to_string(),
                    post_id,
                    domain_filter,
                    fallback_locale.map(str::to_owned),
                )
                .await
        } else {
            self.require_comments_thread_port()?
                .list_comments_for_target(
                    comments_read_port_context(tenant_id, &security, locale.as_str(), post_id)?,
                    TARGET_TYPE_BLOG_POST.to_string(),
                    post_id,
                    domain_filter,
                    fallback_locale.map(str::to_owned),
                )
                .await
        };

        let (items, total) = result.map_err(comments_port_error_to_blog_error)?;
        if !owner_read {
            // A post can be unpublished, moved to another channel or have its
            // comments disabled while the Comments owner request is in flight.
            // Do not return a once-authorized thread after the owner read
            // finishes without revalidating the current public boundary.
            if self.public_comments_mode(tenant_id).await? == BlogCommentsMode::Disabled {
                return Err(BlogError::CommentsDisabled);
            }
            self.ensure_public_post_visible(tenant_id, post_id, public_channel_slug)
                .await?;
        }
        Ok((
            items
                .into_iter()
                .map(Self::map_comment_list_item)
                .collect::<Vec<_>>(),
            total,
        ))
    }

    /// Resolves the Blog post a comment operation targets.
    ///
    /// Module enablement is **not** re-checked here. It is a transport-level
    /// gate (`ensure_blog_module_enabled`, `require_module_enabled`) that the
    /// host applies before Blog services are reachable, backed by a
    /// host-owned `tenant_modules` table that a domain service cannot portably
    /// depend on (module migrations do not create it, and the module must stay
    /// usable in hosts that scope enablement differently). The check this
    /// service does own is that the target row exists *in this tenant*.
    async fn ensure_post_exists(&self, tenant_id: Uuid, post_id: Uuid) -> BlogResult<()> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if post_id.is_nil() {
            return Err(BlogError::validation("Post ID must not be nil"));
        }
        let exists = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
            .is_some();
        if !exists {
            return Err(BlogError::post_not_found(post_id));
        }
        Ok(())
    }

    pub(crate) async fn public_comments_projection_cursor(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
    ) -> BlogResult<i64> {
        blog_comment_projection_delivery::Entity::find()
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(tenant_id))
            .filter(blog_comment_projection_delivery::Column::PostId.eq(post_id))
            .order_by_desc(blog_comment_projection_delivery::Column::ProjectionRevision)
            .one(&self.db)
            .await
            .map(|delivery| delivery.map(|row| row.projection_revision).unwrap_or(0))
            .map_err(BlogError::from)
    }

    pub(crate) async fn ensure_public_post_visible(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        public_channel_slug: Option<&str>,
    ) -> BlogResult<()> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if post_id.is_nil() {
            return Err(BlogError::validation("Post ID must not be nil"));
        }
        let Some(post) = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
        else {
            return Err(BlogError::post_not_found(post_id));
        };

        let status = storage_to_status(&post.status)?;
        // "Public" means published *with* its publication timestamp, as in
        // the public list and the search projection; without it the row is
        // damaged state, not a public comment target.
        if status == BlogPostStatus::Published && post.published_at.is_none() {
            return Err(BlogError::post_not_found(post_id));
        }
        let channel_slugs = blog_post_channel_visibility::Entity::find()
            .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(blog_post_channel_visibility::Column::PostId.eq(post_id))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?
            .into_iter()
            .map(|item| item.channel_slug)
            .collect::<Vec<_>>();

        if !is_public_comment_target(status, &channel_slugs, public_channel_slug) {
            return Err(BlogError::post_not_found(post_id));
        }

        if let Some(channel_slug) = normalize_public_channel_slug(public_channel_slug) {
            let channel_service = rustok_channel::ChannelService::new(self.db.clone());
            let Some(channel) = channel_service
                .get_channel_by_slug(tenant_id, channel_slug.as_str())
                .await
                .map_err(BlogError::from)?
            else {
                return Err(BlogError::post_not_found(post_id));
            };

            if !channel.is_active {
                return Err(BlogError::post_not_found(post_id));
            }

            let enabled = channel_service
                .is_module_enabled_for_tenant(tenant_id, channel.id, "blog")
                .await
                .map_err(BlogError::from)?;
            if !enabled {
                return Err(BlogError::post_not_found(post_id));
            }
        }

        Ok(())
    }

    fn ensure_blog_target(record: &ApiCommentRecord) -> BlogResult<Uuid> {
        if record.target_type != TARGET_TYPE_BLOG_POST {
            return Err(BlogError::comment_not_found(record.id));
        }
        Ok(record.target_id)
    }

    fn map_comment_record(record: ApiCommentRecord) -> BlogResult<CommentResponse> {
        let post_id = Self::ensure_blog_target(&record)?;
        let requested_locale = record.requested_locale.clone();
        let effective_locale = record.effective_locale.clone();
        Ok(CommentResponse {
            id: record.id,
            requested_locale: requested_locale.clone(),
            locale: requested_locale,
            effective_locale,
            post_id,
            author_id: Some(record.author_id),
            content: record.body,
            content_text: record.body_text,
            status: comment_status_label(record.status).to_string(),
            parent_comment_id: record.parent_comment_id,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    fn map_comment_list_item(item: ApiCommentListItem) -> CommentListItem {
        CommentListItem {
            id: item.id,
            locale: item.requested_locale,
            effective_locale: item.effective_locale,
            post_id: item.target_id,
            author_id: Some(item.author_id),
            content_preview: item.body_preview,
            status: comment_status_label(item.status).to_string(),
            parent_comment_id: item.parent_comment_id,
            created_at: item.created_at,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommentDetailReadPolicy {
    Owner,
    Public,
    Denied,
}

/// An ordinary caller may read their own pending comment, but someone else's
/// comment is readable only if approved and its post passes current public
/// visibility. Moderator reads remain independent of the storefront channel.
fn comment_detail_read_policy(
    security: &SecurityContext,
    author_id: Uuid,
    status: ApiCommentStatus,
) -> CommentDetailReadPolicy {
    if may_moderate_comments(security) || security.user_id == Some(author_id) {
        CommentDetailReadPolicy::Owner
    } else if status == ApiCommentStatus::Approved {
        CommentDetailReadPolicy::Public
    } else {
        CommentDetailReadPolicy::Denied
    }
}

/// Only a moderation grant permits reading the owner queue (including
/// pending, spam and trash). Generic read/list grants are not escalation.
fn may_moderate_comments(security: &SecurityContext) -> bool {
    !matches!(
        security.get_scope(Resource::Comments, Action::Moderate),
        PermissionScope::None
    )
}

fn is_public_comment_target(
    status: BlogPostStatus,
    channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    status == BlogPostStatus::Published
        && is_post_visible_for_channel(channel_slugs, public_channel_slug)
}

fn comment_status_label(status: ApiCommentStatus) -> &'static str {
    match status {
        ApiCommentStatus::Pending => "pending",
        ApiCommentStatus::Approved => "approved",
        ApiCommentStatus::Spam => "spam",
        ApiCommentStatus::Trash => "trash",
    }
}

fn comments_write_port_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    locale: &str,
    operation: &str,
    resource_id: Uuid,
    command_id: Uuid,
) -> BlogResult<PortContext> {
    if command_id.is_nil() {
        return Err(BlogError::validation(
            "Comment command_id must be a non-nil UUID",
        ));
    }
    comments_port_context(
        tenant_id,
        security,
        locale,
        operation,
        resource_id,
        Some(command_id),
    )
}

fn comments_read_port_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    locale: &str,
    resource_id: Uuid,
) -> BlogResult<PortContext> {
    comments_port_context(tenant_id, security, locale, "read", resource_id, None)
}

fn comments_public_read_port_context(tenant_id: Uuid, locale: &str, post_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(PUBLIC_COMMENTS_PORT_ACTOR),
        locale,
        format!("blog-comment:public-list:{post_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn comments_port_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    locale: &str,
    operation: &str,
    resource_id: Uuid,
    command_id: Option<Uuid>,
) -> BlogResult<PortContext> {
    if tenant_id.is_nil() {
        return Err(BlogError::validation("Tenant ID must not be nil"));
    }
    if resource_id.is_nil() {
        return Err(BlogError::validation("Resource ID must not be nil"));
    }
    let actor = match security.actor_kind {
        SecurityActorKind::System => PortActor::system(),
        SecurityActorKind::User => PortActor::user(
            security
                .user_id
                .ok_or(BlogError::AuthorRequired)?
                .to_string(),
        ),
        SecurityActorKind::Service if command_id.is_none() => PortActor::service(
            security
                .user_id
                .ok_or_else(|| BlogError::forbidden("service comment reads require an actor id"))?
                .to_string(),
        ),
        SecurityActorKind::Service | SecurityActorKind::Public => {
            return Err(BlogError::forbidden(
                "comment writes require an authenticated user or system actor",
            ));
        }
    };

    let correlation_id = format!("blog-comment:{operation}:{resource_id}");
    let mut context =
        PortContext::new(tenant_id.to_string(), actor, locale, correlation_id.clone())
            .with_deadline(std::time::Duration::from_secs(2));
    if let Some(command_id) = command_id {
        context = context.with_idempotency_key(format!("{correlation_id}:command:{command_id}"));
    }

    if security.actor_kind != SecurityActorKind::System {
        context = context.with_role(security.role.to_string());
        for permission in security.permissions() {
            context = context.with_claim(permission.to_string());
        }
    }

    Ok(context)
}

fn comments_port_error_to_blog_error(error: PortError) -> BlogError {
    let kind = match error.kind {
        PortErrorKind::Validation => rustok_core::error::ErrorKind::Validation,
        PortErrorKind::NotFound => rustok_core::error::ErrorKind::NotFound,
        PortErrorKind::Conflict => rustok_core::error::ErrorKind::Conflict,
        PortErrorKind::Forbidden => rustok_core::error::ErrorKind::Forbidden,
        PortErrorKind::Unavailable => rustok_core::error::ErrorKind::ExternalService,
        PortErrorKind::Timeout => rustok_core::error::ErrorKind::Timeout,
        PortErrorKind::InvariantViolation => rustok_core::error::ErrorKind::Internal,
    };
    BlogError::Rich(Box::new(
        rustok_core::error::RichError::new(kind, error.message).with_error_code(error.code),
    ))
}

#[cfg(test)]
mod rich_content_tests;

#[cfg(test)]
mod public_target_tests;

#[cfg(test)]
mod comment_access_tests {
    use super::*;
    use rustok_api::Permission;
    use rustok_core::UserRole;

    #[test]
    fn ordinary_comment_grants_do_not_enable_mod_queue() {
        let commenter = Uuid::new_v4();
        let other_author = Uuid::new_v4();
        let reader = SecurityContext::from_permissions(
            UserRole::Customer,
            Some(commenter),
            [
                Permission::COMMENTS_READ,
                Permission::COMMENTS_LIST,
                Permission::COMMENTS_CREATE,
            ],
        );
        assert!(!may_moderate_comments(&reader));
        assert_eq!(
            comment_detail_read_policy(&reader, other_author, ApiCommentStatus::Pending),
            CommentDetailReadPolicy::Denied
        );
        assert_eq!(
            comment_detail_read_policy(&reader, other_author, ApiCommentStatus::Spam),
            CommentDetailReadPolicy::Denied
        );
        assert_eq!(
            comment_detail_read_policy(&reader, other_author, ApiCommentStatus::Trash),
            CommentDetailReadPolicy::Denied
        );
        assert_eq!(
            comment_detail_read_policy(&reader, other_author, ApiCommentStatus::Approved),
            CommentDetailReadPolicy::Public,
            "approved comments still require the live Blog post/channel check"
        );
        assert_eq!(
            comment_detail_read_policy(&reader, commenter, ApiCommentStatus::Pending),
            CommentDetailReadPolicy::Owner,
            "an author retains access to their own moderation-pending reply"
        );
    }

    #[test]
    fn moderator_can_read_the_owner_queue() {
        let moderator = SecurityContext::from_permissions(
            UserRole::Manager,
            Some(Uuid::new_v4()),
            [
                Permission::COMMENTS_READ,
                Permission::COMMENTS_LIST,
                Permission::COMMENTS_MODERATE,
            ],
        );
        assert!(may_moderate_comments(&moderator));
        assert_eq!(
            comment_detail_read_policy(&moderator, Uuid::new_v4(), ApiCommentStatus::Trash),
            CommentDetailReadPolicy::Owner
        );
    }

    #[tokio::test]
    async fn comment_create_grant_cannot_bypass_owner_draft_policy() {
        let (db, events) = crate::tests::hardening::setup_hardening_schema().await;
        let tenant_id = Uuid::new_v4();
        let post_id =
            crate::tests::hardening::create_draft_post(&db, &events, tenant_id, "private-comment")
                .await;
        let commenter = SecurityContext::from_permissions(
            UserRole::Customer,
            Some(Uuid::new_v4()),
            [Permission::COMMENTS_CREATE],
        );
        let service = CommentService::from_runtime_capabilities(db, None, None);

        let error = service
            .create_comment(
                tenant_id,
                commenter,
                post_id,
                CreateCommentInput {
                    command_id: Uuid::new_v4(),
                    locale: "en".to_string(),
                    content: crate::richtext::article_document_from_plain_text("private"),
                    parent_comment_id: None,
                },
            )
            .await
            .expect_err("owner-only comment create needs moderation authority");
        assert!(matches!(error, BlogError::Forbidden(_)));
    }
}
