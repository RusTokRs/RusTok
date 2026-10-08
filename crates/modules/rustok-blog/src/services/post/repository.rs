use super::*;

impl PostService {
    pub(super) fn next_persisted_version(version: i32) -> BlogResult<i32> {
        version
            .checked_add(1)
            .filter(|next| *next > 0)
            .ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog post version {} is invalid or exhausted",
                    version
                ))
            })
    }

    pub(super) fn validate_persisted_version(post: &blog_post::Model) -> BlogResult<()> {
        if post.version > 0 {
            return Ok(());
        }
        const ERR_INVALID_VERSION: &str = "invalid persisted version";
        Err(BlogError::invariant(format!(
            "Blog post {} has {ERR_INVALID_VERSION} {}",
            post.id, post.version
        )))
    }

    pub(super) async fn find_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
    ) -> BlogResult<blog_post::Model> {
        blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
            .ok_or(BlogError::PostNotFound(post_id))
            .and_then(|post| {
                Self::validate_persisted_version(&post)?;
                Ok(post)
            })
    }

    pub(super) async fn load_translations(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
    ) -> BlogResult<Vec<blog_post_translation::Model>> {
        blog_post_translation::Entity::find()
            .inner_join(blog_post::Entity)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .all(&self.db)
            .await
            .map_err(BlogError::from)
    }

    pub(super) async fn load_translations_map(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
    ) -> BlogResult<HashMap<Uuid, Vec<blog_post_translation::Model>>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let translations = blog_post_translation::Entity::find()
            .inner_join(blog_post::Entity)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post_translation::Column::PostId.is_in(post_ids.to_vec()))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let mut map: HashMap<Uuid, Vec<blog_post_translation::Model>> = HashMap::new();
        for translation in translations {
            map.entry(translation.post_id)
                .or_default()
                .push(translation);
        }
        Ok(map)
    }

    pub(super) async fn load_channel_slugs(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
    ) -> BlogResult<Vec<String>> {
        let records = blog_post_channel_visibility::Entity::find()
            .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(blog_post_channel_visibility::Column::PostId.eq(post_id))
            .order_by_asc(blog_post_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        Ok(records.into_iter().map(|item| item.channel_slug).collect())
    }

    pub(super) async fn load_channel_slugs_map(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
    ) -> BlogResult<HashMap<Uuid, Vec<String>>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let records = blog_post_channel_visibility::Entity::find()
            .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(blog_post_channel_visibility::Column::PostId.is_in(post_ids.to_vec()))
            .order_by_asc(blog_post_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let mut map: HashMap<Uuid, Vec<String>> = HashMap::new();
        for record in records {
            map.entry(record.post_id)
                .or_default()
                .push(record.channel_slug);
        }
        Ok(map)
    }

    pub(super) async fn replace_channel_visibility_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        post_id: Uuid,
        channel_slugs: &[String],
    ) -> BlogResult<()> {
        ChannelService::new(self.db.clone())
            .ensure_channel_slugs_exist_for_tenant_in_tx(txn, tenant_id, channel_slugs)
            .await
            .map_err(BlogError::from)?;

        blog_post_channel_visibility::Entity::delete_many()
            .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(blog_post_channel_visibility::Column::PostId.eq(post_id))
            .exec(txn)
            .await
            .map_err(BlogError::from)?;

        for channel_slug in channel_slugs {
            blog_post_channel_visibility::ActiveModel {
                id: Set(Uuid::new_v4()),
                post_id: Set(post_id),
                tenant_id: Set(tenant_id),
                channel_slug: Set(channel_slug.clone()),
                created_at: Set(chrono::Utc::now().into()),
            }
            .insert(txn)
            .await
            .map_err(BlogError::from)?;
        }

        Ok(())
    }

    pub(super) fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
        matches!(
            error.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        )
    }

    pub(super) async fn ensure_slug_unique_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        slug: &str,
        exclude_post_id: Option<Uuid>,
    ) -> BlogResult<()> {
        let mut query = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Slug.eq(slug));
        if let Some(exclude_post_id) = exclude_post_id {
            query = query.filter(blog_post::Column::Id.ne(exclude_post_id));
        }

        if query.one(txn).await.map_err(BlogError::from)?.is_some() {
            return Err(BlogError::duplicate_slug(slug.to_string()));
        }

        Ok(())
    }

    /// Makes `route` of the post's slug canonical for `post_id` through the
    /// shared canonical-route writer. A retired route that still points at
    /// another post is released first, so a redirect can never shadow a live
    /// canonical slug. `previous_slug` is the slug being replaced on rename; its
    /// route becomes a permanent redirect to the new canonical route.
    pub(super) async fn claim_post_route_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        post_id: Uuid,
        slug: &str,
        previous_slug: Option<&str>,
    ) -> BlogResult<()> {
        let canonical_route = canonical_post_route(slug);
        let writer = CanonicalUrlWriter::new(self.event_bus.clone());
        writer
            .release_alias_route_in_tx(txn, tenant_id, actor_id, &canonical_route)
            .await?;

        let mutation = CanonicalUrlMutation {
            target_kind: BLOG_POST_TARGET_KIND.to_string(),
            target_id: post_id,
            locale: CANONICAL_POST_ROUTE_LOCALE.to_string(),
            canonical_url: canonical_route,
            alias_urls: previous_slug.map(canonical_post_route).into_iter().collect(),
            retired_targets: Vec::new(),
        };
        writer
            .apply_canonical_url_mutations(
                txn,
                tenant_id,
                actor_id,
                std::slice::from_ref(&mutation),
            )
            .await?;
        Ok(())
    }

    /// Resolves a post by its current slug first, then through the canonical
    /// route registry, which holds routes retired by a slug change. Callers
    /// must redirect when the returned post's slug differs from the requested
    /// one. A route now owned by another module resolves to `None`: Blog does
    /// not serve it.
    pub(super) async fn find_post_by_current_or_canonical_route(
        &self,
        tenant_id: Uuid,
        locale: &str,
        slug: &str,
    ) -> BlogResult<Option<blog_post::Model>> {
        if let Some(post) = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Slug.eq(slug))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
        {
            Self::validate_persisted_version(&post)?;
            return Ok(Some(post));
        }

        let Some(resolved) = CanonicalUrlService::new(self.db.clone())
            .resolve_route(tenant_id, locale, &canonical_post_route(slug))
            .await?
        else {
            return Ok(None);
        };
        if resolved.target_kind != BLOG_POST_TARGET_KIND {
            return Ok(None);
        }

        let post = blog_post::Entity::find_by_id(resolved.target_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
            .ok_or_else(|| {
                BlogError::invariant(format!(
                    "Canonical blog route '{slug}' references missing post {}",
                    resolved.target_id
                ))
            })?;
        Self::validate_persisted_version(&post)?;
        Ok(Some(post))
    }

    pub(super) async fn upsert_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        post_id: Uuid,
        locale: &str,
        input: PostTranslationUpsertInput,
    ) -> BlogResult<()> {
        let PostTranslationUpsertInput {
            title,
            excerpt,
            seo_title,
            seo_description,
            article_body,
            now,
        } = input;
        let locale = normalize_locale(locale)?;
        let existing = blog_post_translation::Entity::find()
            .inner_join(blog_post::Entity)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .filter(blog_post_translation::Column::Locale.eq(locale.as_str()))
            .one(txn)
            .await
            .map_err(BlogError::from)?;

        match existing {
            Some(existing) => {
                let mut active: blog_post_translation::ActiveModel = existing.into();
                if let Some(title) = title {
                    validate_title(&title)?;
                    active.title = Set(title);
                }
                match excerpt {
                    Patch::Keep => {}
                    Patch::Set(value) => active.excerpt = Set(Some(value)),
                    Patch::Clear => active.excerpt = Set(None),
                }
                match seo_title {
                    Patch::Keep => {}
                    Patch::Set(value) => active.seo_title = Set(Some(value)),
                    Patch::Clear => active.seo_title = Set(None),
                }
                match seo_description {
                    Patch::Keep => {}
                    Patch::Set(value) => active.seo_description = Set(Some(value)),
                    Patch::Clear => active.seo_description = Set(None),
                }
                if let Some(article_body) = article_body {
                    active.body = Set(article_body);
                }
                active.updated_at = Set(now.into());
                active.update(txn).await.map_err(BlogError::from)?;
            }
            None => {
                let title = title
                    .ok_or_else(|| BlogError::validation("Title is required for a new locale"))?;
                validate_title(&title)?;
                let article_body = article_body
                    .ok_or_else(|| BlogError::validation("Content is required for a new locale"))?;

                let optional = |patch: Patch<String>| match patch {
                    Patch::Keep | Patch::Clear => None,
                    Patch::Set(value) => Some(value),
                };

                blog_post_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    post_id: Set(post_id),
                    locale: Set(locale),
                    title: Set(title),
                    excerpt: Set(optional(excerpt)),
                    seo_title: Set(optional(seo_title)),
                    seo_description: Set(optional(seo_description)),
                    body: Set(article_body),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await
                .map_err(BlogError::from)?;
            }
        }

        Ok(())
    }
}

pub(crate) async fn load_post_subject_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<Option<PostSubjectSnapshot>> {
    let Some(post) = blog_post::Entity::find_by_id(post_id)
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(BlogError::from)?
    else {
        return Ok(None);
    };

    PostService::validate_persisted_version(&post)?;

    let channel_slugs = blog_post_channel_visibility::Entity::find()
        .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
        .filter(blog_post_channel_visibility::Column::PostId.eq(post_id))
        .order_by_asc(blog_post_channel_visibility::Column::ChannelSlug)
        .all(db)
        .await
        .map_err(BlogError::from)?
        .into_iter()
        .map(|row| row.channel_slug)
        .collect();

    Ok(Some(PostSubjectSnapshot {
        status: storage_to_status(&post.status)?,
        channel_slugs,
        version: post.version,
    }))
}
