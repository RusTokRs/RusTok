use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{
    CategoryPlacementResponse, MoveCategoryInput, MoveCategoryResponse,
    ReorderCategorySiblingsInput, ReorderCategorySiblingsResponse,
    MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES,
};
use crate::entities::forum_category;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

pub(super) struct CategoryCommandService {
    db: DatabaseConnection,
}

impl CategoryCommandService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(super) async fn move_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: MoveCategoryInput,
    ) -> ForumResult<MoveCategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;

        let categories = load_categories_in_tx(&txn, tenant_id).await?;
        let models = categories
            .iter()
            .cloned()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        let category = models
            .get(&category_id)
            .cloned()
            .ok_or(ForumError::CategoryNotFound(category_id))?;
        ensure_parent_exists(&models, input.parent_id)?;

        let mut parent_by_id = models
            .values()
            .map(|category| (category.id, category.parent_id))
            .collect::<HashMap<_, _>>();
        validate_parent_map(&parent_by_id)?;
        parent_by_id.insert(category_id, input.parent_id);
        validate_parent_map(&parent_by_id)?;

        let source_parent_id = category.parent_id;
        let target_index = input.position as usize;
        let updated = if source_parent_id == input.parent_id {
            let mut siblings = sibling_ids(&categories, source_parent_id, Some(category_id));
            if target_index > siblings.len() {
                return Err(ForumError::Validation(format!(
                    "Category position {} exceeds sibling count {}",
                    input.position,
                    siblings.len()
                )));
            }
            siblings.insert(target_index, category_id);
            persist_sibling_order(&txn, &models, source_parent_id, &siblings).await?
        } else {
            let source_siblings = sibling_ids(&categories, source_parent_id, Some(category_id));
            let mut target_siblings = sibling_ids(&categories, input.parent_id, None);
            if target_index > target_siblings.len() {
                return Err(ForumError::Validation(format!(
                    "Category position {} exceeds destination sibling count {}",
                    input.position,
                    target_siblings.len()
                )));
            }
            target_siblings.insert(target_index, category_id);

            let mut updated =
                persist_sibling_order(&txn, &models, source_parent_id, &source_siblings).await?;
            updated.extend(
                persist_sibling_order(&txn, &models, input.parent_id, &target_siblings).await?,
            );
            updated
        };

        let moved = updated
            .iter()
            .find(|placement| placement.id == category_id)
            .cloned()
            .ok_or_else(|| {
                ForumError::Validation("Moved category was not persisted in sibling order".to_string())
            })?;

        txn.commit().await?;
        Ok(MoveCategoryResponse { moved, updated })
    }

    pub(super) async fn reorder_siblings(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: ReorderCategorySiblingsInput,
    ) -> ForumResult<ReorderCategorySiblingsResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        if input.ordered_category_ids.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Category sibling order exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES}"
            )));
        }

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let categories = load_categories_in_tx(&txn, tenant_id).await?;
        let models = categories
            .iter()
            .cloned()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        ensure_parent_exists(&models, input.parent_id)?;
        validate_parent_map(
            &models
                .values()
                .map(|category| (category.id, category.parent_id))
                .collect(),
        )?;

        let current = sibling_ids(&categories, input.parent_id, None);
        let requested = input.ordered_category_ids;
        let requested_set = requested.iter().copied().collect::<HashSet<_>>();
        let current_set = current.iter().copied().collect::<HashSet<_>>();
        if requested_set.len() != requested.len() {
            return Err(ForumError::Validation(
                "Category sibling order contains duplicate category ids".to_string(),
            ));
        }
        if requested_set != current_set || requested.len() != current.len() {
            return Err(ForumError::Validation(
                "Category sibling order must contain every direct child exactly once".to_string(),
            ));
        }

        let siblings = persist_sibling_order(&txn, &models, input.parent_id, &requested).await?;
        txn.commit().await?;
        Ok(ReorderCategorySiblingsResponse {
            parent_id: input.parent_id,
            siblings,
        })
    }
}

pub(super) async fn validate_new_category_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
) -> ForumResult<()> {
    lock_category_tree_in_tx(txn, tenant_id).await?;
    let categories = load_categories_in_tx(txn, tenant_id).await?;
    let mut parent_by_id = categories
        .into_iter()
        .map(|category| (category.id, category.parent_id))
        .collect::<HashMap<_, _>>();
    parent_by_id.insert(category_id, parent_id);
    validate_parent_map(&parent_by_id)
}

pub(super) async fn lock_category_tree_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
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
            "Forum category commands do not support {backend:?}"
        ))),
    }
}

async fn load_categories_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<Vec<forum_category::Model>> {
    let categories = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(forum_category::Column::Position)
        .order_by_asc(forum_category::Column::Id)
        .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
        .all(txn)
        .await?;
    if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum category tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
        )));
    }
    Ok(categories)
}

fn ensure_parent_exists(
    models: &HashMap<Uuid, forum_category::Model>,
    parent_id: Option<Uuid>,
) -> ForumResult<()> {
    if let Some(parent_id) = parent_id {
        if !models.contains_key(&parent_id) {
            return Err(ForumError::Validation(format!(
                "Category parent {parent_id} does not exist in the tenant"
            )));
        }
    }
    Ok(())
}

fn sibling_ids(
    categories: &[forum_category::Model],
    parent_id: Option<Uuid>,
    excluded_id: Option<Uuid>,
) -> Vec<Uuid> {
    categories
        .iter()
        .filter(|category| category.parent_id == parent_id && Some(category.id) != excluded_id)
        .map(|category| category.id)
        .collect()
}

async fn persist_sibling_order(
    txn: &DatabaseTransaction,
    models: &HashMap<Uuid, forum_category::Model>,
    parent_id: Option<Uuid>,
    ordered_ids: &[Uuid],
) -> ForumResult<Vec<CategoryPlacementResponse>> {
    let mut placements = Vec::with_capacity(ordered_ids.len());
    for (position, category_id) in ordered_ids.iter().copied().enumerate() {
        let category = models
            .get(&category_id)
            .cloned()
            .ok_or(ForumError::CategoryNotFound(category_id))?;
        let position = i32::try_from(position).map_err(|_| {
            ForumError::Validation("Category sibling position exceeds i32 range".to_string())
        })?;
        let mut active: forum_category::ActiveModel = category.into();
        active.parent_id = Set(parent_id);
        active.position = Set(position);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        placements.push(CategoryPlacementResponse {
            id: category_id,
            parent_id,
            position,
        });
    }
    Ok(placements)
}

fn validate_parent_map(parent_by_id: &HashMap<Uuid, Option<Uuid>>) -> ForumResult<()> {
    if parent_by_id.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum category tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
        )));
    }

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

#[cfg(test)]
mod tests {
    use super::validate_parent_map;
    use crate::MAX_FORUM_CATEGORY_TREE_DEPTH;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn rejects_cycles_and_excessive_depth() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut cycle = HashMap::from([(a, Some(b)), (b, Some(a))]);
        assert!(validate_parent_map(&cycle).is_err());

        cycle.clear();
        let mut parent = None;
        for _ in 0..=MAX_FORUM_CATEGORY_TREE_DEPTH + 1 {
            let id = Uuid::new_v4();
            cycle.insert(id, parent);
            parent = Some(id);
        }
        assert!(validate_parent_map(&cycle).is_err());
    }
}
