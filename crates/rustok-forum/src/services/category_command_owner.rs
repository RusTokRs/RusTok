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

/// Bounded owner command for the single Forum category-cover relation.
///
/// Media admission runs before the write transaction; the category is checked
/// before and inside the transaction so a slow owner call never holds Forum DB
/// locks and a concurrent category deletion still fails closed.
pub(super) struct CategoryCoverCommandService {
    db: DatabaseConnection,
}

impl CategoryCoverCommandService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(super) async fn set_cover(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        media_context: rustok_api::PortContext,
        media_port: Option<&dyn rustok_media::MediaPublicImageReadPort>,
        input: crate::dto::SetCategoryCoverInput,
    ) -> ForumResult<crate::dto::CategoryCoverResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;

        forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        if let Some(media_id) = input.media_id {
            crate::category_presentation::resolve_category_cover_for_write(
                media_port,
                media_context,
                media_id,
                None,
            )
            .await?;
        }

        let txn = self.db.begin().await?;
        forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        match input.media_id {
            Some(media_id) => {
                let existing = crate::entities::forum_category_cover::Entity::find_by_id(category_id)
                    .one(&txn)
                    .await?;
                match existing {
                    Some(existing) => {
                        if existing.tenant_id != tenant_id {
                            return Err(ForumError::Validation(
                                "Category cover relation belongs to another tenant".to_string(),
                            ));
                        }
                        let mut active: crate::entities::forum_category_cover::ActiveModel =
                            existing.into();
                        active.media_id = Set(media_id);
                        active.updated_at = Set(Utc::now().into());
                        active.update(&txn).await?;
                    }
                    None => {
                        crate::entities::forum_category_cover::ActiveModel {
                            category_id: Set(category_id),
                            tenant_id: Set(tenant_id),
                            media_id: Set(media_id),
                            updated_at: Set(Utc::now().into()),
                        }
                        .insert(&txn)
                        .await?;
                    }
                }
            }
            None => {
                crate::entities::forum_category_cover::Entity::delete_many()
                    .filter(
                        crate::entities::forum_category_cover::Column::TenantId.eq(tenant_id),
                    )
                    .filter(
                        crate::entities::forum_category_cover::Column::CategoryId.eq(category_id),
                    )
                    .exec(&txn)
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

        Ok(crate::dto::CategoryCoverResponse {
            category_id,
            media_id: input.media_id,
        })
    }
}
