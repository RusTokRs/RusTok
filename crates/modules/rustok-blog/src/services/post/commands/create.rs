use super::*;

impl PostService {
    #[instrument(skip(self, security, input))]
    pub async fn create_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePostInput,
    ) -> BlogResult<Uuid> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
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
            scheduled_at,
        } = input;

        if let Some(cat_id) = category_id
            && cat_id.is_nil()
        {
            return Err(BlogError::validation("Category ID must not be nil"));
        }

        validate_title(&title)?;
        validate_post_field_length(excerpt.as_deref(), MAX_POST_EXCERPT_CHARS, "Excerpt")?;
        validate_post_field_length(seo_title.as_deref(), MAX_POST_SEO_TITLE_CHARS, "SEO title")?;
        validate_post_field_length(
            seo_description.as_deref(),
            MAX_POST_SEO_DESCRIPTION_CHARS,
            "SEO description",
        )?;
        validate_optional_featured_image_url(featured_image_url.as_deref())?;
        let locale = normalize_locale(&locale)?;
        validate_tags(&tags)?;

        let author_id = enforce_create_author(&security, Resource::BlogPosts, Action::Create)?;
        let allow_tag_create = !matches!(
            security.get_scope(Resource::Tags, Action::Create),
            rustok_core::PermissionScope::None
        );
        if publish {
            enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;
        }
        let content = normalize_article(content)?;
        let article_body = canonical_article_body(&content)?;
        ensure_article_body_within_bounds(&article_body)?;

        let slug = normalize_slug(slug.as_deref().unwrap_or(&title));
        if slug.is_empty() {
            return Err(BlogError::validation("Slug cannot be empty"));
        }
        if slug.len() > MAX_POST_SLUG_BYTES {
            return Err(BlogError::validation(format!(
                "Slug cannot exceed {MAX_POST_SLUG_BYTES} bytes"
            )));
        }

        let now = chrono::Utc::now();
        let metadata = normalize_custom_metadata(metadata)?;
        let channel_slugs = normalize_channel_slugs(channel_slugs.as_deref().unwrap_or(&[]))?;

        // Validated before the transaction opens: an invalid schedule must not hold a
        // pooled connection while it is rejected.
        let scheduled_at_db = validate_schedule_for_create(scheduled_at, publish, now)?;

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

        let slug_for_error = slug.clone();
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
            scheduled_at: Set(scheduled_at_db),
            is_pinned: Set(false),
            pinned_at: Set(None),
            comment_count: Set(0),
            version: Set(1),
        }
        .insert(&txn)
        .await
        .map_err(|error| {
            if PostService::is_unique_constraint(&error) {
                BlogError::duplicate_slug(slug_for_error.clone())
            } else {
                BlogError::from(error)
            }
        })?;
        self.claim_post_route_in_tx(
            &txn,
            tenant_id,
            security.user_id,
            post_id,
            &slug_for_error,
            None,
        )
        .await?;

        blog_post_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            post_id: Set(post_id),
            locale: Set(locale.clone()),
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
        sync_post_tags_in_tx(
            &self.db,
            &txn,
            tenant_id,
            post_id,
            &tags,
            &locale,
            allow_tag_create,
        )
        .await?;

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

        // A post created in the published state is a publication, not only a
        // creation: subscribers to `blog.post.published` (search, feeds,
        // webhooks, caches) must see the same event a later `publish_post`
        // would have emitted.
        if publish {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::BlogPostPublished {
                        post_id,
                        author_id: Some(author_id),
                    },
                )
                .await
                .map_err(BlogError::from)?;
        }

        if let Some(scheduled) = scheduled_at {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::BlogPostScheduled {
                        post_id,
                        scheduled_at: scheduled.to_rfc3339(),
                    },
                )
                .await
                .map_err(BlogError::from)?;
        }

        txn.commit().await.map_err(BlogError::from)?;
        Ok(post_id)
    }
}
