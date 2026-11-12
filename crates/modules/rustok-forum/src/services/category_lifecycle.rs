/// Verify that a category and every ancestor are active.
///
/// Structural topic commands and restore use the same category-tree lifecycle invariant:
/// a child of an archived ancestor is not an active placement target even when its own
/// lifecycle row is absent.
pub(super) async fn ensure_category_tree_target_is_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let categories =
        super::category::CategoryService::load_categories_in_tx(txn, tenant_id).await?;
    let category_ids = categories
        .iter()
        .map(|category| category.id)
        .collect::<Vec<_>>();
    let parent_by_id = load_category_parents_in_tx(
        &crate::ports::taxonomy::NoopForumTaxonomyPort,
        txn,
        tenant_id,
        &category_ids,
    )
    .await?;
    validate_parent_map(&parent_by_id)?;

    let lifecycle_rows = forum_category_lifecycle::Entity::find()
        .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?;
    let lifecycle_by_category = lifecycle_rows
        .into_iter()
        .map(|lifecycle| (lifecycle.category_id, lifecycle))
        .collect::<HashMap<_, _>>();

    if !parent_by_id.contains_key(&category_id) || lifecycle_by_category.contains_key(&category_id)
    {
        return Err(ForumError::Validation(
            "Forum content cannot be placed in an archived category".to_string(),
        ));
    }

    ensure_restore_ancestors_are_active(&parent_by_id, &lifecycle_by_category, category_id)
}

/// Verify that a deleted topic/reply can be restored into its category tree.
pub(super) async fn ensure_category_restore_target_is_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    ensure_category_tree_target_is_active_in_tx(txn, tenant_id, category_id).await
}

async fn load_category_parents_in_tx(
    port: &dyn crate::ports::taxonomy::ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, Option<Uuid>>> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let projections = port
        .load_scoped_categories(
            txn.into(),
            tenant_id,
            Some(category_ids),
            rustok_api::PLATFORM_FALLBACK_LOCALE,
            Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
        )
        .await?;
    if projections.len() != category_ids.len() {
        return Err(ForumError::Validation(
            "Forum Category Taxonomy hierarchy coverage is incomplete".to_string(),
        ));
    }
    Ok(projections
        .into_iter()
        .map(|category| (category.id, category.parent_id))
        .collect())
}

fn collect_subtree_ids(
    parent_by_id: &HashMap<Uuid, Option<Uuid>>,
    root_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    let mut children_by_parent = HashMap::<Uuid, Vec<Uuid>>::new();
    for (category_id, parent_id) in parent_by_id {
        if let Some(parent_id) = parent_id {
            children_by_parent
                .entry(*parent_id)
                .or_default()
                .push(*category_id);
        }
    }
    for children in children_by_parent.values_mut() {
        children.sort();
    }

    let mut result = Vec::new();
    let mut stack = vec![root_id];
    let mut visited = HashSet::new();
    while let Some(category_id) = stack.pop() {
        if !visited.insert(category_id) {
            return Err(ForumError::Validation(
                "Forum category hierarchy cycle".to_string(),
            ));
        }
        result.push(category_id);
        if result.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category subtree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }
        if let Some(children) = children_by_parent.get(&category_id) {
            stack.extend(children.iter().rev().copied());
        }
    }
    Ok(result)
}

fn ensure_restore_ancestors_are_active(
    parent_by_id: &HashMap<Uuid, Option<Uuid>>,
    lifecycle_by_category: &HashMap<Uuid, forum_category_lifecycle::Model>,
    root_id: Uuid,
) -> ForumResult<()> {
    let mut parent_id = parent_by_id.get(&root_id).copied().flatten();
    while let Some(current_id) = parent_id {
        if !parent_by_id.contains_key(&current_id) {
            return Err(ForumError::Validation(format!(
                "Forum category tree references missing or foreign parent {current_id}"
            )));
        }
        if lifecycle_by_category.contains_key(&current_id) {
            return Err(ForumError::Validation(
                "Category subtree cannot be restored beneath an archived ancestor".to_string(),
            ));
        }
        parent_id = parent_by_id.get(&current_id).copied().flatten();
    }
    Ok(())
}

fn validate_parent_map(parent_by_id: &HashMap<Uuid, Option<Uuid>>) -> ForumResult<()> {
    for category_id in parent_by_id.keys().copied() {
        let mut current_id = category_id;
        let mut depth = 0usize;
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current_id) {
                return Err(ForumError::Validation(
                    "Forum category hierarchy cycle".to_string(),
                ));
            }
            let parent_id = parent_by_id.get(&current_id).ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category tree references missing category {current_id}"
                ))
            })?;
            let Some(parent_id) = *parent_id else {
                break;
            };
            if !parent_by_id.contains_key(&parent_id) {
                return Err(ForumError::Validation(format!(
                    "Forum category tree references missing or foreign parent {parent_id}"
                )));
            }
            depth += 1;
            if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
                return Err(ForumError::Validation(format!(
                    "Forum category tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
                )));
            }
            current_id = parent_id;
        }
    }
    Ok(())
}

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{
    CategorySubtreeLifecycleResponse, MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES,
};
use crate::entities::forum_category_lifecycle;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

/// Transactional category lifecycle owner with inherited projection invalidation.
pub(super) struct CategoryLifecycleProjectionOwnerService {
    db: DatabaseConnection,
    port: std::sync::Arc<dyn crate::ports::taxonomy::ForumTaxonomyPort>,
}

impl CategoryLifecycleProjectionOwnerService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            port: std::sync::Arc::new(crate::ports::taxonomy::NoopForumTaxonomyPort),
        }
    }

    pub(super) fn with_port(
        mut self,
        port: std::sync::Arc<dyn crate::ports::taxonomy::ForumTaxonomyPort>,
    ) -> Self {
        self.port = port;
        self
    }

    pub(super) async fn archive_subtree(
        &self,
        tenant_id: Uuid,
        root_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.set_subtree_archived(tenant_id, root_id, security, true, Action::Manage)
            .await
    }

    pub(super) async fn archive_subtree_for_delete(
        &self,
        tenant_id: Uuid,
        root_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.set_subtree_archived(tenant_id, root_id, security, true, Action::Delete)
            .await
    }

    pub(super) async fn restore_subtree(
        &self,
        tenant_id: Uuid,
        root_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.set_subtree_archived(tenant_id, root_id, security, false, Action::Manage)
            .await
    }

    async fn set_subtree_archived(
        &self,
        tenant_id: Uuid,
        root_id: Uuid,
        security: SecurityContext,
        archived: bool,
        required_action: Action,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        enforce_scope(&security, Resource::ForumCategories, required_action)?;
        let txn = self.db.begin().await?;
        self.port
            .lock_category_hierarchy_writer_in_tx(&txn, tenant_id)
            .await?;

        let categories =
            super::category::CategoryService::load_categories_in_tx(&txn, tenant_id).await?;
        let models = categories
            .iter()
            .cloned()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        if !models.contains_key(&root_id) {
            return Err(ForumError::CategoryNotFound(root_id));
        }

        let category_ids = categories.iter().map(|c| c.id).collect::<Vec<_>>();
        let parent_by_id =
            load_category_parents_in_tx(&*self.port, &txn, tenant_id, &category_ids).await?;
        validate_parent_map(&parent_by_id)?;

        let lifecycle_rows = forum_category_lifecycle::Entity::find()
            .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        let lifecycle_by_category = lifecycle_rows
            .into_iter()
            .map(|lifecycle| (lifecycle.category_id, lifecycle))
            .collect::<HashMap<_, _>>();

        let affected_category_ids = collect_subtree_ids(&parent_by_id, root_id)?;
        if !archived {
            ensure_restore_ancestors_are_active(&parent_by_id, &lifecycle_by_category, root_id)?;
        }

        let now = Utc::now();
        let mut update_ids = affected_category_ids.clone();
        if archived {
            update_ids.reverse();
        }

        let mut changed = HashSet::new();
        for category_id in update_ids {
            let is_archived = lifecycle_by_category.contains_key(&category_id);
            if is_archived == archived {
                continue;
            }

            if archived {
                forum_category_lifecycle::ActiveModel {
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    archived_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            } else {
                forum_category_lifecycle::Entity::delete_many()
                    .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
                    .filter(forum_category_lifecycle::Column::CategoryId.eq(category_id))
                    .exec(&txn)
                    .await?;
            }
            changed.insert(category_id);
        }

        if !changed.is_empty() {
            super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
                &txn,
                tenant_id,
                security.user_id,
            )
            .await?;
        }
        txn.commit().await?;

        let changed_category_ids = affected_category_ids
            .iter()
            .copied()
            .filter(|category_id| changed.contains(category_id))
            .collect::<Vec<_>>();
        let archived_at = if archived {
            lifecycle_by_category
                .get(&root_id)
                .map(|lifecycle| lifecycle.archived_at.to_rfc3339())
                .or_else(|| Some(now.to_rfc3339()))
        } else {
            None
        };

        Ok(CategorySubtreeLifecycleResponse {
            root_id,
            archived,
            archived_at,
            affected_count: affected_category_ids.len() as u32,
            changed_count: changed_category_ids.len() as u32,
            affected_category_ids,
            changed_category_ids,
        })
    }
}
