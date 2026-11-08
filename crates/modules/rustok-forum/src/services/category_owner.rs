use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{
    CategoryListItem, CategoryResponse, CategorySubtreeLifecycleResponse,
    CategoryTopicPolicyResponse, CategoryTreeQuery, CategoryTreeResponse, CreateCategoryInput,
    MAX_FORUM_READ_LIMIT, MoveCategoryInput, MoveCategoryResponse, ReorderCategorySiblingsInput,
    ReorderCategorySiblingsResponse, UpdateCategoryInput, UpdateCategoryTopicPolicyInput,
    bounded_forum_read_limit,
};
use crate::error::{ForumError, ForumResult};

use super::category_visibility::ForumCategoryVisibilityPolicyService;
use super::rbac::enforce_scope;
use super::{category, category_command, category_lifecycle, category_policy};

#[path = "category_taxonomy_tree_read.rs"]
mod taxonomy_tree_read;
use taxonomy_tree_read::CategoryTaxonomyTreeReadService;

/// Public owner facade for forum categories.
///
/// Only explicit domain operations are exposed. The raw persistence service is
/// kept crate-private so callers cannot bypass placement, lifecycle or policy
/// commands through `Deref`.
pub struct CategoryService {
    db: DatabaseConnection,
    inner: category::CategoryProjectionOwnerService,
    read: category::taxonomy_read::CategoryTaxonomyReadService,
    commands: category_command::CategoryCommandProjectionOwnerService,
    lifecycle: category_lifecycle::CategoryLifecycleProjectionOwnerService,
    policy: category_policy::CategoryTopicPolicyService,
    tree_read: CategoryTaxonomyTreeReadService,
    visibility: ForumCategoryVisibilityPolicyService,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db: db.clone(),
            inner: category::CategoryProjectionOwnerService::new(db.clone()),
            read: category::taxonomy_read::CategoryTaxonomyReadService::new(db.clone()),
            commands: category_command::CategoryCommandProjectionOwnerService::new(db.clone()),
            lifecycle: category_lifecycle::CategoryLifecycleProjectionOwnerService::new(db.clone()),
            policy: category_policy::CategoryTopicPolicyService::new(db.clone()),
            tree_read: CategoryTaxonomyTreeReadService::new(db.clone()),
            visibility: ForumCategoryVisibilityPolicyService::new(db),
        }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        let locale = input.locale.clone();
        let category_id = self
            .inner
            .create(tenant_id, security.clone(), input)
            .await?;
        self.get(tenant_id, security, category_id, &locale).await
    }

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

    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        if !self
            .visibility
            .is_category_visible_to_viewer(tenant_id, category_id, !security.is_public_read())
            .await?
        {
            return Err(ForumError::CategoryNotFound(category_id));
        }
        self.read
            .get_with_locale_fallback(tenant_id, security, category_id, locale, fallback_locale)
            .await
    }

    /// Archive the complete category subtree from normal product delete flows.
    /// Physical purge remains a separate retention concern.
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.lifecycle
            .archive_subtree_for_delete(tenant_id, category_id, security)
            .await?;
        Ok(())
    }

    pub async fn tree(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryTreeQuery,
    ) -> ForumResult<CategoryTreeResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let hidden_category_ids = self
            .visibility
            .hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())
            .await?;
        self.tree_read
            .read_with_hidden_categories(tenant_id, query, &hidden_category_ids, security.user_id)
            .await
    }

    pub async fn archive_subtree(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.lifecycle
            .archive_subtree(tenant_id, category_id, security)
            .await
    }

    pub async fn restore_subtree(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.lifecycle
            .restore_subtree(tenant_id, category_id, security)
            .await
    }

    pub async fn topic_policy(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategoryTopicPolicyResponse> {
        self.policy.get(tenant_id, category_id, security).await
    }

    pub async fn set_topic_policy(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryTopicPolicyInput,
    ) -> ForumResult<CategoryTopicPolicyResponse> {
        self.policy
            .set(tenant_id, category_id, security, input)
            .await
    }

    pub async fn move_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: MoveCategoryInput,
    ) -> ForumResult<MoveCategoryResponse> {
        self.commands
            .move_category(tenant_id, category_id, security, input)
            .await
    }

    pub async fn reorder_siblings(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: ReorderCategorySiblingsInput,
    ) -> ForumResult<ReorderCategorySiblingsResponse> {
        self.commands
            .reorder_siblings(tenant_id, security, input)
            .await
    }

    /// Update category metadata without bypassing the atomic placement commands.
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        if input.position.is_some() {
            return Err(ForumError::Validation(
                "Category position must be changed through move/reorder commands".to_string(),
            ));
        }
        let locale = input.locale.clone();
        self.inner
            .update(tenant_id, category_id, security.clone(), input)
            .await?;
        self.get(tenant_id, security, category_id, &locale).await
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .list_paginated_with_locale_fallback(
                tenant_id,
                security,
                locale,
                1,
                MAX_FORUM_READ_LIMIT,
                None,
            )
            .await?;
        Ok(items)
    }

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
                MAX_FORUM_READ_LIMIT,
                fallback_locale,
            )
            .await?;
        Ok(items)
    }

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
        let hidden_category_ids = self
            .visibility
            .hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())
            .await?;
        self.read
            .list_paginated_with_locale_fallback_and_hidden_categories(
                category::taxonomy_read::CategoryTaxonomyListFilter {
                    tenant_id,
                    security,
                    locale,
                    page,
                    per_page: bounded_forum_read_limit(Some(per_page)),
                    fallback_locale,
                    hidden_category_ids: &hidden_category_ids,
                },
            )
            .await
    }

    /// Keyset form of [`Self::list_paginated_with_locale_fallback`] for batch readers.
    pub async fn list_keyset_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        after: Option<Uuid>,
        limit: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<CategoryListItem>, Option<Uuid>)> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let hidden_category_ids = self
            .visibility
            .hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())
            .await?;
        self.read
            .list_keyset_with_locale_fallback_and_hidden_categories(
                category::taxonomy_read::CategoryTaxonomyKeysetFilter {
                    tenant_id,
                    security,
                    locale,
                    after,
                    limit: bounded_forum_read_limit(Some(limit)),
                    fallback_locale,
                    hidden_category_ids: &hidden_category_ids,
                },
            )
            .await
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        category::CategoryService::adjust_counters_in_tx(
            txn,
            tenant_id,
            category_id,
            topic_delta,
            reply_delta,
        )
        .await
    }
}

use rustok_taxonomy::{TaxonomyError, TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::forum_category;

impl CategoryService {
    pub const MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS: usize = 512;

    pub async fn available_locales_for_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        if security.is_public_read() {
            return Err(ForumError::forbidden(
                "Forum category locale enumeration requires an authenticated operator context",
            ));
        }
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum category locale enumeration requires a non-nil tenant id".to_string(),
            ));
        }
        if category_ids.len() > Self::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS {
            return Err(ForumError::Validation(format!(
                "Forum category locale enumeration is limited to {} category IDs",
                Self::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS
            )));
        }
        if category_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::BTreeSet::new();
        for category_id in category_ids {
            if category_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum category locale enumeration requires non-nil category IDs".to_string(),
                ));
            }
            if !seen.insert(*category_id) {
                return Err(ForumError::Validation(format!(
                    "Forum category locale enumeration repeats category {category_id}"
                )));
            }
        }

        let existing = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::Id.is_in(category_ids.to_vec()))
            .all(&self.db)
            .await?;
        let existing_ids = existing
            .into_iter()
            .map(|category| category.id)
            .collect::<std::collections::BTreeSet<_>>();
        for category_id in category_ids {
            if !existing_ids.contains(category_id) {
                return Err(ForumError::CategoryNotFound(*category_id));
            }
        }

        let projections = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some("forum"),
                Some(category_ids),
                rustok_api::PLATFORM_FALLBACK_LOCALE,
                None,
            )
            .await
            .map_err(|error| match error {
                TaxonomyError::Database(error) => ForumError::Database(error),
                other => ForumError::Validation(format!(
                    "Forum Taxonomy category locale enumeration failed: {other}"
                )),
            })?;
        let locales_by_id = projections
            .into_iter()
            .map(|projection| (projection.id, projection.available_locales))
            .collect::<std::collections::HashMap<_, _>>();

        let mut result = Vec::with_capacity(category_ids.len());
        for category_id in category_ids {
            let locales = locales_by_id.get(category_id).cloned().ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {category_id} Taxonomy Category projection is missing"
                ))
            })?;
            if locales.is_empty() {
                return Err(ForumError::Validation(format!(
                    "Forum category {category_id} has no Taxonomy-owned locale translation"
                )));
            }
            result.push((*category_id, locales));
        }

        Ok(result)
    }
}

