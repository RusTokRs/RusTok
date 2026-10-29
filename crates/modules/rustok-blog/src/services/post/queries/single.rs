use super::*;

impl PostService {
    pub async fn get_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        locale: &str,
    ) -> BlogResult<PostResponse> {
        self.get_post_with_locale_fallback(tenant_id, security, post_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_post_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostResponse> {
        self.get_post_with_locale_fallback_on_channel(
            tenant_id, security, post_id, locale, fallback_locale, None,
        )
        .await
    }

    /// Public readers are scoped to the channel and its enabled Blog module;
    /// tenant-wide editors are independent of storefront channel visibility.
    #[instrument(skip(self))]
    pub async fn get_post_with_locale_fallback_on_channel(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> BlogResult<PostResponse> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if post_id.is_nil() {
            return Err(BlogError::validation("Post ID must not be nil"));
        }
        enforce_scope(&security, Resource::BlogPosts, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let post = self.find_post(tenant_id, post_id).await?;
        // Public detail reads follow the same corpus contract as the public
        // list and Search/SEO projections. A published row missing its stamp
        // is damaged state, visible only to an editor who can repair it.
        if !can_read_non_public_posts(&security)
            && (storage_to_status(&post.status)? != BlogPostStatus::Published
                || post.published_at.is_none())
        {
            return Err(BlogError::forbidden("Permission denied"));
        }
        let translations = self.load_translations(tenant_id, post_id).await?;
        let channel_slugs = self.load_channel_slugs(tenant_id, post_id).await?;
        if !can_read_non_public_posts(&security)
            && (!self.channel_is_public_visible(tenant_id, channel_slug).await?
                || !is_post_visible_for_channel(&channel_slugs, channel_slug))
        {
            return Err(BlogError::forbidden("Permission denied"));
        }
        self.build_post_response(
            post,
            translations,
            channel_slugs,
            &locale,
            fallback_locale.as_deref(),
        )
        .await
    }

    /// Draft-only preview read used by token-authenticated preview requests.
    ///
    /// A preview token is the entire authorization of this read: the request has no
    /// principal, so the accessor is deliberately narrow instead of running the read
    /// under system authority. It resolves exactly one post of one tenant and returns
    /// `None` unless that post is currently a `Draft`, so a token stops resolving the
    /// moment the post is published, archived, or deleted.
    #[instrument(skip(self))]
    pub(crate) async fn get_draft_preview(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<PostResponse>> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if post_id.is_nil() {
            return Err(BlogError::validation("Post ID must not be nil"));
        }
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let Some(post) = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
        else {
            return Ok(None);
        };
        if storage_to_status(&post.status)? != BlogPostStatus::Draft {
            return Ok(None);
        }

        let translations = self.load_translations(tenant_id, post_id).await?;
        let channel_slugs = self.load_channel_slugs(tenant_id, post_id).await?;
        let response = self
            .build_post_response(
                post,
                translations,
                channel_slugs,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;
        Ok(Some(response))
    }

    /// Slug lookup of one post by its owner.
    ///
    /// **The channel allowlist is the caller's decision, not this method's.**
    /// The method answers with the post's *status* view only: `Published` for a
    /// caller without tenant-wide `blog_posts:read`, any status otherwise. A
    /// published post that is restricted to a subset of channels is still
    /// returned here, because the two callers apply the boundary at their own
    /// layer and need the row to answer correctly:
    ///
    /// * GraphQL `postBySlug` filters the result through `post_is_readable`
    ///   (channel-aware) before exposing it;
    /// * the SEO provider (`integrations/seo_targets.rs::resolve_route`) applies
    ///   its own load scope and `is_post_visible_for_channel`.
    ///
    /// Any new caller that serves a public surface must do the same or call
    /// [`Self::get_post`] under its own visibility rule — this method alone is
    /// not a public read.
    #[instrument(skip(self))]
    pub async fn get_post_by_slug(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
    ) -> BlogResult<Option<PostResponse>> {
        self.get_post_by_slug_with_locale_fallback(tenant_id, security, locale, slug, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_post_by_slug_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<PostResponse>> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        enforce_scope(&security, Resource::BlogPosts, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let normalized_slug = normalize_slug(slug);
        if normalized_slug.is_empty() {
            return Ok(None);
        }
        // Resolves the current slug first, then a route retired by a slug
        // change. The returned post always carries its current slug, so callers
        // can issue a permanent redirect when the requested slug was retired.
        let Some(post) = self
            .find_post_by_current_or_canonical_route(tenant_id, &normalized_slug)
            .await?
        else {
            return Ok(None);
        };

        if !can_read_non_public_posts(&security)
            && (storage_to_status(&post.status)? != BlogPostStatus::Published
                || post.published_at.is_none())
        {
            return Ok(None);
        }

        let translations = self.load_translations(tenant_id, post.id).await?;
        let channel_slugs = self.load_channel_slugs(tenant_id, post.id).await?;
        self.build_post_response(
            post,
            translations,
            channel_slugs,
            &locale,
            fallback_locale.as_deref(),
        )
        .await
        .map(Some)
    }

    #[instrument(skip(self, security))]
}
