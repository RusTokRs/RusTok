use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::category::{
    CategoryListItem, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    UpdateCategoryInput,
};
use crate::entities::{category, category_translation};
use crate::error::{ContentError, ContentResult};
use crate::locale::PLATFORM_FALLBACK_LOCALE;

pub struct CategoryService {
    db: DatabaseConnection,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, _security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        _security: SecurityContext,
        input: CreateCategoryInput,
    ) -> ContentResult<Uuid> {
        if input.name.trim().is_empty() {
            return Err(ContentError::validation("Category name cannot be empty"));
        }
        if input.name.len() > 255 {
            return Err(ContentError::validation(
                "Category name cannot exceed 255 characters",
            ));
        }

        let slug = input
            .slug
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| slug::slugify(&input.name));

        let now = Utc::now();
        let id = Uuid::new_v4();

        category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            parent_id: Set(input.parent_id),
            position: Set(input.position.unwrap_or(0)),
            depth: Set(0),
            node_count: Set(0),
            settings: Set(input.settings),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(input.locale),
            name: Set(input.name),
            slug: Set(slug),
            description: Set(input.description),
        }
        .insert(&self.db)
        .await?;

        Ok(id)
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> ContentResult<CategoryResponse> {
        let cat = category::Entity::find_by_id(category_id)
            .filter(category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| ContentError::category_not_found(category_id))?;

        let translations = cat
            .find_related(category_translation::Entity)
            .all(&self.db)
            .await?;

        Ok(to_response(cat, translations, locale))
    }

    #[instrument(skip(self, _security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        _security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ContentResult<CategoryResponse> {
        let cat = category::Entity::find_by_id(category_id)
            .filter(category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| ContentError::category_not_found(category_id))?;

        let mut active: category::ActiveModel = cat.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(pos) = input.position {
            active.position = Set(pos);
        }
        if let Some(settings) = input.settings {
            active.settings = Set(settings);
        }
        let cat = active.update(&self.db).await?;

        let existing_tr = category_translation::Entity::find()
            .filter(category_translation::Column::CategoryId.eq(category_id))
            .filter(category_translation::Column::Locale.eq(&input.locale))
            .one(&self.db)
            .await?;

        match existing_tr {
            Some(tr) => {
                let mut active: category_translation::ActiveModel = tr.into();
                if let Some(name) = &input.name {
                    active.name = Set(name.clone());
                    if input.slug.is_none() {
                        active.slug = Set(slug::slugify(name));
                    }
                }
                if let Some(s) = input.slug {
                    active.slug = Set(s);
                }
                if let Some(desc) = input.description {
                    active.description = Set(Some(desc));
                }
                active.update(&self.db).await?;
            }
            None => {
                category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(input.locale.clone()),
                    name: Set(input.name.unwrap_or_default()),
                    slug: Set(input.slug.unwrap_or_default()),
                    description: Set(input.description),
                }
                .insert(&self.db)
                .await?;
            }
        }

        let translations = category_translation::Entity::find()
            .filter(category_translation::Column::CategoryId.eq(category_id))
            .all(&self.db)
            .await?;

        Ok(to_response(cat, translations, &input.locale))
    }

    #[instrument(skip(self, _security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        _security: SecurityContext,
    ) -> ContentResult<()> {
        let cat = category::Entity::find_by_id(category_id)
            .filter(category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| ContentError::category_not_found(category_id))?;

        category_translation::Entity::delete_many()
            .filter(category_translation::Column::CategoryId.eq(category_id))
            .exec(&self.db)
            .await?;

        cat.delete(&self.db).await?;
        Ok(())
    }

    #[instrument(skip(self, _security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        _security: SecurityContext,
        filter: ListCategoriesFilter,
    ) -> ContentResult<(Vec<CategoryListItem>, u64)> {
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let page = filter.page.max(1);

        let paginator = category::Entity::find()
            .filter(category::Column::TenantId.eq(tenant_id))
            .order_by_asc(category::Column::Position)
            .paginate(&self.db, filter.per_page);

        let total = paginator.num_items().await?;
        let cats = paginator.fetch_page(page - 1).await?;

        let ids: Vec<Uuid> = cats.iter().map(|c| c.id).collect();
        let all_translations = if ids.is_empty() {
            vec![]
        } else {
            category_translation::Entity::find()
                .filter(category_translation::Column::CategoryId.is_in(ids))
                .all(&self.db)
                .await?
        };

        let items = cats
            .into_iter()
            .map(|cat| {
                let trs: Vec<&category_translation::Model> = all_translations
                    .iter()
                    .filter(|t| t.category_id == cat.id)
                    .collect();
                let (tr, effective_locale) = resolve_translation(&trs, &locale);
                CategoryListItem {
                    id: cat.id,
                    locale: locale.clone(),
                    effective_locale,
                    name: tr.map(|t| t.name.clone()).unwrap_or_default(),
                    slug: tr.map(|t| t.slug.clone()).unwrap_or_default(),
                    parent_id: cat.parent_id,
                    position: cat.position,
                    settings: cat.settings,
                    created_at: cat.created_at.into(),
                }
            })
            .collect();

        Ok((items, total))
    }
}

fn resolve_translation<'a>(
    translations: &[&'a category_translation::Model],
    locale: &str,
) -> (Option<&'a category_translation::Model>, String) {
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

fn to_response(
    cat: category::Model,
    translations: Vec<category_translation::Model>,
    locale: &str,
) -> CategoryResponse {
    let trs: Vec<&category_translation::Model> = translations.iter().collect();
    let (tr, effective_locale) = resolve_translation(&trs, locale);
    let available_locales = translations.iter().map(|t| t.locale.clone()).collect();

    CategoryResponse {
        id: cat.id,
        tenant_id: cat.tenant_id,
        locale: locale.to_string(),
        effective_locale,
        available_locales,
        name: tr.map(|t| t.name.clone()).unwrap_or_default(),
        slug: tr.map(|t| t.slug.clone()).unwrap_or_default(),
        description: tr.and_then(|t| t.description.clone()),
        parent_id: cat.parent_id,
        position: cat.position,
        settings: cat.settings,
        created_at: cat.created_at.into(),
        updated_at: cat.updated_at.into(),
    }
}
