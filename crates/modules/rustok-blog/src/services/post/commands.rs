use super::*;

impl PostService {
    #[instrument(skip(self, security, input))]
    pub async fn create_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePostInput,
    ) -> BlogResult<Uuid> {
        let CreatePostInput {
            locale,
            title,
            content,
            excerpt,
            slug,
            publish,
            tags,
            category_id,
            featured_image_url,
            seo_title,
            seo_description,
            channel_slugs,
            metadata,
        } = input;

        validate_title(&title)?;
        validate_locale(&locale)?;
        validate_tags(&tags)?;

        let author_id = enforce_create_author(&security, Resource::BlogPosts, Action::Create)?;
        let content = normalize_article(content)?;
        let article_body = canonical_article_body(&content)?;

        let slug = normalize_slug(slug.as_deref().unwrap_or(&title));
        if slug.is_empty() {
            return Err(BlogError::validation("Slug cannot be empty"));
        }

        let now = chrono::Utc::now();
        let metadata = build_post_metadata(
            metadata,
            Some(tags.clone()),
            category_id,
            featured_image_url.clone(),
            seo_title.clone(),
            seo_description.clone(),
        );
        let channel_slugs = normalize_channel_slugs(channel_slugs.as_deref().unwrap_or(&[]));

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        self.ensure_slug_unique_in_tx(&txn, tenant_id, &slug, None)
            .await?;
        if let Some(category_id) = category_id {
            CategoryService::ensure_exists_in_tx(&txn, tenant_id, category_id).await?;
        }

        let post_id = Uuid::new_v4();
        let status = if publish {
            BlogPostStatus::Published
        } else {
            BlogPostStatus::Draft
        };

        blog_post::ActiveModel {
            id: Set(post_id),
            tenant_id: Set(tenant_id),
            author_id: Set(author_id),
            category_id: Set(category_id),
            status: Set(status_to_storage(status).to_string()),
            slug: Set(slug),
            metadata: Set(metadata),
            featured_image_url: Set(featured_image_url),
            published_at: Set(if publish { Some(now.into()) } else { None }),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            archived_at: Set(None),
            comment_count: Set(0),
            view_count: Set(0),
            version: Set(1),
        }
        .insert(&txn)
        .await
        .map_err(BlogError::from)?;

        blog_post_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            post_id: Set(post_id),
            locale: Set(normalize_locale(&locale)?),
            title: Set(title),
            excerpt: Set(excerpt),
            seo_title: Set(seo_title),
            seo_description: Set(seo_description),
            body: Set(article_body),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await
        .map_err(BlogError::from)?;

        self.replace_channel_visibility_in_tx(&txn, tenant_id, post_id, &channel_slugs)
            .await?;
        sync_post_tags_in_tx(&self.db, &txn, tenant_id, post_id, &tags, &locale).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostCreated {
                    post_id,
                    author_id: Some(author_id),
                    locale,
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(post_id)
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        input: UpdatePostInput,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;
        if let Some(expected_version) = input.version
            && post.version != expected_version
        {
            return Err(BlogError::validation("Version mismatch"));
        }

        let locale = input
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        validate_optional_title(input.title.as_deref())?;
        if let Some(ref tags) = input.tags {
            validate_tags(tags)?;
        }

        let mut metadata = post.metadata.clone();
        if let Some(override_metadata) = input.metadata.clone() {
            merge_metadata(&mut metadata, override_metadata);
        }
        if let Some(ref tags) = input.tags {
            set_metadata_array(&mut metadata, "tags", tags.clone());
        }
        if let Some(category_id) = input.category_id {
            set_metadata_uuid(&mut metadata, "category_id", category_id);
        }
        if let Some(ref url) = input.featured_image_url {
            set_metadata_string(&mut metadata, "featured_image_url", url);
        }
        if let Some(ref seo_title) = input.seo_title {
            set_metadata_string(&mut metadata, "seo_title", seo_title);
        }
        if let Some(ref seo_description) = input.seo_description {
            set_metadata_string(&mut metadata, "seo_description", seo_description);
        }
        strip_channel_visibility_metadata(&mut metadata);
        let channel_slugs = input
            .channel_slugs
            .as_ref()
            .map(|items| normalize_channel_slugs(items))
            .unwrap_or_default();
        let replace_channel_visibility = input.channel_slugs.is_some();

        let article_body = input
            .content
            .clone()
            .map(normalize_article)
            .transpose()?
            .as_ref()
            .map(canonical_article_body)
            .transpose()?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let now = chrono::Utc::now();

        let mut post_active: blog_post::ActiveModel = post.clone().into();
        if let Some(ref slug) = input.slug {
            let normalized = normalize_slug(slug);
            if normalized.is_empty() {
                return Err(BlogError::validation("Slug cannot be empty"));
            }
            self.ensure_slug_unique_in_tx(&txn, tenant_id, &normalized, Some(post_id))
                .await?;
            post_active.slug = Set(normalized);
        }
        if input.category_id.is_some() {
            if let Some(category_id) = input.category_id {
                CategoryService::ensure_exists_in_tx(&txn, tenant_id, category_id).await?;
            }
            post_active.category_id = Set(input.category_id);
        }
        if input.featured_image_url.is_some() {
            post_active.featured_image_url = Set(input.featured_image_url.clone());
        }
        post_active.metadata = Set(metadata);
        post_active.updated_at = Set(now.into());
        post_active.version = Set(post.version + 1);
        post_active.update(&txn).await.map_err(BlogError::from)?;

        self.upsert_translation_in_tx(
            &txn,
            post_id,
            &locale,
            PostTranslationUpsertInput {
                title: input.title,
                excerpt: input.excerpt,
                seo_title: input.seo_title,
                seo_description: input.seo_description,
                article_body,
                now,
            },
        )
        .await?;

        if replace_channel_visibility {
            self.replace_channel_visibility_in_tx(&txn, tenant_id, post_id, &channel_slugs)
                .await?;
        }
        if let Some(ref tags) = input.tags {
            sync_post_tags_in_tx(&self.db, &txn, tenant_id, post_id, tags, &locale).await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostUpdated { post_id, locale },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn publish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.status = Set(status_to_storage(BlogPostStatus::Published).to_string());
        active.published_at = Set(Some(now.into()));
        active.archived_at = Set(None);
        active.updated_at = Set(now.into());
        active.version = Set(post.version + 1);
        active.update(&txn).await.map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostPublished {
                    post_id,
                    author_id: Some(post.author_id),
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn unpublish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.status = Set(status_to_storage(BlogPostStatus::Draft).to_string());
        active.published_at = Set(None);
        active.updated_at = Set(now.into());
        active.version = Set(post.version + 1);
        active.update(&txn).await.map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostUnpublished { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn archive_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        reason: Option<String>,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.status = Set(status_to_storage(BlogPostStatus::Archived).to_string());
        active.archived_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.version = Set(post.version + 1);
        active.update(&txn).await.map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostArchived {
                    post_id,
                    reason: reason.clone(),
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn delete_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Delete,
            post.author_id,
        )?;
        if storage_to_status(&post.status)? == BlogPostStatus::Published {
            return Err(BlogError::CannotDeletePublished);
        }

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        blog_post::Entity::delete_by_id(post_id)
            .exec(&txn)
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostDeleted { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }
}
