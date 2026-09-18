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
        let translations = self.load_translations(post_id).await?;
        let channel_slugs = self.load_channel_slugs(post_id).await?;
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
        let Some(post) = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Slug.eq(normalize_slug(slug)))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
        else {
            return Ok(None);
        };

        if storage_to_status(&post.status)? != BlogPostStatus::Published
            && !can_read_non_public_posts(&security)
        {
            return Ok(None);
        }

        let translations = self.load_translations(post.id).await?;
        let channel_slugs = self.load_channel_slugs(post.id).await?;
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
            let tagged_post_ids = find_post_ids_by_tag(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;
            if tagged_post_ids.is_empty() {
                return Ok(PostListResponse::new(Vec::new(), 0, &query));
            }
            select = select.filter(blog_post::Column::Id.is_in(tagged_post_ids));
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

        let translations_map = self.load_translations_map(&post_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(&post_ids).await?;
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
            let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
            let resolved =
                resolve_translation_record(&translations, &locale, fallback_locale.as_deref());
            let translation = resolved.translation;
            let tags = tags_map
                .get(&post.id)
                .cloned()
                .unwrap_or_else(|| extract_tags(&post.metadata));

            items.push(PostSummary {
                id: post.id,
                title: translation
                    .map(|item| item.title.clone())
                    .unwrap_or_default(),
                slug: post.slug.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                excerpt: translation.and_then(|item| item.excerpt.clone()),
                status: storage_to_status(&post.status)?,
                author_id: post.author_id,
                author_name: None,
                category_id: post.category_id,
                category_name: post
                    .category_id
                    .and_then(|category_id| category_names_map.get(&category_id).cloned()),
                tags,
                featured_image_url: post.featured_image_url.clone(),
                channel_slugs: channel_slugs_map
                    .get(&post.id)
                    .cloned()
                    .unwrap_or_else(|| extract_channel_slugs(&post.metadata)),
                comment_count: post.comment_count as i64,
                published_at: post.published_at.map(Into::into),
                created_at: post.created_at.into(),
            });
        }

        Ok(PostListResponse::new(items, total, &query))
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

        let tag_filter = query.tag.clone();
        let mut select = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)));

        if let Some(ref tag) = tag_filter {
            let tagged_post_ids = find_post_ids_by_tag(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;
            if tagged_post_ids.is_empty() {
                return Ok(PostListResponse::new(Vec::new(), 0, &query));
            }
            select = select.filter(blog_post::Column::Id.is_in(tagged_post_ids));
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
        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();

        let translations_map = self.load_translations_map(&post_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(&post_ids).await?;
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
            let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
            let resolved =
                resolve_translation_record(&translations, &locale, fallback_locale.as_deref());
            let translation = resolved.translation;
            let tags = tags_map
                .get(&post.id)
                .cloned()
                .unwrap_or_else(|| extract_tags(&post.metadata));

            items.push(PostSummary {
                id: post.id,
                title: translation
                    .map(|item| item.title.clone())
                    .unwrap_or_default(),
                slug: post.slug.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                excerpt: translation.and_then(|item| item.excerpt.clone()),
                status: storage_to_status(&post.status)?,
                author_id: post.author_id,
                author_name: None,
                category_id: post.category_id,
                category_name: post
                    .category_id
                    .and_then(|category_id| category_names_map.get(&category_id).cloned()),
                tags,
                featured_image_url: post.featured_image_url.clone(),
                channel_slugs: channel_slugs_map
                    .get(&post.id)
                    .cloned()
                    .unwrap_or_else(|| extract_channel_slugs(&post.metadata)),
                comment_count: post.comment_count as i64,
                published_at: post.published_at.map(Into::into),
                created_at: post.created_at.into(),
            });
        }

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
}
