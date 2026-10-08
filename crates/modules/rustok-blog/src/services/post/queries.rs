use rustok_api::PLATFORM_FALLBACK_LOCALE;

use super::*;

impl PostService {
    #[instrument(skip(self))]
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
        enforce_scope(&security, Resource::BlogPosts, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let post = self.find_post(tenant_id, post_id).await?;
        if !can_read_non_public_posts(&security)
            && storage_to_status(&post.status)? != BlogPostStatus::Published
        {
            return Err(BlogError::forbidden("Permission denied"));
        }
        let translations = self.load_translations(tenant_id, post_id).await?;
        let channel_slugs = self.load_channel_slugs(tenant_id, post_id).await?;
        self.build_post_response(
            post,
            translations,
            channel_slugs,
            &locale,
            fallback_locale.as_deref(),
        )
        .await
    }

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

        if storage_to_status(&post.status)? != BlogPostStatus::Published
            && !can_read_non_public_posts(&security)
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
    pub async fn list_posts(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
    ) -> BlogResult<PostListResponse> {
        self.list_posts_with_locale_fallback(tenant_id, security, query, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn list_posts_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostListResponse> {
        enforce_scope(&security, Resource::BlogPosts, Action::List)?;
        let locale = query
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let tag_filter = query.tag.clone();
        let mut select =
            blog_post::Entity::find().filter(blog_post::Column::TenantId.eq(tenant_id));

        if let Some(ref tag) = tag_filter {
            let Some(tag_id) = resolve_tag_id_for_posts(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?
            else {
                return Ok(PostListResponse::new(Vec::new(), 0, &query));
            };
            select = apply_tag_filter(select, tenant_id, tag_id);
        }

        if let Some(status) = query.status {
            select = select.filter(blog_post::Column::Status.eq(status_to_storage(status)));
        } else if !can_read_non_public_posts(&security) {
            select = select
                .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)));
        }
        if !can_read_non_public_posts(&security)
            && matches!(query.status, Some(status) if status != BlogPostStatus::Published)
        {
            return Ok(PostListResponse::new(Vec::new(), 0, &query));
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(blog_post::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(blog_post::Column::CategoryId.eq(category_id));
        }

        select = apply_post_sort(select, &query);

        let paginator = select.paginate(&self.db, query.per_page() as u64);
        let total = paginator.num_items().await.map_err(BlogError::from)?;
        let posts = paginator
            .fetch_page((query.page().saturating_sub(1)) as u64)
            .await
            .map_err(BlogError::from)?;
        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();

        let translations_map = self.load_translations_map(tenant_id, &post_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(tenant_id, &post_ids).await?;
        let tags_map = load_post_tags_map(
            &self.db,
            tenant_id,
            &post_ids,
            &locale,
            fallback_locale.as_deref(),
        )
        .await?;
        let category_ids = posts
            .iter()
            .filter_map(|post| post.category_id)
            .collect::<Vec<_>>();
        let category_names_map = self
            .load_category_names_map(
                tenant_id,
                &category_ids,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;

        let mut items = Vec::with_capacity(posts.len());
        for post in posts {
            Self::validate_persisted_version(&post)?;
            let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
            if translations.is_empty() {
                return Err(BlogError::invariant(format!(
                    "Blog post {} has no localized records",
                    post.id
                )));
            }
            let resolved =
                resolve_translation_record(&translations, &locale, fallback_locale.as_deref());
            let translation = resolved.translation.ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog post {} locale resolver returned no translation from a non-empty set",
                    post.id
                ))
            })?;
            let tags = tags_map.get(&post.id).cloned().unwrap_or_default();

            items.push(PostSummary {
                id: post.id,
                title: translation.title.clone(),
                slug: post.slug.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&translations, |item| {
                    item.locale.as_str()
                }),
                excerpt: translation.excerpt.clone(),
                status: storage_to_status(&post.status)?,
                author_id: post.author_id,
                author_name: None,
                category_id: post.category_id,
                category_name: post
                    .category_id
                    .and_then(|category_id| category_names_map.get(&category_id).cloned()),
                tags,
                featured_image_url: post.featured_image_url.clone(),
                channel_slugs: channel_slugs_map.get(&post.id).cloned().unwrap_or_default(),
                comment_count: post.comment_count as i64,
                published_at: post.published_at.map(Into::into),
                created_at: post.created_at.into(),
            });
        }

        Ok(PostListResponse::new(items, total, &query))
    }

    /// Public channel gate. Returns `false` when the channel is unknown, inactive,
    /// or does not enable Blog; the caller then returns an empty page.
    async fn channel_is_public_visible(
        &self,
        tenant_id: Uuid,
        channel_slug: Option<&str>,
    ) -> BlogResult<bool> {
        let Some(channel_slug) = normalize_public_channel_slug(channel_slug) else {
            return Ok(true);
        };
        let channel_service = rustok_channel::ChannelService::new(self.db.clone());
        let Some(channel) = channel_service
            .get_channel_by_slug(tenant_id, channel_slug.as_str())
            .await
            .map_err(BlogError::from)?
        else {
            return Ok(false);
        };
        Ok(channel.is_active
            && channel_service
                .is_module_enabled_for_tenant(tenant_id, channel.id, "blog")
                .await
                .map_err(BlogError::from)?)
    }

    /// Builds public summaries for already-filtered posts: translations, channel
    /// slugs, tags and category names are loaded in batch for the page only.
    async fn summarize_posts(
        &self,
        tenant_id: Uuid,
        posts: Vec<blog_post::Model>,
        locale: String,
        fallback_locale: Option<String>,
    ) -> BlogResult<Vec<PostSummary>> {
        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();

        let translations_map = self.load_translations_map(tenant_id, &post_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(tenant_id, &post_ids).await?;
        let tags_map = load_post_tags_map(
            &self.db,
            tenant_id,
            &post_ids,
            &locale,
            fallback_locale.as_deref(),
        )
        .await?;
        let category_ids = posts
            .iter()
            .filter_map(|post| post.category_id)
            .collect::<Vec<_>>();
        let category_names_map = self
            .load_category_names_map(
                tenant_id,
                &category_ids,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;

        let mut items = Vec::with_capacity(posts.len());
        for post in posts {
            Self::validate_persisted_version(&post)?;
            let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
            if translations.is_empty() {
                return Err(BlogError::invariant(format!(
                    "Blog post {} has no localized records",
                    post.id
                )));
            }
            let resolved =
                resolve_translation_record(&translations, &locale, fallback_locale.as_deref());
            let translation = resolved.translation.ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog post {} locale resolver returned no translation from a non-empty set",
                    post.id
                ))
            })?;
            let tags = tags_map.get(&post.id).cloned().unwrap_or_default();

            items.push(PostSummary {
                id: post.id,
                title: translation.title.clone(),
                slug: post.slug.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&translations, |item| {
                    item.locale.as_str()
                }),
                excerpt: translation.excerpt.clone(),
                status: storage_to_status(&post.status)?,
                author_id: post.author_id,
                author_name: None,
                category_id: post.category_id,
                category_name: post
                    .category_id
                    .and_then(|category_id| category_names_map.get(&category_id).cloned()),
                tags,
                featured_image_url: post.featured_image_url.clone(),
                channel_slugs: channel_slugs_map.get(&post.id).cloned().unwrap_or_default(),
                comment_count: post.comment_count as i64,
                published_at: post.published_at.map(Into::into),
                created_at: post.created_at.into(),
            });
        }

        Ok(items)
    }

    /// Public keyset page of published posts, ordered `(published_at DESC, id DESC)`.
    /// Fetches one extra row to learn whether another page exists, so the list
    /// never counts rows.
    #[instrument(skip(self))]
    pub async fn list_public_visible_keyset(
        &self,
        tenant_id: Uuid,
        query: PublicPostsPageQuery,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> BlogResult<PublicPostPage> {
        let locale = query
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let per_page = u64::from(query.per_page());

        if !self.channel_is_public_visible(tenant_id, channel_slug).await? {
            return Ok(PublicPostPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let mut select = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)))
            .filter(blog_post::Column::PublishedAt.is_not_null());

        if let Some(ref tag) = query.tag {
            let Some(tag_id) = resolve_tag_id_for_posts(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?
            else {
                return Ok(PublicPostPage {
                    items: Vec::new(),
                    next_cursor: None,
                });
            };
            select = apply_tag_filter(select, tenant_id, tag_id);
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(blog_post::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(blog_post::Column::CategoryId.eq(category_id));
        }
        if let Some(after) = query.after {
            select = select.filter(
                Condition::any()
                    .add(blog_post::Column::PublishedAt.lt(after.published_at))
                    .add(
                        Condition::all()
                            .add(blog_post::Column::PublishedAt.eq(after.published_at))
                            .add(blog_post::Column::Id.lt(after.id)),
                    ),
            );
        }

        select = apply_public_post_channel_filter(select, tenant_id, channel_slug);
        let mut posts = select
            .order_by_desc(blog_post::Column::PublishedAt)
            .order_by_desc(blog_post::Column::Id)
            .limit(per_page + 1)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let has_next_page = posts.len() as u64 > per_page;
        posts.truncate(per_page as usize);
        let next_cursor = if has_next_page {
            let last = posts.last().ok_or_else(|| {
                BlogError::invariant("Keyset page reported a next page without rows")
            })?;
            let published_at = last.published_at.ok_or_else(|| {
                BlogError::invariant(format!("Published blog post {} has no published_at", last.id))
            })?;
            Some(PublishedPostCursor {
                published_at,
                id: last.id,
            })
        } else {
            None
        };

        let items = self
            .summarize_posts(tenant_id, posts, locale, fallback_locale)
            .await?;
        Ok(PublicPostPage { items, next_cursor })
    }

    #[instrument(skip(self))]
    pub async fn list_public_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        query: PostListQuery,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> BlogResult<PostListResponse> {
        let locale = query
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        if !self.channel_is_public_visible(tenant_id, channel_slug).await? {
            return Ok(PostListResponse::new(Vec::new(), 0, &query));
        }

        let tag_filter = query.tag.clone();
        let mut select = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)));

        if let Some(ref tag) = tag_filter {
            let Some(tag_id) = resolve_tag_id_for_posts(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?
            else {
                return Ok(PostListResponse::new(Vec::new(), 0, &query));
            };
            select = apply_tag_filter(select, tenant_id, tag_id);
        }

        if let Some(author_id) = query.author_id {
            select = select.filter(blog_post::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(blog_post::Column::CategoryId.eq(category_id));
        }

        select = apply_public_post_channel_filter(select, tenant_id, channel_slug);
        select = apply_post_sort(select, &query);

        let paginator = select.paginate(&self.db, query.per_page() as u64);
        let total = paginator.num_items().await.map_err(BlogError::from)?;
        let posts = paginator
            .fetch_page((query.page().saturating_sub(1)) as u64)
            .await
            .map_err(BlogError::from)?;
        let items = self
            .summarize_posts(
                tenant_id,
                posts,
                locale.clone(),
                fallback_locale.clone(),
            )
            .await?;
        Ok(PostListResponse::new(items, total, &query))
    }

    pub async fn get_posts_by_tag(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        tag: String,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            tag: Some(tag),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub async fn get_posts_by_category(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            category_id: Some(category_id),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub async fn get_posts_by_author(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        author_id: Uuid,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            author_id: Some(author_id),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub(super) async fn load_category_names_map(
        &self,
        tenant_id: Uuid,
        category_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<HashMap<Uuid, String>> {
        crate::services::category_name_projection::load_category_names_map(
            &self.db,
            tenant_id,
            category_ids,
            locale,
            fallback_locale,
        )
        .await
    }

    pub(super) async fn build_post_response(
        &self,
        post: blog_post::Model,
        translations: Vec<blog_post_translation::Model>,
        channel_slugs: Vec<String>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostResponse> {
        let tags_map = load_post_tags_map(
            &self.db,
            post.tenant_id,
            &[post.id],
            locale,
            fallback_locale,
        )
        .await?;
        let category_name = if let Some(category_id) = post.category_id {
            self.load_category_names_map(post.tenant_id, &[category_id], locale, fallback_locale)
                .await?
                .get(&category_id)
                .cloned()
        } else {
            None
        };
        if translations.is_empty() {
            return Err(BlogError::invariant(format!(
                "Blog post {} has no localized records",
                post.id
            )));
        }
        let resolved = resolve_translation_record(&translations, locale, fallback_locale);
        let translation = resolved.translation.ok_or_else(|| {
            BlogError::invariant(format!(
                "Blog post {} locale resolver returned no translation from a non-empty set",
                post.id
            ))
        })?;
        let (content, content_plain_text) = project_stored_article(&translation.body)?;

        Ok(PostResponse {
            id: post.id,
            tenant_id: post.tenant_id,
            author_id: post.author_id,
            title: translation.title.clone(),
            slug: post.slug,
            requested_locale: locale.to_string(),
            locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            available_locales: available_locales_from(&translations, |item| item.locale.as_str()),
            content,
            content_plain_text,
            excerpt: translation.excerpt.clone(),
            status: storage_to_status(&post.status)?,
            category_id: post.category_id,
            category_name,
            tags: tags_map.get(&post.id).cloned().unwrap_or_default(),
            featured_image_url: post.featured_image_url,
            seo_title: translation.seo_title.clone(),
            seo_description: translation.seo_description.clone(),
            channel_slugs,
            metadata: scrub_reserved_metadata(post.metadata),
            comment_count: post.comment_count as i64,
            created_at: post.created_at.into(),
            updated_at: post.updated_at.into(),
            published_at: post.published_at.map(Into::into),
            version: post.version,
        })
    }
}
