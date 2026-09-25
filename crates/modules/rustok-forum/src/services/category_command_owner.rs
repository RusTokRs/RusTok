use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{
    CategoryPlacementResponse, MAX_FORUM_CATEGORY_TREE_NODES, MoveCategoryInput, MoveCategoryResponse,
    ReorderCategorySiblingsInput, ReorderCategorySiblingsResponse,
};
use crate::entities::forum_category_lifecycle;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

pub(super) struct CategoryCommandProjectionOwnerService {
    db: DatabaseConnection,
}

impl CategoryCommandProjectionOwnerService {
    pub(super) fn new(db: DatabaseConnection) -> Self { Self { db } }

    pub(super) async fn move_category(
        &self, tenant_id: Uuid, category_id: Uuid, security: SecurityContext, input: MoveCategoryInput
    ) -> ForumResult<MoveCategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let txn = self.db.begin().await?;
        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id)
            .await
            .map_err(super::category::taxonomy_sync::map_taxonomy_error)?;

        let categories = super::category::CategoryService::load_categories_in_tx(&txn, tenant_id).await?;
        let models = categories.iter().cloned().map(|c| (c.id, c)).collect::<HashMap<_, _>>();
        if !models.contains_key(&category_id) {
            return Err(ForumError::CategoryNotFound(category_id));
        }
        if let Some(parent_id) = input.parent_id && !models.contains_key(&parent_id) {
            return Err(ForumError::Validation(format!("Category parent {parent_id} does not exist in the tenant")));
        }
        if let Some(parent_id) = input.parent_id {
            let parent_archived = forum_category_lifecycle::Entity::find()
                .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
                .filter(forum_category_lifecycle::Column::CategoryId.eq(parent_id))
                .one(&txn)
                .await?
                .is_some();
            if parent_archived {
                let category_archived = forum_category_lifecycle::Entity::find()
                    .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
                    .filter(forum_category_lifecycle::Column::CategoryId.eq(category_id))
                    .one(&txn)
                    .await?
                    .is_some();
                if !category_archived {
                    return Err(ForumError::Validation("active forum category cannot have archived parent".to_string()));
                }
            }
        }
        let updated = super::category::taxonomy_sync::move_category_in_tx(
            &txn, tenant_id, category_id, input.parent_id, input.position
        ).await?;
        let moved = updated.iter().find(|p| p.id == category_id).cloned().ok_or_else(||
            ForumError::Validation("Moved category was not persisted in sibling order".to_string()))?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn, tenant_id, security.user_id
        ).await?;
        txn.commit().await?;
        Ok(MoveCategoryResponse { moved, updated })
    }

    pub(super) async fn reorder_siblings(
        &self, tenant_id: Uuid, security: SecurityContext, input: ReorderCategorySiblingsInput
    ) -> ForumResult<ReorderCategorySiblingsResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        if input.ordered_category_ids.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Category sibling order exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES}"
            )));
        }
        let txn = self.db.begin().await?;
        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id)
            .await
            .map_err(super::category::taxonomy_sync::map_taxonomy_error)?;
        let categories = super::category::CategoryService::load_categories_in_tx(&txn, tenant_id).await?;
        let models = categories.iter().cloned().map(|c| (c.id, c)).collect::<HashMap<_, _>>();
        if let Some(parent_id) = input.parent_id && !models.contains_key(&parent_id) {
            return Err(ForumError::Validation(format!("Category parent {parent_id} does not exist in the tenant")));
        }
        for id in &input.ordered_category_ids {
            if !models.contains_key(id) {
                return Err(ForumError::CategoryNotFound(*id));
            }
        }
        let current = super::category::taxonomy_sync::load_category_siblings_in_tx(
            &txn, tenant_id, input.parent_id
        ).await?;
        let current_set = current.iter().copied().collect::<HashSet<_>>();
        let requested = input.ordered_category_ids;
        let requested_set = requested.iter().copied().collect::<HashSet<_>>();
        if requested_set.len() != requested.len() {
            return Err(ForumError::Validation("Category sibling order contains duplicate category ids".to_string()));
        }
        if requested_set != current_set || requested.len() != current.len() {
            return Err(ForumError::Validation("Category sibling order must contain every direct child exactly once".to_string()));
        }
        super::category::taxonomy_sync::reorder_category_siblings_in_tx(
            &txn, tenant_id, input.parent_id, &requested
        ).await?;
        let siblings = requested.iter().copied().enumerate().map(|(position, id)| {
            Ok::<_, ForumError>(CategoryPlacementResponse {
                id,
                parent_id: input.parent_id,
                position: i32::try_from(position).map_err(|_|
                    ForumError::Validation("Category sibling position exceeds i32 range".to_string())
                )?,
            })
        }).collect::<Result<Vec<_>, _>>()?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn, tenant_id, security.user_id
        ).await?;
        txn.commit().await?;
        Ok(ReorderCategorySiblingsResponse { parent_id: input.parent_id, siblings })
    }
}