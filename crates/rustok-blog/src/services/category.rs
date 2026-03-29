use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::PLATFORM_FALLBACK_LOCALE;
use rustok_core::{Action, Resource, SecurityContext};

use crate::dto::{
    CategoryListItem, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    UpdateCategoryInput,
};
use crate::entities::{blog_category, blog_category_translation};
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::{enforce_owned_scope, enforce_scope};

pub struct CategoryService {
    db: DatabaseConnection,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> BlogResult<Uuid> {
        enforce_scope(&security, Resource::Categories, Action::Create)?;
        validate_category_name(&input.name)?;
        let slug = normalize_category_slug(input.slug.as_deref(), &input.name);
        let now = Utc::now();
        let id = Uuid::new_v4();

        blog_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            parent_id: Set(input.parent_id),
            position: Set(input.position.unwrap_or(0)),
            depth: Set(0),
            post_count: Set(0),
            settings: Set(input.settings),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        blog_category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(normalize_locale(&input.locale)?),
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
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<CategoryResponse> {
        enforce_scope(&security, Resource::Categories, Action::Read)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;

        let translations = category
            .find_related(blog_category_translation::Entity)
            .all(&self.db)
            .await?;

        Ok(to_category_response(category, translations, locale))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> BlogResult<CategoryResponse> {
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        enforce_owned_scope(&security, Resource::Categories, Action::Update, category.id)?;

        let mut active: blog_category::ActiveModel = category.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(position) = input.position {
            active.position = Set(position);
        }
        if let Some(settings) = input.settings {
            active.settings = Set(settings);
        }
        let category = active.update(&self.db).await?;

        let locale = normalize_locale(&input.locale)?;
        let existing_translation = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::Locale.eq(&locale))
            .one(&self.db)
            .await?;

        match existing_translation {
            Some(translation) => {
                let mut active: blog_category_translation::ActiveModel = translation.into();
                if let Some(name) = &input.name {
                    validate_category_name(name)?;
                    active.name = Set(name.to_string());
                    if input.slug.is_none() {
                        active.slug = Set(normalize_slug_like(name));
                    }
                }
                if let Some(slug_value) = input.slug.as_deref() {
                    active.slug = Set(normalize_non_empty_slug(slug_value)?);
                }
                if input.description.is_some() {
                    active.description = Set(input.description);
                }
                active.update(&self.db).await?;
            }
            None => {
                let name = input
                    .name
                    .ok_or_else(|| BlogError::validation("Category name is required"))?;
                validate_category_name(&name)?;
                let slug = match input.slug.as_deref() {
                    Some(slug_value) => normalize_non_empty_slug(slug_value)?,
                    None => normalize_slug_like(&name),
                };

                blog_category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(name),
                    slug: Set(slug),
                    description: Set(input.description),
                }
                .insert(&self.db)
                .await?;
            }
        }

        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .all(&self.db)
            .await?;

        Ok(to_category_response(category, translations, &locale))
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        enforce_owned_scope(&security, Resource::Categories, Action::Delete, category.id)?;

        blog_category_translation::Entity::delete_many()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .exec(&self.db)
            .await?;

        category.delete(&self.db).await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListCategoriesFilter,
    ) -> BlogResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::Categories, Action::List)?;
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let page = filter.page.max(1);

        let paginator = blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(blog_category::Column::Position)
            .paginate(&self.db, filter.per_page.max(1));

        let total = paginator.num_items().await?;
        let categories = paginator.fetch_page(page - 1).await?;
        let category_ids: Vec<Uuid> = categories.iter().map(|category| category.id).collect();
        let all_translations = if category_ids.is_empty() {
            Vec::new()
        } else {
            blog_category_translation::Entity::find()
                .filter(blog_category_translation::Column::CategoryId.is_in(category_ids))
                .all(&self.db)
                .await?
        };

        let items = categories
            .into_iter()
            .map(|category| {
                let translations: Vec<&blog_category_translation::Model> = all_translations
                    .iter()
                    .filter(|translation| translation.category_id == category.id)
                    .collect();
                let (translation, effective_locale) =
                    resolve_category_translation(&translations, &locale);

                CategoryListItem {
                    id: category.id,
                    locale: locale.clone(),
                    effective_locale,
                    name: translation
                        .map(|translation| translation.name.clone())
                        .unwrap_or_default(),
                    slug: translation
                        .map(|translation| translation.slug.clone())
                        .unwrap_or_default(),
                    parent_id: category.parent_id,
                    position: category.position,
                    settings: category.settings,
                    created_at: category.created_at.into(),
                }
            })
            .collect();

        Ok((items, total))
    }

    pub(crate) async fn ensure_exists_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> BlogResult<()> {
        let exists = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        if exists.is_none() {
            return Err(BlogError::category_not_found(category_id));
        }
        Ok(())
    }
}

fn validate_category_name(name: &str) -> BlogResult<()> {
    if name.trim().is_empty() {
        return Err(BlogError::validation("Category name cannot be empty"));
    }
    if name.len() > 255 {
        return Err(BlogError::validation(
            "Category name cannot exceed 255 characters",
        ));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    let normalized = locale.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(BlogError::validation("Locale cannot be empty"));
    }
    Ok(normalized)
}

fn normalize_category_slug(input: Option<&str>, fallback_name: &str) -> String {
    input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_slug_like)
        .unwrap_or_else(|| normalize_slug_like(fallback_name))
}

fn normalize_non_empty_slug(slug: &str) -> BlogResult<String> {
    let normalized = normalize_slug_like(slug);
    if normalized.is_empty() {
        return Err(BlogError::validation("Slug cannot be empty"));
    }
    Ok(normalized)
}

fn normalize_slug_like(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn resolve_category_translation<'a>(
    translations: &[&'a blog_category_translation::Model],
    locale: &str,
) -> (Option<&'a blog_category_translation::Model>, String) {
    if let Some(translation) = translations
        .iter()
        .copied()
        .find(|translation| translation.locale == locale)
    {
        return (Some(translation), locale.to_string());
    }
    if let Some(translation) = translations
        .iter()
        .copied()
        .find(|translation| translation.locale == PLATFORM_FALLBACK_LOCALE)
    {
        return (Some(translation), PLATFORM_FALLBACK_LOCALE.to_string());
    }
    if let Some(translation) = translations.first().copied() {
        return (Some(translation), translation.locale.clone());
    }
    (None, locale.to_string())
}

fn to_category_response(
    category: blog_category::Model,
    translations: Vec<blog_category_translation::Model>,
    locale: &str,
) -> CategoryResponse {
    let translations_refs: Vec<&blog_category_translation::Model> = translations.iter().collect();
    let (translation, effective_locale) = resolve_category_translation(&translations_refs, locale);

    CategoryResponse {
        id: category.id,
        tenant_id: category.tenant_id,
        locale: locale.to_string(),
        effective_locale,
        available_locales: translations
            .iter()
            .map(|item| item.locale.clone())
            .collect(),
        name: translation
            .map(|translation| translation.name.clone())
            .unwrap_or_default(),
        slug: translation
            .map(|translation| translation.slug.clone())
            .unwrap_or_default(),
        description: translation.and_then(|translation| translation.description.clone()),
        parent_id: category.parent_id,
        position: category.position,
        settings: category.settings,
        created_at: category.created_at.into(),
        updated_at: category.updated_at.into(),
    }
}
