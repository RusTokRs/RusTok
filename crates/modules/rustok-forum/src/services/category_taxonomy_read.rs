use std::collections::HashMap;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_taxonomy::{TaxonomyError, TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    sea_query::{Query, SelectStatement},
};
use uuid::Uuid;

use crate::dto::{CategoryListItem, CategoryResponse};
use crate::entities::{
    forum_category, forum_category_lifecycle, forum_category_taxonomy_binding,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::subscription::SubscriptionService;

/// Transitional CAT-5 read adapter.
///
/// Forum still owns category membership, lifecycle/visibility, moderation,
/// counters and subscription state. Canonical localized copy and presentation
/// are read only through the typed Forum -> Taxonomy Category binding.
pub(in crate::services) struct CategoryTaxonomyReadService {
    db: DatabaseConnection,
}

impl CategoryTaxonomyReadService {
    pub(in crate::services) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(in crate::services) async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;

        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;
        let binding = forum_category_taxonomy_binding::Entity::find_by_id((tenant_id, category_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| missing_binding(category_id))?;

        let mut projections = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some("forum"),
                Some(&[binding.taxonomy_category_id]),
                locale,
                fallback_locale,
            )
            .await
            .map_err(map_taxonomy_read_error)?;
        let projection = projections
            .pop()
            .ok_or_else(|| missing_projection(category_id, binding.taxonomy_category_id))?;
        if !projections.is_empty() || projection.id != binding.taxonomy_category_id {
            return Err(ForumError::Validation(
                "Forum category Taxonomy projection returned an inconsistent identity".to_string(),
            ));
        }

        let parent_id = self
            .forum_parent_id_for_taxonomy_parent(tenant_id, projection.parent_id)
            .await?;
        let is_subscribed = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &[category_id], security.user_id)
            .await?
            .get(&category_id)
            .copied()
            .unwrap_or(false);

        Ok(CategoryResponse {
            id: category.id,
            requested_locale: projection.requested_locale.clone(),
            locale: projection.requested_locale,
            effective_locale: projection.effective_locale,
            available_locales: projection.available_locales,
            name: projection.name,
            slug: projection.slug,
            description: projection.description,
            icon: projection.icon_key,
            color: projection.color,
            parent_id,
            position: projection.position,
            topic_count: category.topic_count,
            reply_count: category.reply_count,
            moderated: category.moderated,
            is_subscribed,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::services) async fn list_paginated_with_locale_fallback_and_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;

        let mut query = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(
                forum_category::Column::Id
                    .not_in_subquery(archived_category_ids_subquery(tenant_id)),
            );
        if !hidden_category_ids.is_empty() {
            query =
                query.filter(forum_category::Column::Id.is_not_in(hidden_category_ids.to_vec()));
        }

        let paginator = query
            .order_by_asc(forum_category::Column::Position)
            .paginate(&self.db, per_page.max(1));
        let total = paginator.num_items().await?;
        let categories = paginator.fetch_page(page.saturating_sub(1)).await?;
        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        if category_ids.is_empty() {
            return Ok((Vec::new(), total));
        }

        let bindings = forum_category_taxonomy_binding::Entity::find()
            .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .filter(
                forum_category_taxonomy_binding::Column::ForumCategoryId
                    .is_in(category_ids.clone()),
            )
            .all(&self.db)
            .await?;
        let binding_by_forum_id = bindings
            .iter()
            .map(|binding| (binding.forum_category_id, binding.taxonomy_category_id))
            .collect::<HashMap<_, _>>();
        for category_id in &category_ids {
            if !binding_by_forum_id.contains_key(category_id) {
                return Err(missing_binding(*category_id));
            }
        }

        let taxonomy_ids = bindings
            .iter()
            .map(|binding| binding.taxonomy_category_id)
            .collect::<Vec<_>>();
        let projections = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some("forum"),
                Some(&taxonomy_ids),
                locale,
                fallback_locale,
            )
            .await
            .map_err(map_taxonomy_read_error)?;
        let projection_by_taxonomy_id = projections
            .into_iter()
            .map(|projection| (projection.id, projection))
            .collect::<HashMap<_, _>>();

        let subscription_flags = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &category_ids, security.user_id)
            .await?;
        let mut items = Vec::with_capacity(categories.len());
        for category in categories {
            let taxonomy_id = *binding_by_forum_id
                .get(&category.id)
                .ok_or_else(|| missing_binding(category.id))?;
            let projection = projection_by_taxonomy_id
                .get(&taxonomy_id)
                .ok_or_else(|| missing_projection(category.id, taxonomy_id))?;

            items.push(CategoryListItem {
                id: category.id,
                requested_locale: projection.requested_locale.clone(),
                locale: projection.requested_locale.clone(),
                effective_locale: projection.effective_locale.clone(),
                available_locales: projection.available_locales.clone(),
                name: projection.name.clone(),
                slug: projection.slug.clone(),
                description: projection.description.clone(),
                icon: projection.icon_key.clone(),
                color: projection.color.clone(),
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

    async fn forum_parent_id_for_taxonomy_parent(
        &self,
        tenant_id: Uuid,
        taxonomy_parent_id: Option<Uuid>,
    ) -> ForumResult<Option<Uuid>> {
        let Some(taxonomy_parent_id) = taxonomy_parent_id else {
            return Ok(None);
        };
        let binding = forum_category_taxonomy_binding::Entity::find()
            .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .filter(
                forum_category_taxonomy_binding::Column::TaxonomyCategoryId.eq(taxonomy_parent_id),
            )
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                ForumError::Validation(format!(
                    "Taxonomy parent Category {taxonomy_parent_id} has no Forum category binding"
                ))
            })?;
        Ok(Some(binding.forum_category_id))
    }
}

fn archived_category_ids_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(forum_category_lifecycle::Column::CategoryId)
        .from(forum_category_lifecycle::Entity)
        .and_where(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .to_owned()
}

fn missing_binding(category_id: Uuid) -> ForumError {
    ForumError::Validation(format!(
        "Forum category {category_id} has no Taxonomy Category binding"
    ))
}

fn missing_projection(category_id: Uuid, taxonomy_category_id: Uuid) -> ForumError {
    ForumError::Validation(format!(
        "Forum category {category_id} Taxonomy Category {taxonomy_category_id} projection is missing"
    ))
}

fn map_taxonomy_read_error(error: TaxonomyError) -> ForumError {
    match error {
        TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Forum Taxonomy category read projection failed: {other}"
        )),
    }
}
