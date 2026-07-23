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
    CategorySubtreeLifecycleResponse, MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES,
};
use crate::entities::{forum_category, forum_category_lifecycle};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

pub(super) struct CategoryLifecycleService {
    db: DatabaseConnection,
}

impl CategoryLifecycleService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
        lock_category_tree_in_tx(&txn, tenant_id).await?;

        let categories = load_categories_in_tx(&txn, tenant_id).await?;
        let models = categories
            .iter()
            .cloned()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        let root = models
            .get(&root_id)
            .cloned()
            .ok_or(ForumError::CategoryNotFound(root_id))?;
        validate_parent_map(&models)?;

        let lifecycle_rows = forum_category_lifecycle::Entity::find()
            .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        let lifecycle_by_category = lifecycle_rows
            .into_iter()
            .map(|lifecycle| (lifecycle.category_id, lifecycle))
            .collect::<HashMap<_, _>>();

        let affected_category_ids = collect_subtree_ids(&categories, root_id)?;
        if !archived {
            ensure_restore_ancestors_are_active(&models, &lifecycle_by_category, &root)?;
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
            "Forum category lifecycle does not support {backend:?}"
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

fn collect_subtree_ids(
    categories: &[forum_category::Model],
    root_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    let mut children_by_parent = HashMap::<Uuid, Vec<Uuid>>::new();
    for category in categories {
        if let Some(parent_id) = category.parent_id {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(category.id);
        }
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
    models: &HashMap<Uuid, forum_category::Model>,
    lifecycle_by_category: &HashMap<Uuid, forum_category_lifecycle::Model>,
    root: &forum_category::Model,
) -> ForumResult<()> {
    let mut parent_id = root.parent_id;
    while let Some(current_id) = parent_id {
        let parent = models.get(&current_id).ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category tree references missing or foreign parent {current_id}"
            ))
        })?;
        if lifecycle_by_category.contains_key(&current_id) {
            return Err(ForumError::Validation(
                "Category subtree cannot be restored beneath an archived ancestor".to_string(),
            ));
        }
        parent_id = parent.parent_id;
    }
    Ok(())
}

fn validate_parent_map(models: &HashMap<Uuid, forum_category::Model>) -> ForumResult<()> {
    for category_id in models.keys().copied() {
        let mut current_id = category_id;
        let mut depth = 0usize;
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current_id) {
                return Err(ForumError::Validation(
                    "Forum category hierarchy cycle".to_string(),
                ));
            }
            let category = models.get(&current_id).ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category tree references missing category {current_id}"
                ))
            })?;
            let Some(parent_id) = category.parent_id else {
                break;
            };
            if !models.contains_key(&parent_id) {
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
