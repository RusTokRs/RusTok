use std::{collections::HashMap, sync::Arc};

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, TenantLocale};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    TaxonomyCategoryDeleteCleanupPort, TaxonomyOwnerCategoryReader, TaxonomyScopeType,
    TaxonomyService,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use super::category::CategoryService as CategoryCommandCore;
use super::category_delete::BlogCategoryDeleteCleanup;
use super::rbac::enforce_scope;
use crate::dto::{
    CategoryListItem, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    MAX_BLOG_CATEGORY_TREE_NODES, UpdateCategoryInput,
};
use crate::entities::blog_category;
use crate::error::{BlogError, BlogResult};

const BLOG_TAXONOMY_SCOPE: &str = "blog";

/// Public Blog Category owner facade.
///
/// Blog keeps membership, settings, timestamps and command authorization. Canonical
/// Category identity/copy/hierarchy is owned by Taxonomy through the typed binding;
/// neither public reads nor command writes depend on the retired Blog translation mirror.
pub struct CategoryService {
    commands: CategoryCommandCore,
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    category_delete_cleanup: Option<Arc<dyn TaxonomyCategoryDeleteCleanupPort>>,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            commands: CategoryCommandCore::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
            category_delete_cleanup: None,
        }
    }

    /// Attach the host-owned cleanup required before a canonical Taxonomy Category is deleted.
    ///
    /// Delete fails closed when this port is absent; reads and other writes do not require it.
    pub fn with_category_delete_cleanup(
        mut self,
        cleanup: Arc<dyn TaxonomyCategoryDeleteCleanupPort>,
    ) -> Self {
        self.category_delete_cleanup = Some(cleanup);
        self
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> BlogResult<Uuid> {
        self.commands.create(tenant_id, security, input).await
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<CategoryResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::Read)?;
        self.load_category_response(tenant_id, category_id, locale)
            .await
    }

    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> BlogResult<CategoryResponse> {
        let response_locale = input.locale.clone();
        self.commands
            .update(tenant_id, category_id, security, input)
            .await?;
        self.load_category_response(tenant_id, category_id, &response_locale)
            .await
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogCategories, Action::Delete)?;
        let capability_cleanup = self.category_delete_cleanup.clone().ok_or_else(|| {
            BlogError::invariant(
                "Blog Category delete requires host-composed Taxonomy capability cleanup",
            )
        })?;
        let cleanup = BlogCategoryDeleteCleanup::new(
            category_id,
            security.user_id,
            self.event_bus.clone(),
            capability_cleanup,
        );
        TaxonomyService::new(self.db.clone())
            .delete_module_category_with_cleanup(
                tenant_id,
                category_id,
                BLOG_TAXONOMY_SCOPE,
                &cleanup,
            )
            .await
            .map_err(BlogError::from)
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListCategoriesFilter,
    ) -> BlogResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::BlogCategories, Action::List)?;
        let locale =
            normalize_locale(filter.locale.as_deref().unwrap_or(PLATFORM_FALLBACK_LOCALE))?;
        let page = filter.page.max(1);
        let per_page = filter.per_page.clamp(1, 100);

        let categories = blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(blog_category::Column::Id)
            .limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)
            .all(&self.db)
            .await?;
        if categories.len() > MAX_BLOG_CATEGORY_TREE_NODES as usize {
            return Err(BlogError::invariant(format!(
                "Persisted Blog category tree exceeds the bounded limit of {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
            )));
        }
        if categories.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();

        let canonical = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some(BLOG_TAXONOMY_SCOPE),
                Some(&category_ids),
                &locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;
        if canonical.len() != categories.len() {
            return Err(BlogError::invariant(
                "Blog Category Taxonomy projection coverage is incomplete",
            ));
        }

        let canonical_by_id = canonical
            .into_iter()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        if canonical_by_id.len() != categories.len() {
            return Err(BlogError::invariant(
                "Blog Category Taxonomy projection contains duplicate identities",
            ));
        }

        let mut rows = categories
            .into_iter()
            .map(|category| {
                let canonical = canonical_by_id.get(&category.id).ok_or_else(|| {
                    BlogError::invariant(format!(
                        "Blog category {} Taxonomy projection is missing",
                        category.id
                    ))
                })?;
                let parent_id = canonical.parent_id;
                Ok((category, canonical.clone(), parent_id))
            })
            .collect::<BlogResult<Vec<_>>>()?;
        rows.sort_by(|(left_blog, left, _), (right_blog, right, _)| {
            left.position
                .cmp(&right.position)
                .then_with(|| left_blog.id.cmp(&right_blog.id))
        });

        let total = rows.len() as u64;
        let offset = page.saturating_sub(1).saturating_mul(per_page);
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let items = rows
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .map(|(category, canonical, parent_id)| CategoryListItem {
                id: category.id,
                locale: canonical.requested_locale,
                effective_locale: canonical.effective_locale,
                name: canonical.name,
                slug: canonical.slug,
                parent_id,
                position: canonical.position,
                settings: category.settings,
                created_at: category.created_at.into(),
            })
            .collect();

        Ok((items, total))
    }

    async fn load_category_response(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<CategoryResponse> {
        let locale = normalize_locale(locale)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        let taxonomy_ids = [category_id];
        let canonical = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some(BLOG_TAXONOMY_SCOPE),
                Some(&taxonomy_ids),
                &locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog category {category_id} points to a missing Taxonomy Category"
                ))
            })?;
        let parent_id = canonical.parent_id;

        Ok(CategoryResponse {
            id: category.id,
            tenant_id,
            locale: canonical.requested_locale,
            effective_locale: canonical.effective_locale,
            available_locales: canonical.available_locales,
            name: canonical.name,
            slug: canonical.slug,
            description: canonical.description,
            parent_id,
            position: canonical.position,
            settings: category.settings,
            created_at: category.created_at.into(),
            updated_at: category.updated_at.into(),
        })
    }
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|_| BlogError::validation("Invalid locale"))
}
