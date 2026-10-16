use super::*;

impl PostService {
    /// Pin a post to the top of public listings.
    /// Only published posts can be pinned.
    #[instrument(skip(self, security))]
    pub async fn pin_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        self.set_pinned(tenant_id, post_id, security, true).await
    }

    /// Unpin a post from the top of public listings.
    #[instrument(skip(self, security))]
    pub async fn unpin_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        self.set_pinned(tenant_id, post_id, security, false).await
    }

    /// The one write path of Blog pin state.
    ///
    /// Pin state is presentation state owned by this command, not part of the post
    /// revision: `CreatePostInput`/`UpdatePostInput` cannot set it, so no writer can
    /// persist `is_pinned` without `pinned_at` and no writer bypasses the
    /// published-only rule. The mutation runs in one transaction on a row-locked
    /// read, so a concurrent delete cannot leave a silently successful pin.
    ///
    /// `version` and the search projection are intentionally untouched: the public
    /// ordering reads `is_pinned`/`pinned_at` from the post row, and the Blog Search
    /// document payload does not project pin state. A pin therefore changes listing
    /// order only, and the module documents that boundary in `README.md`.
    ///
    /// The statement names the three pin columns instead of saving the read entity
    /// back. A whole-entity write would carry every column of `blog_posts` — including
    /// `status`, `published_at`, `archived_at`, `scheduled_at` and `version` — so pin
    /// state could only stay out of the lifecycle by relying on the values it had just
    /// read. Writing `is_pinned`/`pinned_at`/`updated_at` explicitly makes the
    /// lifecycle columns out of reach for this command by construction, including for
    /// any writer that reaches this path without the row lock.
    async fn set_pinned(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        pinned: bool,
    ) -> BlogResult<()> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if post_id.is_nil() {
            return Err(BlogError::validation("Post ID must not be nil"));
        }
        enforce_scope(&security, Resource::BlogPosts, Action::Update)?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let post = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(BlogError::from)?
            .ok_or(BlogError::PostNotFound(post_id))?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;
        if pinned {
            validate_pin_target(storage_to_status(&post.status)?)?;
        }

        let now = chrono::Utc::now();
        let pinned_at: Option<chrono::DateTime<chrono::FixedOffset>> =
            pinned.then(|| now.fixed_offset());
        let result = blog_post::Entity::update_many()
            .col_expr(
                blog_post::Column::IsPinned,
                sea_orm::sea_query::Expr::value(pinned),
            )
            .col_expr(
                blog_post::Column::PinnedAt,
                sea_orm::sea_query::Expr::value(pinned_at),
            )
            .col_expr(
                blog_post::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now.fixed_offset()),
            )
            .filter(blog_post::Column::Id.eq(post_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .exec(&txn)
            .await
            .map_err(BlogError::from)?;
        if result.rows_affected != 1 {
            return Err(BlogError::conflict(
                "Blog post disappeared while its pin state was being written",
            ));
        }

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }
}

/// Only published posts can be pinned. Unpinning stays available for every status so
/// an archived post can always be released from the pinned block.
pub(crate) fn validate_pin_target(status: BlogPostStatus) -> BlogResult<()> {
    if status == BlogPostStatus::Published {
        Ok(())
    } else {
        Err(BlogError::validation("Only published posts can be pinned"))
    }
}

#[cfg(test)]
mod pin_tests {
    use super::validate_pin_target;
    use crate::state_machine::BlogPostStatus;

    #[test]
    fn only_published_posts_can_be_pinned() {
        assert!(validate_pin_target(BlogPostStatus::Published).is_ok());
        assert!(validate_pin_target(BlogPostStatus::Draft).is_err());
        assert!(validate_pin_target(BlogPostStatus::Archived).is_err());
    }
}
