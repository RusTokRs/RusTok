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
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Create)?;
        validate_category_name(&input.name)?;
        let locale = normalize_locale(&input.locale)?;
        let slug = normalize_required_slug(&input.slug)?;
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
            name: Set(input.name),
            slug: Set(slug),
            description: Set(input.description),
        }
        .insert(&txn)
        .await?;

        taxonomy_sync::sync_siblings_for_parent_in_tx(&txn, tenant_id, input.parent_id).await?;
        super::category_translation_evidence::record_category_translation_change_in_tx(
            &txn,
            tenant_id,
            id,
            "create",
            rustok_translation_targets::TranslationResourceLifecycle::Active,
        )
        .await?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        self.inner.get(tenant_id, security, id, &locale).await
    }

    #[instrument(skip(self, security, input))]
    pub(super) async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Update)?;
        let locale = normalize_locale(&input.locale)?;
        let translation_requested =
            input.name.is_some() || input.slug.is_some() || input.description.is_some();
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

        let existing_translation = forum_category_translation::Entity::find()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category_id))
            .filter(forum_category_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;

        match existing_translation {
            Some(existing_translation) => {
                let previous_slug = normalize_required_slug(&existing_translation.slug)?;
                let next_slug = match input.slug.as_deref() {
                    Some(slug) => normalize_required_slug(slug)?,
                    None => match input.name.as_deref() {
                        Some(name) => {
                            validate_category_name(name)?;
                            normalize_required_slug(name)?
                        }
                        None => previous_slug.clone(),
                    },
                };
                let slug_changed = previous_slug != next_slug;

                let mut active: forum_category_translation::ActiveModel =
                    existing_translation.into();
                if let Some(name) = input.name {
                    validate_category_name(&name)?;
                    active.name = Set(name);
                }
                if slug_changed {
                    active.slug = Set(next_slug);
                }
                if input.description.is_some() {
                    active.description = Set(input.description);
                }
                active.update(&txn).await?;
            }
            None => {
                let name = input.name.ok_or_else(|| {
                    ForumError::Validation("Category name is required".to_string())
                })?;
                validate_category_name(&name)?;
                let slug = input
                    .slug
                    .as_deref()
                    .map(normalize_required_slug)
                    .transpose()?
                    .unwrap_or_else(|| normalize_slug(&name));
                let slug = normalize_required_slug(&slug)?;

                forum_category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(name),
                    slug: Set(slug),
                    description: Set(input.description),
                }
                .insert(&txn)
                .await?;
            }
        }

        taxonomy_sync::sync_category_locale_in_tx(&txn, tenant_id, category_id, &locale).await?;
        if translation_requested {
            super::category_translation_evidence::record_category_translation_change_in_tx(
                &txn,
                tenant_id,
                category_id,
                "update",
                rustok_translation_targets::TranslationResourceLifecycle::Active,
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
        self.inner
            .get(tenant_id, security, category_id, &locale)
            .await
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

        super::category_translation_evidence::record_category_translation_change_in_tx(
            &txn,
            tenant_id,
            category_id,
            "delete",
            rustok_translation_targets::TranslationResourceLifecycle::Deleted,
        )
        .await?;
        forum_category_translation::Entity::delete_many()
            .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_category_translation::Column::CategoryId.eq(category_id))
            .exec(&txn)
            .await?;
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
