/// Transactional owner facade for topic-local audience replacement.
///
/// The original service remains the read delegate. This facade uses the same
/// private normalized persistence helpers and writes a topic projection
/// invalidation before the policy transaction commits.
pub struct ForumTopicAudiencePolicyOwnerService {
    inner: ForumTopicAudiencePolicyService,
    db: DatabaseConnection,
}

impl ForumTopicAudiencePolicyOwnerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: ForumTopicAudiencePolicyService::new(db.clone()),
            db,
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumTopicAudiencePolicy> {
        self.inner.get(tenant_id, topic_id, security).await
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: SetForumTopicAudiencePolicyInput,
    ) -> ForumResult<ForumTopicAudiencePolicy> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let constraints = input.constraints.normalize()?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let topic = super::topic_audience_lock::lock_active_topic_audience_write_in_tx(
            &txn, tenant_id, topic_id,
        )
        .await?;
        super::topic_audience_lock::lock_topic_audience_scopes_in_tx(&txn, tenant_id, &[topic_id])
            .await?;
        load_category_audience_policy(&txn, tenant_id, topic.category_id).await?;

        forum_topic_audience_policy::Entity::delete_many()
            .filter(forum_topic_audience_policy::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_audience_policy::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;

        if !constraints_are_empty(&constraints) {
            forum_topic_audience_policy::ActiveModel {
                tenant_id: Set(tenant_id),
                topic_id: Set(topic_id),
                minimum_trust_level: Set(constraints.minimum_trust_level.map(i16::from)),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;

            insert_roles(&txn, tenant_id, topic_id, &constraints).await?;
            insert_channels(&txn, tenant_id, topic_id, &constraints).await?;
            insert_groups(&txn, tenant_id, topic_id, &constraints).await?;
            insert_users(&txn, tenant_id, topic_id, &constraints).await?;
        }

        let result = load_policy_for_topic(&txn, tenant_id, &topic).await?;
        super::projection_invalidation::publish_forum_topic_projection_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        Ok(result)
    }
}
