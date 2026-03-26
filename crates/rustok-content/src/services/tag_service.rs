use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::tag::{CreateTagInput, ListTagsFilter, TagListItem, TagResponse, UpdateTagInput};
use crate::entities::{tag, tag_translation, taggable};
use crate::error::{ContentError, ContentResult};
use crate::locale::PLATFORM_FALLBACK_LOCALE;

pub struct TagService {
    db: DatabaseConnection,
}

impl TagService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, _security, input))]
    pub async fn create_tag(
        &self,
        tenant_id: Uuid,
        _security: SecurityContext,
        input: CreateTagInput,
    ) -> ContentResult<Uuid> {
        if input.name.trim().is_empty() {
            return Err(ContentError::validation("Tag name cannot be empty"));
        }
        if input.name.len() > 100 {
            return Err(ContentError::validation(
                "Tag name cannot exceed 100 characters",
            ));
        }

        let slug = input
            .slug
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| slug::slugify(&input.name));

        let id = Uuid::new_v4();
        let now = Utc::now();

        tag::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            use_count: Set(0),
            created_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        tag_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tag_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(input.locale),
            name: Set(input.name),
            slug: Set(slug),
        }
        .insert(&self.db)
        .await?;

        Ok(id)
    }

    #[instrument(skip(self))]
    pub async fn get_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        locale: &str,
    ) -> ContentResult<TagResponse> {
        let t = tag::Entity::find_by_id(tag_id)
            .filter(tag::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| ContentError::tag_not_found(tag_id))?;

        let translations = t
            .find_related(tag_translation::Entity)
            .all(&self.db)
            .await?;
        Ok(to_tag_response(t, translations, locale))
    }

    #[instrument(skip(self, _security, input))]
    pub async fn update_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        _security: SecurityContext,
        input: UpdateTagInput,
    ) -> ContentResult<TagResponse> {
        let t = tag::Entity::find_by_id(tag_id)
            .filter(tag::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| ContentError::tag_not_found(tag_id))?;

        let existing_tr = tag_translation::Entity::find()
            .filter(tag_translation::Column::TagId.eq(tag_id))
            .filter(tag_translation::Column::Locale.eq(&input.locale))
            .one(&self.db)
            .await?;

        match existing_tr {
            Some(tr) => {
                let mut active: tag_translation::ActiveModel = tr.into();
                if let Some(name) = &input.name {
                    active.name = Set(name.clone());
                    if input.slug.is_none() {
                        active.slug = Set(slug::slugify(name));
                    }
                }
                if let Some(s) = input.slug {
                    active.slug = Set(s);
                }
                active.update(&self.db).await?;
            }
            None => {
                tag_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tag_id: Set(tag_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(input.locale.clone()),
                    name: Set(input.name.unwrap_or_default()),
                    slug: Set(input.slug.unwrap_or_default()),
                }
                .insert(&self.db)
                .await?;
            }
        }

        let translations = tag_translation::Entity::find()
            .filter(tag_translation::Column::TagId.eq(tag_id))
            .all(&self.db)
            .await?;

        Ok(to_tag_response(t, translations, &input.locale))
    }

    #[instrument(skip(self, _security))]
    pub async fn delete_tag(
        &self,
        tenant_id: Uuid,
        tag_id: Uuid,
        _security: SecurityContext,
    ) -> ContentResult<()> {
        let t = tag::Entity::find_by_id(tag_id)
            .filter(tag::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| ContentError::tag_not_found(tag_id))?;

        taggable::Entity::delete_many()
            .filter(taggable::Column::TagId.eq(tag_id))
            .exec(&self.db)
            .await?;

        tag_translation::Entity::delete_many()
            .filter(tag_translation::Column::TagId.eq(tag_id))
            .exec(&self.db)
            .await?;

        t.delete(&self.db).await?;
        Ok(())
    }

    #[instrument(skip(self, _security))]
    pub async fn list_tags(
        &self,
        tenant_id: Uuid,
        _security: SecurityContext,
        filter: ListTagsFilter,
    ) -> ContentResult<(Vec<TagListItem>, u64)> {
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let page = filter.page.max(1);

        let paginator = tag::Entity::find()
            .filter(tag::Column::TenantId.eq(tenant_id))
            .order_by_desc(tag::Column::UseCount)
            .paginate(&self.db, filter.per_page);

        let total = paginator.num_items().await?;
        let tags = paginator.fetch_page(page - 1).await?;

        let ids: Vec<Uuid> = tags.iter().map(|t| t.id).collect();
        let all_translations = if ids.is_empty() {
            vec![]
        } else {
            tag_translation::Entity::find()
                .filter(tag_translation::Column::TagId.is_in(ids))
                .all(&self.db)
                .await?
        };

        let items = tags
            .into_iter()
            .map(|t| {
                let trs: Vec<&tag_translation::Model> = all_translations
                    .iter()
                    .filter(|tr| tr.tag_id == t.id)
                    .collect();
                let (tr, effective_locale) = resolve_tag_translation(&trs, &locale);
                TagListItem {
                    id: t.id,
                    locale: locale.clone(),
                    effective_locale,
                    name: tr.map(|r| r.name.clone()).unwrap_or_default(),
                    slug: tr.map(|r| r.slug.clone()).unwrap_or_default(),
                    use_count: t.use_count,
                    created_at: t.created_at.into(),
                }
            })
            .collect();

        Ok((items, total))
    }

    /// Find a tag by slug for a locale, or create it if it doesn't exist.
    /// Used by PostService and TopicService to attach tags by name.
    pub async fn find_or_create_by_name(
        &self,
        tenant_id: Uuid,
        locale: &str,
        name: &str,
    ) -> ContentResult<Uuid> {
        let tag_slug = slug::slugify(name);

        let existing = tag_translation::Entity::find()
            .filter(tag_translation::Column::TenantId.eq(tenant_id))
            .filter(tag_translation::Column::Locale.eq(locale))
            .filter(tag_translation::Column::Slug.eq(&tag_slug))
            .one(&self.db)
            .await?;

        if let Some(tr) = existing {
            return Ok(tr.tag_id);
        }

        let id = Uuid::new_v4();
        let now = Utc::now();

        tag::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            use_count: Set(0),
            created_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        tag_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tag_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.to_string()),
            name: Set(name.to_string()),
            slug: Set(tag_slug),
        }
        .insert(&self.db)
        .await?;

        Ok(id)
    }

    /// Replace all tags for a content item.
    /// target_type examples: "blog_post", "forum_topic", "product"
    pub async fn set_tags_for(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        target_id: Uuid,
        tag_names: &[String],
        locale: &str,
    ) -> ContentResult<()> {
        taggable::Entity::delete_many()
            .filter(taggable::Column::TargetType.eq(target_type))
            .filter(taggable::Column::TargetId.eq(target_id))
            .exec(&self.db)
            .await?;

        let now = Utc::now();
        for name in tag_names {
            let tag_id = self.find_or_create_by_name(tenant_id, locale, name).await?;
            taggable::ActiveModel {
                tag_id: Set(tag_id),
                target_type: Set(target_type.to_string()),
                target_id: Set(target_id),
                created_at: Set(now.into()),
            }
            .insert(&self.db)
            .await?;
        }

        Ok(())
    }

    /// Get all tags for a content item with locale fallback.
    pub async fn get_tags_for(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        target_id: Uuid,
        locale: &str,
    ) -> ContentResult<Vec<TagResponse>> {
        let taggables = taggable::Entity::find()
            .filter(taggable::Column::TargetType.eq(target_type))
            .filter(taggable::Column::TargetId.eq(target_id))
            .all(&self.db)
            .await?;

        let tag_ids: Vec<Uuid> = taggables.iter().map(|t| t.tag_id).collect();
        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        let tags = tag::Entity::find()
            .filter(tag::Column::TenantId.eq(tenant_id))
            .filter(tag::Column::Id.is_in(tag_ids.clone()))
            .all(&self.db)
            .await?;

        let all_translations = tag_translation::Entity::find()
            .filter(tag_translation::Column::TagId.is_in(tag_ids))
            .all(&self.db)
            .await?;

        let responses = tags
            .into_iter()
            .map(|t| {
                let trs: Vec<tag_translation::Model> = all_translations
                    .iter()
                    .filter(|tr| tr.tag_id == t.id)
                    .cloned()
                    .collect();
                to_tag_response(t, trs, locale)
            })
            .collect();

        Ok(responses)
    }
}

fn resolve_tag_translation<'a>(
    translations: &[&'a tag_translation::Model],
    locale: &str,
) -> (Option<&'a tag_translation::Model>, String) {
    if let Some(tr) = translations.iter().copied().find(|t| t.locale == locale) {
        return (Some(tr), locale.to_string());
    }
    if let Some(tr) = translations
        .iter()
        .copied()
        .find(|t| t.locale == PLATFORM_FALLBACK_LOCALE)
    {
        return (Some(tr), PLATFORM_FALLBACK_LOCALE.to_string());
    }
    if let Some(tr) = translations.first().copied() {
        return (Some(tr), tr.locale.clone());
    }
    (None, locale.to_string())
}

fn to_tag_response(
    t: tag::Model,
    translations: Vec<tag_translation::Model>,
    locale: &str,
) -> TagResponse {
    let trs: Vec<&tag_translation::Model> = translations.iter().collect();
    let (tr, effective_locale) = resolve_tag_translation(&trs, locale);

    TagResponse {
        id: t.id,
        tenant_id: t.tenant_id,
        locale: locale.to_string(),
        effective_locale,
        name: tr.map(|r| r.name.clone()).unwrap_or_default(),
        slug: tr.map(|r| r.slug.clone()).unwrap_or_default(),
        use_count: t.use_count,
        created_at: t.created_at.into(),
    }
}
