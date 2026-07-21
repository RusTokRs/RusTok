use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CategoryListItem, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    UpdateCategoryInput,
};
use crate::entities::{blog_category, blog_category_translation};
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::enforce_scope;

pub struct CategoryService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> BlogResult<Uuid> {
        enforce_scope(&security, Resource::BlogCategories, Action::Create)?;
        validate_category_name(&input.name)?;
        let slug = normalize_category_slug(input.slug.as_deref(), &input.name)?;
        let locale = normalize_locale(&input.locale)?;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        if let Some(parent_id) = input.parent_id {
            Self::ensure_exists_in_tx(&txn, tenant_id, parent_id).await?;
        }

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
        .insert(&txn)
        .await?;

        blog_category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(locale),
            name: Set(input.name),
            slug: Set(slug),
            description: Set(input.description),
        }
        .insert(&txn)
        .await?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(id)
    }

    #[instrument(skip(self, security))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<CategoryResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::Read)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;

        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
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
        enforce_scope(&security, Resource::BlogCategories, Action::Update)?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;

        let mut active: blog_category::ActiveModel = category.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(position) = input.position {
            active.position = Set(position);
        }
        if let Some(settings) = input.settings {
            active.settings = Set(settings);
        }
        let category = active.update(&txn).await?;

        let locale = normalize_locale(&input.locale)?;
        let existing_translation = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .filter(blog_category_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;

        match existing_translation {
            Some(translation) => {
                let mut active: blog_category_translation::ActiveModel = translation.into();
                if let Some(name) = &input.name {
                    validate_category_name(name)?;
                    active.name = Set(name.to_string());
                    if input.slug.is_none() {
                        active.slug = Set(normalize_non_empty_slug(name)?);
                    }
                }
                if let Some(slug_value) = input.slug.as_deref() {
                    active.slug = Set(normalize_non_empty_slug(slug_value)?);
                }
                if input.description.is_some() {
                    active.description = Set(input.description);
                }
                active.update(&txn).await?;
            }
            None => {
                let name = input
                    .name
                    .ok_or_else(|| BlogError::validation("Category name is required"))?;
                validate_category_name(&name)?;
                let slug = match input.slug.as_deref() {
                    Some(slug_value) => normalize_non_empty_slug(slug_value)?,
                    None => normalize_non_empty_slug(&name)?,
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
                .insert(&txn)
                .await?;
            }
        }

        self.publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)
            .await?;

        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        let response = to_category_response(category, translations, &locale);

        txn.commit().await.map_err(BlogError::from)?;
        Ok(response)
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogCategories, Action::Delete)?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;

        blog_category_translation::Entity::delete_many()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .exec(&txn)
            .await?;

        category.delete(&txn).await?;

        self.publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)
            .await?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListCategoriesFilter,
    ) -> BlogResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::BlogCategories, Action::List)?;
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let page = filter.page.max(1);
        let per_page = filter.per_page.clamp(1, 100);

        let paginator = blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(blog_category::Column::Position)
            .paginate(&self.db, per_page);

        let total = paginator.num_items().await?;
        let categories = paginator.fetch_page(page - 1).await?;
        let category_ids: Vec<Uuid> = categories.iter().map(|category| category.id).collect();
        let all_translations = if category_ids.is_empty() {
            Vec::new()
        } else {
            blog_category_translation::Entity::find()
                .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
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

    async fn publish_blog_reindex_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> BlogResult<()> {
        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                actor_id,
                DomainEvent::ReindexRequested {
                    target_type: "blog".to_string(),
                    target_id: None,
                },
            )
            .await
            .map_err(BlogError::from)
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

fn normalize_category_slug(input: Option<&str>, fallback_name: &str) -> BlogResult<String> {
    let value = input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_name);
    normalize_non_empty_slug(value)
}

fn normalize_non_empty_slug(slug: &str) -> BlogResult<String> {
    let normalized = normalize_slug_like(slug);
    if normalized.is_empty() {
        return Err(BlogError::validation(
            "Slug must contain at least one ASCII letter or digit",
        ));
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
        return (
            Some(translation),
            PLATFORM_FALLBACK_LOCALE.to_string(),
        );
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
