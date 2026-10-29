use super::*;

impl CommentService {
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
    /// post and a channel-hidden post are valid targets. The public
    /// publication/channel policy is not applied to owner writes, but target
    /// existence is rechecked after the external Comments commit. If the post
    /// disappeared while the port call was in flight, the comment is deleted
    /// with the existing idempotent compensation command.
    #[instrument(skip(self, security, input))]
    pub async fn create_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        input: CreateCommentInput,
    ) -> BlogResult<CommentResponse> {
        // Owner writes bypass the public publication and comments-mode gates.
        // Only comment moderators may use that exception; COMMENT_CREATE alone
        // never authorizes writing to drafts, archived or channel-hidden posts.
        enforce_scope(&security, Resource::Comments, Action::Moderate)?;
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
        // Both write profiles must confirm the target still exists after the
        // external Comments commit. The owner profile does not require public
        // visibility, but a post deleted during the port call must not leave
        // an orphan comment after its TargetDeleted cleanup was delivered.
        let revalidated = match target_policy {
            CommentTargetPolicy::Owner => self.ensure_post_exists(tenant_id, post_id).await,
            CommentTargetPolicy::Public => {
                self.ensure_public_comment_creation_allowed(tenant_id, post_id, public_channel_slug)
                    .await
            }
        };
        match revalidated {
            Ok(()) => {}
            Err(revalidation_error) => {
                // The Comments write already crossed an owner boundary. Any failure in
                // the final Blog target check means the create command cannot report
                // success without leaving a committed foreign-side effect behind.
                if let Err(compensation_error) =
                    self.compensate_created_comment(tenant_id, record.id).await
                {
                    return Err(BlogError::invariant(format!(
                        "Blog post {post_id} target policy could not be confirmed after comment creation and the compensating comment delete failed: {compensation_error}"
                    )));
                }

                return Err(revalidation_error);
            }
        }

        Self::map_comment_record(record)
    }

    async fn compensate_created_comment(
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
        // Moderation is authority over a comment, not over the post: the
        // canonical grant is `comments:moderate` (Blog reaches comments through
        // the Comments port). Gating it on `blog_posts:manage` let a Blog
        // manager moderate a module it does not own and denied the operation to
        // a moderator who holds only the Comments grant.
        enforce_scope(&security, Resource::Comments, Action::Moderate)?;

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
                // delegation (Blog already enforced `comments:moderate`
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
}
