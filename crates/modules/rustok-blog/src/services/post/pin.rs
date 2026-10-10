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

    async fn set_pinned(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        pinned: bool,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogPosts, Action::Update)?;
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;

        // Only published posts can be pinned
        if pinned && storage_to_status(&post.status)? != BlogPostStatus::Published {
            return Err(BlogError::validation("Only published posts can be pinned"));
        }

        let now = chrono::Utc::now();
        let pinned_at: Option<chrono::DateTime<chrono::FixedOffset>> =
            if pinned { Some(now.into()) } else { None };
        let updated_at: chrono::DateTime<chrono::FixedOffset> = now.into();

        blog_post::Entity::update_many()
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
                sea_orm::sea_query::Expr::value(updated_at),
            )
            .filter(blog_post::Column::Id.eq(post_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .exec(&self.db)
            .await
            .map_err(BlogError::Database)?;

        Ok(())
    }
}
