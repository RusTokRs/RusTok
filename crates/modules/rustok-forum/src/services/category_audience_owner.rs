/// Transactional owner facade for category audience policy replacement.
///
/// Read operations continue through the established service. Replacement is
/// repeated here inside the same module so private normalized storage helpers
/// remain the single persistence implementation and the full Forum projection
/// invalidation shares the owner transaction.
pub struct ForumCategoryAudiencePolicyOwnerService {
    inner: ForumCategoryAudiencePolicyService,
    db: DatabaseConnection,
}

impl ForumCategoryAudiencePolicyOwnerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: ForumCategoryAudiencePolicyService::new(db.clone()),
            db,
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumCategoryAudiencePolicy> {
        self.inner.get(tenant_id, category_id, security).await
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: SetForumCategoryAudiencePolicyInput,
    ) -> ForumResult<ForumCategoryAudiencePolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let constraints = input.constraints.normalize()?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        load_category_ancestor_ids(&txn, tenant_id, category_id).await?;

        forum_category_audience_policy::Entity::delete_many()
            .filter(forum_category_audience_policy::Column::TenantId.eq(tenant_id))
            .filter(forum_category_audience_policy::Column::CategoryId.eq(category_id))
            .exec(&txn)
            .await?;

        if !constraints_are_empty(&constraints) {
            forum_category_audience_policy::ActiveModel {
                tenant_id: Set(tenant_id),
                category_id: Set(category_id),
                minimum_trust_level: Set(constraints.minimum_trust_level.map(i16::from)),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;

            insert_roles(&txn, tenant_id, category_id, &constraints).await?;
            insert_channels(&txn, tenant_id, category_id, &constraints).await?;
            insert_groups(&txn, tenant_id, category_id, &constraints).await?;
            insert_users(&txn, tenant_id, category_id, &constraints).await?;
        }

        let result = load_category_audience_policy(&txn, tenant_id, category_id).await?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        Ok(result)
    }
}
