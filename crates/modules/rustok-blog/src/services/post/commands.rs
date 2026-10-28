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
        if let Patch::Set(cat_id) = category_id
            && cat_id.is_nil()
        {
            return Err(BlogError::validation("Category ID must not be nil"));
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
        if let Some(article_body) = article_body.as_deref() {
            ensure_article_body_within_bounds(article_body)?;
        }

        let requested_slug = slug.as_deref().map(normalize_slug);
        if let Some(requested_slug) = requested_slug.as_deref()
            && requested_slug.is_empty()
        {
            return Err(BlogError::validation("Slug cannot be empty"));
        }
        // A slug that normalizes to the stored one is not a change: treating
        // it as one would turn a copy-only edit into a full reindex event and
        // a route claim for a route the post already owns.
        let normalized_slug = requested_slug.filter(|slug| !slug.is_empty() && slug != &post.slug);
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
            || scheduled_at.is_changed();
        let has_relation_change = tags.is_some();
        if !has_owner_change && !has_translation_change && !has_relation_change {
            return Err(BlogError::validation("No Blog post changes were supplied"));
        }

        let full_reindex = has_owner_change || has_relation_change;
        let next_version = Self::next_persisted_version(post.version)?;
        // Validated before the transaction opens: an invalid schedule must not hold a
        // pooled connection while it is rejected.
        let now = chrono::Utc::now();
        let next_schedule =
            validate_schedule_for_update(&scheduled_at, storage_to_status(&post.status)?, now)?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;

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
        match next_schedule {
            Patch::Keep => {}
            Patch::Set(scheduled) => {
                update = update.col_expr(
                    blog_post::Column::ScheduledAt,
                    sea_orm::sea_query::Expr::value(Some(scheduled)),
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
        if !lifecycle_command_should_apply(LifecycleCommand::Publish, current)? {
            return Ok(());
        }

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
        if !lifecycle_command_should_apply(LifecycleCommand::Unpublish, current)? {
            return Ok(());
        }

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
        if !lifecycle_command_should_apply(LifecycleCommand::Archive, current)? {
            return Ok(());
        }

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
        if !lifecycle_command_should_apply(LifecycleCommand::Restore, current)? {
            return Ok(());
        }

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
        remove_post_tag_links_in_tx(&txn, tenant_id, post_id).await?;
        // The comment projection ledger of a deleted post is terminal state:
        // no post can ever be projected for it again (a later Comments event
        // for the post re-cleans and is dropped by the projection handler),
        // so leaving the rows behind would retain per-comment state forever.
        blog_comment_projection_delivery::Entity::delete_many()
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(tenant_id))
            .filter(blog_comment_projection_delivery::Column::PostId.eq(post_id))
            .exec(&txn)
            .await
            .map_err(BlogError::from)?;
        // Preview tokens are the deleted post's authorization; they must not survive
        // it in the same transaction (the FK cascade is the second, database-level
        // guarantee).
        crate::services::preview_token::delete_post_preview_tokens_in_tx(&txn, tenant_id, post_id)
            .await?;
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
                    target_type: BLOG_POST_TARGET_TYPE.to_string(),
                    target_id: post_id,
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }
}

/// The four lifecycle commands of a Blog post.
///
/// Commands, not target statuses, are the unit of lifecycle semantics: `Draft`
/// is the target of two different commands (`Unpublish` from `Published` and
/// `Restore` from `Archived`), so a validation that only inspects the target
/// cannot tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleCommand {
    Publish,
    Unpublish,
    Archive,
    Restore,
}

impl LifecycleCommand {
    /// The one source status this command may start from and its target.
    const fn edge(self) -> (BlogPostStatus, BlogPostStatus) {
        match self {
            Self::Publish => (BlogPostStatus::Draft, BlogPostStatus::Published),
            Self::Unpublish => (BlogPostStatus::Published, BlogPostStatus::Draft),
            Self::Archive => (BlogPostStatus::Published, BlogPostStatus::Archived),
            Self::Restore => (BlogPostStatus::Archived, BlogPostStatus::Draft),
        }
    }
}

/// Decides whether a lifecycle command must run against the **stored** status.
///
/// Returns `Ok(false)` when the post is already in the command's target state:
/// that is a successful no-op, so a retried request after a lost response never
/// fails and never emits a second lifecycle event (idempotent command
/// semantics, `AGENTS.md` §7).
///
/// A post in any status other than the command's declared source is rejected
/// with an error that names the command instead of the raw transition: the
/// state graph allows `Archived -> Draft`, which is exactly what `Restore`
/// means, but it is not what `Unpublish` means.
fn lifecycle_command_should_apply(
    command: LifecycleCommand,
    current: BlogPostStatus,
) -> BlogResult<bool> {
    let (source, target) = command.edge();
    if current == target {
        return Ok(false);
    }
    if current != source {
        return Err(match (command, current) {
            (LifecycleCommand::Publish, BlogPostStatus::Archived) => {
                BlogError::CannotPublishArchived
            }
            (LifecycleCommand::Unpublish, BlogPostStatus::Archived) => {
                BlogError::CannotUnpublishArchived
            }
            (LifecycleCommand::Restore, BlogPostStatus::Published) => {
                BlogError::CannotRestorePublished
            }
            _ => BlogError::invalid_transition(current, target),
        });
    }
    ensure_transition(current, target)?;
    Ok(true)
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
pub(super) async fn apply_status_transition_in_tx(
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

    // A preview token authorizes a read of a *draft* only: `get_draft_preview` refuses to
    // serve any other status. A token row that outlived the draft state would still be a
    // live authorization the moment the post returned to draft (`unpublish`, `restore`),
    // so leaving the draft state revokes the post's tokens in the same transaction. The
    // delete is tenant- and post-scoped, and it is a no-op for a post that had no tokens.
    if next != BlogPostStatus::Draft {
        crate::services::preview_token::delete_post_preview_tokens_in_tx(txn, tenant_id, post_id)
            .await?;
    }

    Ok(())
}
