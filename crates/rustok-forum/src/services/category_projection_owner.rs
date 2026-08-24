use std::ops::Deref;

/// Transactional owner facade for category content mutations.
///
/// Read methods remain delegated to the established category service. Create,
/// update and compatibility delete reuse the same private validators and tree
/// helpers from this module while adding a full Forum projection invalidation
/// before commit because category copy and ancestry affect topic documents.
pub(super) struct CategoryProjectionOwnerService {
    inner: CategoryService,
    db: DatabaseConnection,
}

impl CategoryProjectionOwnerService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: CategoryService::new(db.clone()),
            db,
        }
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
        }

        shift_siblings_for_insert_in_tx(&txn, tenant_id, input.parent_id, requested_position, now)
            .await?;

        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            parent_id: Set(input.parent_id),
            position: Set(requested_position),
            icon: Set(input.icon),
            color: Set(input.color),
            moderated: Set(input.moderated),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        forum_category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            name: Set(canonical_name.clone()),
            slug: Set(slug.clone()),
            description: Set(canonical_description.clone()),
        }
        .insert(&txn)
        .await?;

        taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            id,
            locale,
            canonical_name,
            slug,
            canonical_description,
        )
        .await?;
        taxonomy_sync::sync_siblings_for_parent_in_tx(&txn, tenant_id, input.parent_id).await?;
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
        if let Some(position) = input.position {
            active.position = Set(position);
        }
        if input.icon.is_some() {
            active.icon = Set(input.icon);
        }
        if input.color.is_some() {
            active.color = Set(input.color);
        }
        if let Some(moderated) = input.moderated {
            active.moderated = Set(moderated);
        }
        active.update(&txn).await?;

        let existing_canonical = taxonomy_sync::load_category_locale_copy_in_tx(
            &txn,
            tenant_id,
            category_id,
            &locale,
        )
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

        let existing_compatibility = forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category_id))
            .filter(forum_category_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;
        match existing_compatibility {
            Some(existing) => {
                let mut active: forum_category_translation::ActiveModel = existing.into();
                active.name = Set(canonical_name.clone());
                active.slug = Set(canonical_slug.clone());
                active.description = Set(canonical_description.clone());
                active.update(&txn).await?;
            }
            None => {
                forum_category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(canonical_name.clone()),
                    slug: Set(canonical_slug.clone()),
                    description: Set(canonical_description.clone()),
                }
                .insert(&txn)
                .await?;
            }
        }

        taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            category_id,
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

    #[instrument(skip(self, security))]
    pub(super) async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumCategories, Action::Delete)?;
        let txn = self.db.begin().await?;
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        forum_category::Entity::delete_by_id(category.id)
            .exec(&txn)
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

impl Deref for CategoryProjectionOwnerService {
    type Target = CategoryService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub(super) mod taxonomy_sync {
    include!("category_taxonomy_sync.rs");
}
