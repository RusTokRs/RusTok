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
            scheduled_at,
            is_pinned,
        } = input;

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

        // Validate scheduled_at: must be in the future and cannot be set with publish=true
        let scheduled_at_db = if let Some(scheduled) = scheduled_at {
            if publish {
                return Err(BlogError::validation(
                    "Cannot set scheduled_at when publish is true",
                ));
            }
            let scheduled_utc = scheduled.with_timezone(&chrono::Utc);
            if scheduled_utc <= now {
                return Err(BlogError::validation("scheduled_at must be in the future"));
            }
            Some(scheduled.fixed_offset())
        } else {
            None
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
            is_pinned: Set(is_pinned.unwrap_or(false)),
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

    #[instrument(skip(self, security, input))]
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        input: UpdatePostInput,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogPosts, Action::Update)?;
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;

        let UpdatePostInput {
            locale,
            title,
            content,
            excerpt,
            slug,
            tags,
            category_id,
            featured_image_url,
            seo_title,
            seo_description,
            channel_slugs,
            metadata,
            version,
            scheduled_at,
            is_pinned,
        } = input;

        if post.version != version {
            return Err(BlogError::conflict(format!(
                "Blog post version mismatch: expected {version}, current {}",
                post.version
            )));
        }

        validate_optional_title(title.as_deref())?;
        if let Patch::Set(value) = excerpt.as_ref() {
            validate_post_field_length(Some(value.as_str()), MAX_POST_EXCERPT_CHARS, "Excerpt")?;
        }
        if let Patch::Set(value) = seo_title.as_ref() {
            validate_post_field_length(
                Some(value.as_str()),
                MAX_POST_SEO_TITLE_CHARS,
                "SEO title",
            )?;
        }
        if let Patch::Set(value) = seo_description.as_ref() {
            validate_post_field_length(
                Some(value.as_str()),
                MAX_POST_SEO_DESCRIPTION_CHARS,
                "SEO description",
            )?;
        }
        if let Some(ref tags) = tags {
            validate_tags(tags)?;
        }
        if let Patch::Set(url) = featured_image_url.as_ref() {
            validate_featured_image_url(url)?;
        }

        let has_translation_change = title.is_some()
            || content.is_some()
            || excerpt.is_changed()
            || seo_title.is_changed()
            || seo_description.is_changed();
        let needs_locale = has_translation_change || tags.is_some();
        let locale = match locale {
            Some(locale) => Some(normalize_locale(&locale)?),
            None if needs_locale => {
                return Err(BlogError::validation(
                    "Locale is required when changing localized Blog post copy or tags",
                ));
            }
            None => None,
        };

        let article_body = content
            .map(normalize_article)
            .transpose()?
            .as_ref()
            .map(canonical_article_body)
            .transpose()?;

        let normalized_slug = slug
            .as_deref()
            .map(normalize_slug)
            .filter(|slug| !slug.is_empty());
        if slug.is_some() && normalized_slug.is_none() {
            return Err(BlogError::validation("Slug cannot be empty"));
        }
        if let Some(normalized_slug) = normalized_slug.as_deref()
            && normalized_slug.len() > MAX_POST_SLUG_BYTES
        {
            return Err(BlogError::validation(format!(
                "Slug cannot exceed {MAX_POST_SLUG_BYTES} bytes"
            )));
        }

        let next_metadata = match metadata {
            Some(metadata) => normalize_custom_metadata(Some(metadata))?,
            None => scrub_reserved_metadata(post.metadata.clone()),
        };
        let metadata_changed = metadata_changed(&post.metadata, &next_metadata);
        let normalized_channels = channel_slugs
            .as_ref()
            .map(|items| normalize_channel_slugs(items))
            .transpose()?;

        let has_owner_change = normalized_slug.is_some()
            || category_id.is_changed()
            || featured_image_url.is_changed()
            || normalized_channels.is_some()
            || metadata_changed
            || scheduled_at.is_changed()
            || is_pinned.is_some();
        let has_relation_change = tags.is_some();
        if !has_owner_change && !has_translation_change && !has_relation_change {
            return Err(BlogError::validation("No Blog post changes were supplied"));
        }

        let full_reindex = has_owner_change || has_relation_change;
        let next_version = Self::next_persisted_version(post.version)?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let now = chrono::Utc::now();

        if let Some(ref slug) = normalized_slug {
            self.ensure_slug_unique_in_tx(&txn, tenant_id, slug, Some(post_id))
                .await?;
        }
        if let Patch::Set(category_id) = category_id.as_ref() {
            CategoryService::ensure_exists_in_tx(&txn, tenant_id, *category_id).await?;
        }

        let mut update = blog_post::Entity::update_many()
            .col_expr(
                blog_post::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now.fixed_offset()),
            )
            .col_expr(
                blog_post::Column::Version,
                sea_orm::sea_query::Expr::value(next_version),
            )
            .filter(blog_post::Column::Id.eq(post_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Version.eq(version));

        if let Some(ref slug) = normalized_slug {
            update = update.col_expr(
                blog_post::Column::Slug,
                sea_orm::sea_query::Expr::value(slug.clone()),
            );
        }
        match category_id {
            Patch::Keep => {}
            Patch::Set(category_id) => {
                update = update.col_expr(
                    blog_post::Column::CategoryId,
                    sea_orm::sea_query::Expr::value(Some(category_id)),
                );
            }
            Patch::Clear => {
                update = update.col_expr(
                    blog_post::Column::CategoryId,
                    sea_orm::sea_query::Expr::value(Option::<Uuid>::None),
                );
            }
        }
        match featured_image_url {
            Patch::Keep => {}
            Patch::Set(url) => {
                update = update.col_expr(
                    blog_post::Column::FeaturedImageUrl,
                    sea_orm::sea_query::Expr::value(Some(url)),
                );
            }
            Patch::Clear => {
                update = update.col_expr(
                    blog_post::Column::FeaturedImageUrl,
                    sea_orm::sea_query::Expr::value(Option::<String>::None),
                );
            }
        }
        if metadata_changed {
            update = update.col_expr(
                blog_post::Column::Metadata,
                sea_orm::sea_query::Expr::value(next_metadata),
            );
        }
        match scheduled_at {
            Patch::Keep => {}
            Patch::Set(scheduled) => {
                let scheduled_utc = scheduled.with_timezone(&chrono::Utc);
                if scheduled_utc <= now {
                    return Err(BlogError::validation("scheduled_at must be in the future"));
                }
                update = update.col_expr(
                    blog_post::Column::ScheduledAt,
                    sea_orm::sea_query::Expr::value(Some(scheduled.fixed_offset())),
                );
            }
            Patch::Clear => {
                update = update.col_expr(
                    blog_post::Column::ScheduledAt,
                    sea_orm::sea_query::Expr::value(
                        Option::<chrono::DateTime<chrono::FixedOffset>>::None,
                    ),
                );
            }
        }
        if let Some(pinned) = is_pinned {
            let pinned_at = if pinned { Some(now) } else { None };
            update = update
                .col_expr(
                    blog_post::Column::IsPinned,
                    sea_orm::sea_query::Expr::value(pinned),
                )
                .col_expr(
                    blog_post::Column::PinnedAt,
                    sea_orm::sea_query::Expr::value(pinned_at.map(|t| t.fixed_offset())),
                );
        }

        let normalized_slug_for_error = normalized_slug.clone();
        let result = update.exec(&txn).await.map_err(|error| {
            if normalized_slug_for_error.is_some() && PostService::is_unique_constraint(&error) {
                BlogError::duplicate_slug(normalized_slug_for_error.clone().unwrap_or_default())
            } else {
                BlogError::from(error)
            }
        })?;
        if result.rows_affected != 1 {
            return Err(BlogError::conflict(
                "Blog post changed concurrently before the update could be applied",
            ));
        }

        if let Some(ref slug) = normalized_slug
            && *slug != post.slug
        {
            // The previous route becomes a permanent redirect to the new
            // canonical route; `CanonicalUrlChanged` drives SEO redirects.
            self.claim_post_route_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                post_id,
                slug,
                Some(post.slug.as_str()),
            )
            .await?;
        }

        if has_translation_change {
            let locale = locale.as_deref().ok_or_else(|| {
                BlogError::invariant(
                    "localized change reached persistence without a canonical locale",
                )
            })?;
            self.upsert_translation_in_tx(
                &txn,
                tenant_id,
                post_id,
                locale,
                PostTranslationUpsertInput {
                    title,
                    excerpt,
                    seo_title,
                    seo_description,
                    article_body,
                    now,
                },
            )
            .await?;
        }

        if let Some(channel_slugs) = normalized_channels {
            self.replace_channel_visibility_in_tx(&txn, tenant_id, post_id, &channel_slugs)
                .await?;
        }
        if let Some(tags) = tags {
            let locale = locale.as_deref().ok_or_else(|| {
                BlogError::invariant("tag mutation reached persistence without a canonical locale")
            })?;
            sync_post_tags_in_tx(
                &self.db,
                &txn,
                tenant_id,
                post_id,
                &tags,
                locale,
                !matches!(
                    security.get_scope(Resource::Tags, Action::Create),
                    rustok_core::PermissionScope::None
                ),
            )
            .await?;
        }

        let event = if full_reindex {
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: Some(post_id),
            }
        } else {
            DomainEvent::BlogPostUpdated {
                post_id,
                locale: locale.ok_or_else(|| {
                    BlogError::invariant(
                        "localized-only update reached event publication without a canonical locale",
                    )
                })?,
            }
        };
        self.event_bus
            .publish_in_tx(&txn, tenant_id, security.user_id, event)
            .await
            .map_err(BlogError::from)?;

        // Publish schedule events if scheduled_at was changed
        match scheduled_at {
            Patch::Set(scheduled) => {
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
            Patch::Clear => {
                self.event_bus
                    .publish_in_tx(
                        &txn,
                        tenant_id,
                        security.user_id,
                        DomainEvent::BlogPostScheduleCancelled { post_id },
                    )
                    .await
                    .map_err(BlogError::from)?;
            }
            Patch::Keep => {}
        }

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
        enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let current = storage_to_status(&post.status)?;
        ensure_transition(current, BlogPostStatus::Published)?;

        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        apply_status_transition_in_tx(
            &txn,
            tenant_id,
            post_id,
            post.version,
            BlogPostStatus::Published,
            // published_at records the first publication and is never reset.
            Some(post.published_at.map(Into::into).unwrap_or(now)),
            None,
        )
        .await?;

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
        enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let current = storage_to_status(&post.status)?;
        ensure_transition(current, BlogPostStatus::Draft)?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        apply_status_transition_in_tx(
            &txn,
            tenant_id,
            post_id,
            post.version,
            BlogPostStatus::Draft,
            post.published_at.map(Into::into),
            None,
        )
        .await?;

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
        if let Some(reason) = reason.as_deref()
            && reason.chars().count() > MAX_POST_ARCHIVE_REASON_CHARS
        {
            return Err(BlogError::validation(format!(
                "Archive reason cannot exceed {MAX_POST_ARCHIVE_REASON_CHARS} characters"
            )));
        }

        enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let current = storage_to_status(&post.status)?;
        ensure_transition(current, BlogPostStatus::Archived)?;

        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        apply_status_transition_in_tx(
            &txn,
            tenant_id,
            post_id,
            post.version,
            BlogPostStatus::Archived,
            post.published_at.map(Into::into),
            Some(now),
        )
        .await?;

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
    pub async fn restore_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let current = storage_to_status(&post.status)?;
        ensure_transition(current, BlogPostStatus::Draft)?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        apply_status_transition_in_tx(
            &txn,
            tenant_id,
            post_id,
            post.version,
            BlogPostStatus::Draft,
            post.published_at.map(Into::into),
            None,
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ReindexRequested {
                    target_type: "blog".to_string(),
                    target_id: Some(post_id),
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
        enforce_scope(&security, Resource::BlogPosts, Action::Delete)?;
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
        remove_post_tag_usage_in_tx(&txn, tenant_id, post_id).await?;
        let deleted = blog_post::Entity::delete_many()
            .filter(blog_post::Column::Id.eq(post_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Version.eq(post.version))
            .exec(&txn)
            .await
            .map_err(BlogError::from)?;
        if deleted.rows_affected != 1 {
            return Err(BlogError::conflict(
                "Blog post changed concurrently before deletion",
            ));
        }
        BlogPostRouteOwner::new(self.event_bus.clone())
            .remove_post_routes_in_tx(&txn, tenant_id, security.user_id, post_id, &post.slug)
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostDeleted { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TargetDeleted {
                    target_type: "blog_post".to_string(),
                    target_id: post_id,
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    /// Publish all scheduled posts whose scheduled_at time has passed.
    /// This method is called by the scheduler worker.
    pub async fn publish_scheduled_posts(&self) -> BlogResult<Vec<(Uuid, Uuid)>> {
        let now = chrono::Utc::now();

        // Find all draft posts with scheduled_at in the past
        let posts = blog_post::Entity::find()
            .filter(blog_post::Column::Status.eq("draft"))
            .filter(blog_post::Column::ScheduledAt.is_not_null())
            .filter(blog_post::Column::ScheduledAt.lte(now))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let mut published = Vec::new();

        for post in posts {
            let txn = self.db.begin().await.map_err(BlogError::from)?;

            // Use the same transition logic as manual publish
            apply_status_transition_in_tx(
                &txn,
                post.tenant_id,
                post.id,
                post.version,
                BlogPostStatus::Published,
                Some(now),
                None,
            )
            .await?;

            // Publish the BlogPostPublished event
            self.event_bus
                .publish_in_tx(
                    &txn,
                    post.tenant_id,
                    None, // System actor
                    DomainEvent::BlogPostPublished {
                        post_id: post.id,
                        author_id: Some(post.author_id),
                    },
                )
                .await
                .map_err(BlogError::from)?;

            txn.commit().await.map_err(BlogError::from)?;
            published.push((post.tenant_id, post.id));
        }

        Ok(published)
    }
}

fn ensure_transition(current: BlogPostStatus, next: BlogPostStatus) -> BlogResult<()> {
    if current.can_transition_to(next) {
        Ok(())
    } else {
        Err(BlogError::invalid_transition(current, next))
    }
}

/// Applies a lifecycle transition. `published_at` is the first-publication
/// timestamp to persist; callers carry it over from the stored post unless the
/// post is being published for the first time.
async fn apply_status_transition_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
    expected_version: i32,
    next: BlogPostStatus,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    archived_at: Option<chrono::DateTime<chrono::Utc>>,
) -> BlogResult<()> {
    let now = chrono::Utc::now();
    let mut update = blog_post::Entity::update_many()
        .col_expr(
            blog_post::Column::Status,
            sea_orm::sea_query::Expr::value(status_to_storage(next).to_string()),
        )
        .col_expr(
            blog_post::Column::PublishedAt,
            sea_orm::sea_query::Expr::value(published_at.map(|value| value.fixed_offset())),
        )
        .col_expr(
            blog_post::Column::ArchivedAt,
            sea_orm::sea_query::Expr::value(archived_at.map(|value| value.fixed_offset())),
        )
        .col_expr(
            blog_post::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now.fixed_offset()),
        )
        .col_expr(
            blog_post::Column::Version,
            sea_orm::sea_query::Expr::value(PostService::next_persisted_version(expected_version)?),
        );

    // Clear scheduled_at when publishing (whether manually or by scheduler)
    if next == BlogPostStatus::Published {
        update = update.col_expr(
            blog_post::Column::ScheduledAt,
            sea_orm::sea_query::Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
        );
    }

    let result = update
        .filter(blog_post::Column::Id.eq(post_id))
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::Version.eq(expected_version))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;

    if result.rows_affected != 1 {
        return Err(BlogError::conflict(
            "Blog post changed concurrently before lifecycle transition",
        ));
    }

    Ok(())
}
