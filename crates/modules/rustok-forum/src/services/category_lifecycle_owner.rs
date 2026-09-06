/// Transactional category lifecycle owner with inherited projection invalidation.
pub(super) struct CategoryLifecycleProjectionOwnerService {
    db: DatabaseConnection,
}

impl CategoryLifecycleProjectionOwnerService {
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
