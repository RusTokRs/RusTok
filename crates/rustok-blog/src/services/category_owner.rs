use std::collections::HashMap;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, TenantLocale};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyOwnerCategory, TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use super::category::{
    ApplyExactCategoryTranslationInput, CategoryService as LegacyCategoryService,
    CategoryTranslationApplyResult,
};
use super::rbac::enforce_scope;
use crate::dto::{
    CategoryListItem, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    MAX_BLOG_CATEGORY_TREE_NODES, UpdateCategoryInput,
};
use crate::entities::{blog_category, blog_category_taxonomy_binding};
use crate::error::{BlogError, BlogResult};

const BLOG_TAXONOMY_SCOPE: &str = "blog";

/// Public Blog Category owner facade.
///
/// Blog keeps membership, settings, timestamps and domain mutations. Canonical
/// Category identity/copy/hierarchy is materialized through the typed Taxonomy
/// binding so public reads no longer depend on `blog_category_translations` or
/// Blog-owned placement columns.
pub struct CategoryService {
    legacy: LegacyCategoryService,
    db: DatabaseConnection,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            legacy: LegacyCategoryService::new(db.clone(), event_bus),
            db,
        }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> BlogResult<Uuid> {
        self.legacy.create(tenant_id, security, input).await
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<CategoryResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        let binding = load_binding(&self.db, tenant_id, category_id).await?;
        let taxonomy_ids = [binding.taxonomy_category_id];
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
                BlogError::validation(format!(
                    "Blog category {category_id} binding points to a missing Taxonomy Category"
                ))
            })?;
        let parent_id = resolve_parent_binding(&self.db, tenant_id, canonical.parent_id).await?;

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

    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> BlogResult<CategoryResponse> {
        self.legacy
            .update(tenant_id, category_id, security, input)
            .await
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        self.legacy.delete(tenant_id, category_id, security).await
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListCategoriesFilter,
    ) -> BlogResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::BlogCategories, Action::List)?;
        let locale = normalize_locale(
            filter
                .locale
                .as_deref()
                .unwrap_or(PLATFORM_FALLBACK_LOCALE),
        )?;
        let page = filter.page.max(1);
        let per_page = filter.per_page.clamp(1, 100);

        let categories = blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(blog_category::Column::Id)
            .limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)
            .all(&self.db)
            .await?;
        if categories.len() > MAX_BLOG_CATEGORY_TREE_NODES as usize {
            return Err(BlogError::validation(format!(
                "Blog category tree exceeds the bounded limit of {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
            )));
        }
        if categories.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let category_ids = categories.iter().map(|category| category.id).collect::<Vec<_>>();
        let bindings = blog_category_taxonomy_binding::Entity::find()
            .filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .filter(blog_category_taxonomy_binding::Column::BlogCategoryId.is_in(category_ids))
            .all(&self.db)
            .await?;
        if bindings.len() != categories.len() {
            return Err(BlogError::validation(
                "Blog Category Taxonomy binding coverage is incomplete",
            ));
        }

        let taxonomy_ids = bindings
            .iter()
            .map(|binding| binding.taxonomy_category_id)
            .collect::<Vec<_>>();
        let canonical = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some(BLOG_TAXONOMY_SCOPE),
                Some(&taxonomy_ids),
                &locale,
                Some(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;
        if canonical.len() != categories.len() {
            return Err(BlogError::validation(
                "Blog Category Taxonomy projection coverage is incomplete",
            ));
        }

        let binding_by_blog = bindings
            .iter()
            .map(|binding| (binding.blog_category_id, binding.taxonomy_category_id))
            .collect::<HashMap<_, _>>();
        let blog_by_taxonomy = bindings
            .iter()
            .map(|binding| (binding.taxonomy_category_id, binding.blog_category_id))
            .collect::<HashMap<_, _>>();
        if binding_by_blog.len() != categories.len() || blog_by_taxonomy.len() != categories.len() {
            return Err(BlogError::validation(
                "Blog Category Taxonomy binding is not one-to-one",
            ));
        }
        let canonical_by_id = canonical
            .into_iter()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        if canonical_by_id.len() != categories.len() {
            return Err(BlogError::validation(
                "Blog Category Taxonomy projection contains duplicate identities",
            ));
        }

        let mut rows = categories
            .into_iter()
            .map(|category| {
                let taxonomy_id = binding_by_blog
                    .get(&category.id)
                    .copied()
                    .ok_or_else(|| {
                        BlogError::validation(format!(
                            "Blog category {} has no Taxonomy binding",
                            category.id
                        ))
                    })?;
                let canonical = canonical_by_id.get(&taxonomy_id).ok_or_else(|| {
                    BlogError::validation(format!(
                        "Blog category {} Taxonomy projection is missing",
                        category.id
                    ))
                })?;
                let parent_id = canonical
                    .parent_id
                    .map(|parent_taxonomy_id| {
                        blog_by_taxonomy
                            .get(&parent_taxonomy_id)
                            .copied()
                            .ok_or_else(|| {
                                BlogError::validation(format!(
                                    "Taxonomy Category {parent_taxonomy_id} parent has no Blog binding"
                                ))
                            })
                    })
                    .transpose()?;
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

    pub(crate) async fn apply_exact_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        input: ApplyExactCategoryTranslationInput,
    ) -> BlogResult<CategoryTranslationApplyResult> {
        self.legacy
            .apply_exact_translation_in_tx(txn, tenant_id, category_id, input)
            .await
    }
}

async fn load_binding(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<blog_category_taxonomy_binding::Model> {
    blog_category_taxonomy_binding::Entity::find_by_id((tenant_id, category_id))
        .one(db)
        .await?
        .ok_or_else(|| {
            BlogError::validation(format!(
                "Blog category {category_id} has no Taxonomy binding"
            ))
        })
}

async fn resolve_parent_binding(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    parent_taxonomy_id: Option<Uuid>,
) -> BlogResult<Option<Uuid>> {
    let Some(parent_taxonomy_id) = parent_taxonomy_id else {
        return Ok(None);
    };
    blog_category_taxonomy_binding::Entity::find()
        .filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
        .filter(
            blog_category_taxonomy_binding::Column::TaxonomyCategoryId.eq(parent_taxonomy_id),
        )
        .one(db)
        .await?
        .map(|binding| binding.blog_category_id)
        .ok_or_else(|| {
            BlogError::validation(format!(
                "Taxonomy Category {parent_taxonomy_id} parent has no Blog binding"
            ))
        })
        .map(Some)
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|_| BlogError::validation("Invalid locale"))
}
