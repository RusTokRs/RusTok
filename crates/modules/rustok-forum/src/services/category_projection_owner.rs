use crate::entities::forum_category_lifecycle;

/// Transactional owner facade for canonical Forum Category mutations.
///
/// Forum owns membership, policy/counters and command authorization; Taxonomy
/// owns canonical localized copy, routes, hierarchy and presentation. Public
/// reads are composed separately through Taxonomy-backed read adapters.
pub(super) struct CategoryProjectionOwnerService {
    db: DatabaseConnection,
}

impl CategoryProjectionOwnerService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub(super) async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> ForumResult<Uuid> {
        enforce_scope(&security, Resource::ForumCategories, Action::Create)?;
        validate_category_name(&input.name)?;
        let locale = normalize_locale(&input.locale)?;
        let slug = normalize_required_slug(&input.slug)?;
        let canonical_name = input.name.clone();
        let canonical_description = input.description.clone();
        let requested_position = input.position.unwrap_or(0);
        if requested_position < 0 {
            return Err(ForumError::Validation(
                "Category position cannot be negative".to_string(),
            ));
        }

        let now = Utc::now();
        let id = Uuid::new_v4();
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;

        if let Some(parent_id) = input.parent_id {
            CategoryService::find_category_in_tx(&txn, tenant_id, parent_id).await?;
            if forum_category_lifecycle::Entity::find()
                .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
                .filter(forum_category_lifecycle::Column::CategoryId.eq(parent_id))
                .one(&txn)
                .await?
                .is_some()
            {
                return Err(ForumError::Validation(
                    "active forum category cannot have archived parent".to_string(),
                ));
            }
        }

        shift_siblings_for_insert_in_tx(&txn, tenant_id, input.parent_id, requested_position)
            .await?;

        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            moderated: Set(input.moderated),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            id,
            input.parent_id,
            requested_position,
            input.icon,
            input.color,
            locale,
            canonical_name,
            slug,
            canonical_description,
        )
        .await?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        Ok(id)
    }

    #[instrument(skip(self, security, input))]
    pub(super) async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumCategories, Action::Update)?;
        if input.position.is_some() {
            return Err(ForumError::Validation(
                "Category position must be changed through move/reorder commands".to_string(),
            ));
        }
        let locale = normalize_locale(&input.locale)?;
        let requested_name = input.name.clone();
        let requested_slug = input.slug.clone();
        let requested_description = input.description.clone();
        let txn = self.db.begin().await?;
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        let mut active: forum_category::ActiveModel = category.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(moderated) = input.moderated {
            active.moderated = Set(moderated);
        }
        active.update(&txn).await?;

        let existing_placement =
            rustok_taxonomy::entities::taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, category_id))
                .one(&txn)
                .await?;
        let (parent_id, position) = match existing_placement {
            Some(p) => (p.parent_term_id, p.position),
            None => (None, 0),
        };
        let existing_presentation =
            rustok_taxonomy::entities::taxonomy_category_presentation::Entity::find_by_id((tenant_id, category_id))
                .one(&txn)
                .await?;
        let (icon, color) = match existing_presentation {
            Some(p) => (input.icon.or(p.icon_key), input.color.or(p.color)),
            None => (input.icon, input.color),
        };

        let existing_canonical =
            taxonomy_sync::load_category_locale_copy_in_tx(&txn, tenant_id, category_id, &locale)
                .await?;
        let (canonical_name, canonical_slug, canonical_description) = match existing_canonical {
            Some(existing) => {
                let name = requested_name.clone().unwrap_or(existing.name);
                validate_category_name(&name)?;
                let slug = match requested_slug.as_deref() {
                    Some(slug) => normalize_required_slug(slug)?,
                    None if requested_name.is_some() => normalize_required_slug(&name)?,
                    None => normalize_required_slug(&existing.slug)?,
                };
                let description = if requested_description.is_some() {
                    requested_description.clone()
                } else {
                    existing.description
                };
                (name, slug, description)
            }
            None => {
                let name = requested_name.clone().ok_or_else(|| {
                    ForumError::Validation("Category name is required".to_string())
                })?;
                validate_category_name(&name)?;
                let slug = requested_slug
                    .as_deref()
                    .map(normalize_required_slug)
                    .transpose()?
                    .unwrap_or_else(|| normalize_slug(&name));
                let slug = normalize_required_slug(&slug)?;
                (name, slug, requested_description.clone())
            }
        };

        taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            category_id,
            parent_id,
            position,
            icon,
            color,
            locale,
            canonical_name,
            canonical_slug,
            canonical_description,
        )
        .await?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }
}

pub(super) mod taxonomy_sync {
    include!("category_taxonomy_sync.rs");
}
