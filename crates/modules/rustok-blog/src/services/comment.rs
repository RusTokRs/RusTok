use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind,
    SharedStaticModuleSettingsReader,
};
use rustok_comments_api::{
    CommentListItem as ApiCommentListItem, CommentRecord as ApiCommentRecord,
    CommentStatus as ApiCommentStatus, CommentsThreadPort,
    CreateCommentInput as ApiCreateCommentInput, ListCommentsFilter as ApiListCommentsFilter,
    SetCommentStatusRequest as ApiSetCommentStatusRequest,
    UpdateCommentInput as ApiUpdateCommentInput,
};
use rustok_core::{SecurityActorKind, SecurityContext};
use std::sync::Arc;

use crate::BlogPostStatus;
use crate::domain::comment_policy::{
    BLOG_COMMENTS_MODE_SETTING, BlogCommentsMode, parse_comments_mode,
};
use crate::dto::{
    CommentListItem, CommentResponse, CreateCommentInput, ListCommentsFilter, ModerateCommentInput,
    UpdateCommentInput,
};
use crate::entities::{blog_comment_projection_delivery, blog_post, blog_post_channel_visibility};
use crate::error::{BlogError, BlogResult};
use crate::services::{is_post_visible_for_channel, post::storage_to_status, rbac::enforce_scope};

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
        if !snapshot.enabled {
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

    /// Creates a comment only when the owning Blog post is publicly visible in
    /// the current channel. Browser-facing transports must use this command so
    /// callers cannot submit an arbitrary Comments target or comment on drafts.
    #[instrument(skip(self, security, input))]
    pub async fn create_public_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        public_channel_slug: Option<&str>,
        input: CreateCommentInput,
    ) -> BlogResult<CommentResponse> {
        enforce_scope(&security, Resource::Comments, Action::Create)?;
        self.ensure_public_comment_creation_allowed(tenant_id, post_id, public_channel_slug)
            .await?;

        self.create_comment_internal(
            tenant_id,
            security,
            post_id,
            public_channel_slug,
            CommentTargetPolicy::Public,
            input,
        )
        .await
    }

    /// Creates a comment for an owner-facing actor.
    ///
    /// The target only has to be a Blog post of the tenant: a draft, an archived
    /// post and a post that is hidden from the storefront channels are valid
    /// targets here. The public publication/channel boundary and the public
    /// comments mode belong to [`Self::create_public_comment`]; applying them here
    /// would write the comment to the Comments owner and then compensate it away,
    /// so the comment would never survive the command.
    #[instrument(skip(self, security, input))]
    pub async fn create_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        input: CreateCommentInput,
    ) -> BlogResult<CommentResponse> {
        self.create_comment_internal(
            tenant_id,
            security,
            post_id,
            None,
            CommentTargetPolicy::Owner,
            input,
        )
        .await
    }

    async fn create_comment_internal(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        public_channel_slug: Option<&str>,
        target_policy: CommentTargetPolicy,
        input: CreateCommentInput,
    ) -> BlogResult<CommentResponse> {
        enforce_scope(&security, Resource::Comments, Action::Create)?;
        self.ensure_post_exists(tenant_id, post_id).await?;

        if security.user_id.is_none() {
            return Err(BlogError::AuthorRequired);
        }

        let command_id = input.command_id;
        if command_id.is_nil() {
            return Err(BlogError::validation("Command ID must not be nil"));
        }
        if let Some(parent_id) = input.parent_comment_id
            && parent_id.is_nil()
        {
            return Err(BlogError::validation("Parent comment ID must not be nil"));
        }

        let locale = input.locale.clone();
        let record = self
            .require_comments_thread_port()?
            .create_comment(
                comments_write_port_context(
                    tenant_id,
                    &security,
                    locale.as_str(),
                    "create",
                    post_id,
                    command_id,
                )?,
                ApiCreateCommentInput {
                    target_type: TARGET_TYPE_BLOG_POST.to_string(),
                    target_id: post_id,
                    locale,
                    body: input.content,
                    parent_comment_id: input.parent_comment_id,
                    status: ApiCommentStatus::Pending,
                },
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;

        // Blog and Comments are separate owner boundaries, so the pre-create target
        // check cannot be atomic with the external port call. Revalidate the full public
        // publication/channel boundary before reporting success. If the target became
        // unpublished, inactive, hidden from the current channel, or deleted while the
        // external write was in flight, compensate the already-created comment with a
        // fresh idempotent delete command. The terminal TargetDeleted event remains the
        // durable cross-owner cleanup backstop for the deletion case.
        //
        // An owner-facing write has no public boundary to revalidate: its target is
        // the Blog post itself, which was checked for existence before the write.
        if target_policy == CommentTargetPolicy::Owner {
            return Self::map_comment_record(record);
        }

        match self
            .ensure_public_comment_creation_allowed(tenant_id, post_id, public_channel_slug)
            .await
        {
            Ok(()) => {}
            Err(revalidation_error) => {
                // The Comments write already crossed an owner boundary. Any failure in
                // the final Blog visibility check means the create command cannot report
                // success without leaving a committed foreign-side effect behind.
                if let Err(compensation_error) = self
                    .compensate_created_public_comment(tenant_id, record.id)
                    .await
                {
                    return Err(BlogError::invariant(format!(
                        "Blog post {post_id} public visibility could not be confirmed after comment creation and the compensating comment delete failed: {compensation_error}"
                    )));
                }

                return Err(revalidation_error);
            }
        }

        Self::map_comment_record(record)
    }

    async fn compensate_created_public_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
    ) -> BlogResult<()> {
        // Deterministic per compensated comment: a retry after a lost response
        // replays the same idempotency key instead of issuing a fresh delete
        // command that could fail with NotFound and mask the original error.
        let compensation_command_id = comment_id;
        let system_security = SecurityContext::system();
        self.require_comments_thread_port()?
            .delete_comment(
                comments_write_port_context(
                    tenant_id,
                    &system_security,
                    PLATFORM_FALLBACK_LOCALE,
                    "delete-after-public-target-loss",
                    comment_id,
                    compensation_command_id,
                )?,
                comment_id,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        Ok(())
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
        enforce_scope(&security, Resource::Comments, Action::Read)?;
        let record = self
            .require_comments_thread_port()?
            .get_comment(
                comments_read_port_context(tenant_id, &security, locale, comment_id)?,
                comment_id,
                fallback_locale.map(str::to_owned),
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        let post_id = Self::ensure_blog_target(&record)?;
        self.ensure_post_exists(tenant_id, post_id).await?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
        input: UpdateCommentInput,
    ) -> BlogResult<CommentResponse> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if comment_id.is_nil() {
            return Err(BlogError::validation("Comment ID must not be nil"));
        }
        if input.command_id.is_nil() {
            return Err(BlogError::validation("Command ID must not be nil"));
        }
        enforce_scope(&security, Resource::Comments, Action::Update)?;
        let existing = self
            .require_comments_thread_port()?
            .get_comment(
                comments_read_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    input.locale.as_str(),
                    comment_id,
                )?,
                comment_id,
                None,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        let post_id = Self::ensure_blog_target(&existing)?;
        self.ensure_post_exists(tenant_id, post_id).await?;

        let locale = input.locale.clone();
        let domain_input = ApiUpdateCommentInput {
            locale: locale.clone(),
            body: input.content,
        };

        let record = self
            .require_comments_thread_port()?
            .update_comment(
                comments_write_port_context(
                    tenant_id,
                    &security,
                    locale.as_str(),
                    "update",
                    comment_id,
                    input.command_id,
                )?,
                comment_id,
                domain_input,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security, input))]
    pub async fn moderate_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
        input: ModerateCommentInput,
        fallback_locale: Option<&str>,
    ) -> BlogResult<CommentResponse> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if comment_id.is_nil() {
            return Err(BlogError::validation("Comment ID must not be nil"));
        }
        if input.command_id.is_nil() {
            return Err(BlogError::validation("Command ID must not be nil"));
        }
        enforce_scope(&security, Resource::BlogPosts, Action::Manage)?;

        let locale = input
            .locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(PLATFORM_FALLBACK_LOCALE);

        let existing = self
            .require_comments_thread_port()?
            .get_comment(
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
        let post_id = Self::ensure_blog_target(&existing)?;
        self.ensure_post_exists(tenant_id, post_id).await?;

        let record = self
            .require_comments_thread_port()?
            .set_comment_status(
                // The write crosses into the Comments owner under system
                // delegation (Blog already enforced `BlogPosts:Manage`
                // above), but the human moderator must stay attributable:
                // carry the actor identity as a port claim instead of
                // letting the audit trail end at "system".
                match security.user_id {
                    Some(moderator_id) => comments_write_port_context(
                        tenant_id,
                        &SecurityContext::system(),
                        locale,
                        "moderate",
                        comment_id,
                        input.command_id,
                    )?
                    .with_claim(format!("blog_moderator:{moderator_id}")),
                    None => comments_write_port_context(
                        tenant_id,
                        &SecurityContext::system(),
                        locale,
                        "moderate",
                        comment_id,
                        input.command_id,
                    )?,
                },
                comment_id,
                ApiSetCommentStatusRequest {
                    status: input.status.into(),
                    fallback_locale: fallback_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security))]
    pub async fn delete_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
        command_id: Uuid,
    ) -> BlogResult<()> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if comment_id.is_nil() {
            return Err(BlogError::validation("Comment ID must not be nil"));
        }
        if command_id.is_nil() {
            return Err(BlogError::validation("Command ID must not be nil"));
        }
        enforce_scope(&security, Resource::Comments, Action::Delete)?;
        let existing = self
            .require_comments_thread_port()?
            .get_comment(
                comments_read_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    PLATFORM_FALLBACK_LOCALE,
                    comment_id,
                )?,
                comment_id,
                None,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        let post_id = Self::ensure_blog_target(&existing)?;
        self.ensure_post_exists(tenant_id, post_id).await?;

        self.require_comments_thread_port()?
            .delete_comment(
                comments_write_port_context(
                    tenant_id,
                    &security,
                    PLATFORM_FALLBACK_LOCALE,
                    "delete",
                    comment_id,
                    command_id,
                )?,
                comment_id,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;

        Ok(())
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
        if !security.is_public_read() {
            enforce_scope(&security, Resource::Comments, Action::List)?;
            self.ensure_post_exists(tenant_id, post_id).await?;
        } else {
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
        let domain_filter = ApiListCommentsFilter {
            locale: locale.clone(),
            page: filter.page,
            per_page: filter.per_page,
        };

        let result = if security.is_public_read() {
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
        Ok((
            items
                .into_iter()
                .map(Self::map_comment_list_item)
                .collect::<Vec<_>>(),
            total,
        ))
    }

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

        if let Some(channel_slug) = public_channel_slug {
            let channel_service = rustok_channel::ChannelService::new(self.db.clone());
            let Some(channel) = channel_service
                .get_channel_by_slug(tenant_id, channel_slug)
                .await
                .map_err(|_| BlogError::post_not_found(post_id))?
            else {
                return Err(BlogError::post_not_found(post_id));
            };

            if !channel.is_active {
                return Err(BlogError::post_not_found(post_id));
            }

            let enabled = channel_service
                .is_module_enabled_for_tenant(tenant_id, channel.id, "blog")
                .await
                .map_err(|_| BlogError::post_not_found(post_id))?;
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
