use chrono::Utc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Statement, TransactionTrait,
    sea_query::{Expr, Query, SelectStatement},
};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;

use crate::dto::{CategoryListItem, CategoryResponse, CreateCategoryInput, UpdateCategoryInput};
use crate::entities::{forum_category, forum_category_lifecycle, forum_category_translation};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::subscription::SubscriptionService;

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
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Create)?;
        validate_category_name(&input.name)?;
        let locale = normalize_locale(&input.locale)?;
        let slug = normalize_required_slug(&input.slug)?;
        let requested_position = input.position.unwrap_or(0);
        if requested_position < 0 {
            return Err(ForumError::Validation(
                "Category position cannot be negative".to_string(),
            ));
        }

        let now = Utc::now();
        let id = Uuid::new_v4();
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;

        if let Some(parent_id) = input.parent_id {
            Self::find_category_in_tx(&txn, tenant_id, parent_id).await?;
        }

        shift_siblings_for_insert_in_tx(&txn, tenant_id, input.parent_id, requested_position, now)
            .await?;

        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            parent_id: Set(input.parent_id),
            position: Set(requested_position),
            icon: Set(input.icon),
            color: Set(input.color),
            moderated: Set(input.moderated),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        forum_category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            name: Set(input.name),
            slug: Set(slug),
            description: Set(input.description),
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;
        self.get(tenant_id, security, id, &locale).await
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
    ) -> ForumResult<CategoryResponse> {
        self.get_with_locale_fallback(tenant_id, security, category_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;
        let translations = self.load_translations(tenant_id, category_id).await?;
        let is_subscribed = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &[category_id], security.user_id)
            .await?
            .get(&category_id)
            .copied()
            .unwrap_or(false);
        to_category_response(
            category,
            translations,
            is_subscribed,
            &locale,
            fallback_locale.as_deref(),
        )
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Update)?;
        let locale = normalize_locale(&input.locale)?;
        let txn = self.db.begin().await?;
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        let mut active: forum_category::ActiveModel = category.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(position) = input.position {
            active.position = Set(position);
        }
        if input.icon.is_some() {
            active.icon = Set(input.icon);
        }
        if input.color.is_some() {
            active.color = Set(input.color);
        }
        if let Some(moderated) = input.moderated {
            active.moderated = Set(moderated);
        }
        active.update(&txn).await?;

        let existing_translation = forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category_id))
            .filter(forum_category_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;

        match existing_translation {
            Some(existing_translation) => {
                let mut active: forum_category_translation::ActiveModel =
                    existing_translation.into();
                if let Some(name) = input.name {
                    validate_category_name(&name)?;
                    active.name = Set(name.clone());
                    if input.slug.is_none() {
                        active.slug = Set(normalize_slug(&name));
                    }
                }
                if let Some(slug) = input.slug.as_deref() {
                    active.slug = Set(normalize_required_slug(slug)?);
                }
                if input.description.is_some() {
                    active.description = Set(input.description);
                }
                active.update(&txn).await?;
            }
            None => {
                let name = input.name.ok_or_else(|| {
                    ForumError::Validation("Category name is required".to_string())
                })?;
                validate_category_name(&name)?;
                let slug = input
                    .slug
                    .as_deref()
                    .map(normalize_required_slug)
                    .transpose()?
                    .unwrap_or_else(|| normalize_slug(&name));

                forum_category_translation::ActiveModel {
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

        txn.commit().await?;
        self.get(tenant_id, security, category_id, &locale).await
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumCategories, Action::Delete)?;
        let txn = self.db.begin().await?;
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        forum_category_translation::Entity::delete_many()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category_id))
            .exec(&txn)
            .await?;

        forum_category::Entity::delete_by_id(category.id)
            .exec(&txn)
            .await?;

        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .list_paginated_with_locale_fallback(tenant_id, security, locale, 1, 1000, None)
            .await?;
        Ok(items)
    }

    #[instrument(skip(self, security))]
    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .list_paginated_with_locale_fallback(
                tenant_id,
                security,
                locale,
                1,
                1000,
                fallback_locale,
            )
            .await?;
        Ok(items)
    }

    #[instrument(skip(self, security))]
    pub async fn list_paginated_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let query = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(
                Expr::col((forum_category::Entity, forum_category::Column::Id))
                    .not_in_subquery(archived_category_ids_subquery(tenant_id)),
            );
        let paginator = query
            .order_by_asc(forum_category::Column::Position)
            .paginate(&self.db, per_page.max(1));
        let total = paginator.num_items().await?;
        let categories = paginator.fetch_page(page.saturating_sub(1)).await?;
        let category_ids: Vec<Uuid> = categories.iter().map(|item| item.id).collect();
        let translations_by_category_id = self
            .load_translations_map_for_categories(tenant_id, &category_ids)
            .await?;
        let subscription_flags = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &category_ids, security.user_id)
            .await?;

        let mut items = Vec::with_capacity(categories.len());
        for category in categories {
            let localized = translations_by_category_id
                .get(&category.id)
                .cloned()
                .unwrap_or_default();
            let resolved = resolve_by_locale_with_fallback(
                &localized,
                &locale,
                fallback_locale.as_deref(),
                |translation| translation.locale.as_str(),
            );
            let translation = resolved.item.ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {} has no localized translation",
                    category.id
                ))
            })?;

            items.push(CategoryListItem {
                id: category.id,
                requested_locale: locale.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&localized, |translation| {
                    translation.locale.as_str()
                }),
                name: translation.name.clone(),
                slug: translation.slug.clone(),
                description: translation.description.clone(),
                icon: category.icon.clone(),
                color: category.color.clone(),
                topic_count: category.topic_count,
                reply_count: category.reply_count,
                is_subscribed: subscription_flags
                    .get(&category.id)
                    .copied()
                    .unwrap_or(false),
            });
        }

        Ok((items, total))
    }

    pub(crate) async fn ensure_exists_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<()> {
        Self::find_category_in_tx(txn, tenant_id, category_id).await?;
        Ok(())
    }

    pub(crate) async fn find_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<forum_category::Model> {
        let existing = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        existing.ok_or(ForumError::CategoryNotFound(category_id))
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        let mut active: forum_category::ActiveModel = category.clone().into();
        active.topic_count = Set((category.topic_count + topic_delta).max(0));
        active.reply_count = Set((category.reply_count + reply_delta).max(0));
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(())
    }

    async fn load_translations(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<Vec<forum_category_translation::Model>> {
        Ok(forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category_id))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_for_categories(
        &self,
        tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> ForumResult<Vec<forum_category_translation::Model>> {
        if category_ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.is_in(category_ids.to_vec()))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_map_for_categories(
        &self,
        tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, Vec<forum_category_translation::Model>>> {
        let mut map: HashMap<Uuid, Vec<forum_category_translation::Model>> = HashMap::new();
        for translation in self
            .load_translations_for_categories(tenant_id, category_ids)
            .await?
        {
            map.entry(translation.category_id)
                .or_default()
                .push(translation);
        }
        Ok(map)
    }
}

fn archived_category_ids_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(forum_category_lifecycle::Column::CategoryId)
        .from(forum_category_lifecycle::Entity)
        .and_where(
            Expr::col((
                forum_category_lifecycle::Entity,
                forum_category_lifecycle::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .to_owned()
}

async fn lock_category_tree_in_tx(txn: &DatabaseTransaction, tenant_id: Uuid) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [tenant_id.to_string().into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum category creation does not support {backend:?}"
        ))),
    }
}

async fn shift_siblings_for_insert_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    requested_position: i32,
    now: chrono::DateTime<Utc>,
) -> ForumResult<()> {
    let siblings = match parent_id {
        Some(parent_id) => {
            forum_category::Entity::find()
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .filter(forum_category::Column::ParentId.eq(parent_id))
                .filter(forum_category::Column::Position.gte(requested_position))
                .order_by_desc(forum_category::Column::Position)
                .order_by_desc(forum_category::Column::Id)
                .all(txn)
                .await?
        }
        None => {
            forum_category::Entity::find()
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .filter(forum_category::Column::ParentId.is_null())
                .filter(forum_category::Column::Position.gte(requested_position))
                .order_by_desc(forum_category::Column::Position)
                .order_by_desc(forum_category::Column::Id)
                .all(txn)
                .await?
        }
    };

    for sibling in siblings {
        let next_position = sibling.position.checked_add(1).ok_or_else(|| {
            ForumError::Validation("Category sibling position exceeds i32 range".to_string())
        })?;
        let mut active: forum_category::ActiveModel = sibling.into();
        active.position = Set(next_position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }

    Ok(())
}

fn to_category_response(
    category: forum_category::Model,
    translations: Vec<forum_category_translation::Model>,
    is_subscribed: bool,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ForumResult<CategoryResponse> {
    let resolved =
        resolve_by_locale_with_fallback(&translations, locale, fallback_locale, |translation| {
            translation.locale.as_str()
        });
    let translation = resolved.item.ok_or_else(|| {
        ForumError::Validation(format!(
            "Forum category {} has no localized translation",
            category.id
        ))
    })?;

    Ok(CategoryResponse {
        id: category.id,
        requested_locale: locale.to_string(),
        locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        available_locales: available_locales_from(&translations, |translation| {
            translation.locale.as_str()
        }),
        name: translation.name.clone(),
        slug: translation.slug.clone(),
        description: translation.description.clone(),
        icon: category.icon,
        color: category.color,
        parent_id: category.parent_id,
        position: category.position,
        topic_count: category.topic_count,
        reply_count: category.reply_count,
        moderated: category.moderated,
        is_subscribed,
    })
}

fn validate_category_name(name: &str) -> ForumResult<()> {
    if name.trim().is_empty() {
        return Err(ForumError::Validation(
            "Category name cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn normalize_required_slug(value: &str) -> ForumResult<String> {
    let slug = normalize_slug(value);
    if slug.is_empty() {
        return Err(ForumError::Validation(
            "Category slug cannot be empty".to_string(),
        ));
    }
    Ok(slug)
}

fn normalize_slug(value: &str) -> String {
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
