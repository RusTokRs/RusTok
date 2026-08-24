/// Transactional category placement owner with full Forum projection invalidation.
pub(super) struct CategoryCommandProjectionOwnerService {
    db: DatabaseConnection,
}

impl CategoryCommandProjectionOwnerService {
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
                ForumError::Validation(
                    "Moved category was not persisted in sibling order".to_string(),
                )
            })?;

        let mut mirrored = HashSet::new();
        for placement in &updated {
            if mirrored.insert(placement.id) {
                super::category::taxonomy_sync::sync_category_any_locale_in_tx(
                    &txn,
                    tenant_id,
                    placement.id,
                )
                .await?;
            }
        }
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
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
        let parent_by_id = models
            .values()
            .map(|category| (category.id, category.parent_id))
            .collect::<HashMap<_, _>>();
        validate_parent_map(&parent_by_id)?;

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
        for placement in &siblings {
            super::category::taxonomy_sync::sync_category_any_locale_in_tx(
                &txn,
                tenant_id,
                placement.id,
            )
            .await?;
        }
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        Ok(ReorderCategorySiblingsResponse {
            parent_id: input.parent_id,
            siblings,
        })
    }
}
