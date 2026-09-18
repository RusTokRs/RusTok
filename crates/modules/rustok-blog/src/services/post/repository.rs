use super::*;

impl PostService {
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
    }

    pub(super) async fn load_translations(
        &self,
        post_id: Uuid,
    ) -> BlogResult<Vec<blog_post_translation::Model>> {
        blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .all(&self.db)
            .await
            .map_err(BlogError::from)
    }

    pub(super) async fn load_translations_map(
        &self,
        post_ids: &[Uuid],
    ) -> BlogResult<HashMap<Uuid, Vec<blog_post_translation::Model>>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let translations = blog_post_translation::Entity::find()
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

    pub(super) async fn load_channel_slugs(&self, post_id: Uuid) -> BlogResult<Vec<String>> {
        let records = blog_post_channel_visibility::Entity::find()
            .filter(blog_post_channel_visibility::Column::PostId.eq(post_id))
            .order_by_asc(blog_post_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        Ok(records.into_iter().map(|item| item.channel_slug).collect())
    }

    pub(super) async fn load_channel_slugs_map(
        &self,
        post_ids: &[Uuid],
    ) -> BlogResult<HashMap<Uuid, Vec<String>>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let records = blog_post_channel_visibility::Entity::find()
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
        blog_post_channel_visibility::Entity::delete_many()
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
            return Err(BlogError::duplicate_slug(
                slug.to_string(),
                PLATFORM_FALLBACK_LOCALE.to_string(),
            ));
        }

        Ok(())
    }

    pub(super) async fn upsert_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
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
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .filter(blog_post_translation::Column::Locale.eq(locale.as_str()))
            .one(txn)
            .await
            .map_err(BlogError::from)?;

        match existing {
            Some(existing) => {
                let mut active: blog_post_translation::ActiveModel = existing.clone().into();
                if let Some(title) = title {
                    validate_title(&title)?;
                    active.title = Set(title);
                }
                if excerpt.is_some() {
                    active.excerpt = Set(excerpt);
                }
                if seo_title.is_some() {
                    active.seo_title = Set(seo_title);
                }
                if seo_description.is_some() {
                    active.seo_description = Set(seo_description);
                }
                if let Some(article_body) = article_body {
                    active.body = Set(article_body);
                }
                active.updated_at = Set(now.into());
                active.update(txn).await.map_err(BlogError::from)?;
            }
            None => {
                let baseline = self
                    .translation_seed_in_tx(txn, post_id)
                    .await
                    .map_err(BlogError::from)?;
                let title = title
                    .or_else(|| baseline.as_ref().map(|item| item.title.clone()))
                    .ok_or_else(|| BlogError::validation("Title is required for a new locale"))?;
                validate_title(&title)?;
                let excerpt =
                    excerpt.or_else(|| baseline.as_ref().and_then(|item| item.excerpt.clone()));
                let seo_title =
                    seo_title.or_else(|| baseline.as_ref().and_then(|item| item.seo_title.clone()));
                let seo_description = seo_description.or_else(|| {
                    baseline
                        .as_ref()
                        .and_then(|item| item.seo_description.clone())
                });
                let article_body = article_body
                    .or_else(|| baseline.as_ref().map(|item| item.body.clone()))
                    .ok_or_else(|| BlogError::validation("Content is required for a new locale"))?;

                blog_post_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    post_id: Set(post_id),
                    locale: Set(locale),
                    title: Set(title),
                    excerpt: Set(excerpt),
                    seo_title: Set(seo_title),
                    seo_description: Set(seo_description),
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

    pub(super) async fn translation_seed_in_tx(
        &self,
        txn: &DatabaseTransaction,
        post_id: Uuid,
    ) -> Result<Option<blog_post_translation::Model>, sea_orm::DbErr> {
        blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .order_by_asc(blog_post_translation::Column::CreatedAt)
            .one(txn)
            .await
    }


}
