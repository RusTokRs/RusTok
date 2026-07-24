use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::TopicUnreadSummaryReadModel;
use crate::entities::{forum_reply, forum_topic, forum_topic_revision};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::read_model::ForumReadModelService;
use crate::services::read_tracking::{
    ForumTopicReadState, ForumTopicReadStateService, MarkForumTopicReadInput,
};
use crate::state_machine::ReplyStatus;

pub type ForumTopicUnreadSummary = TopicUnreadSummaryReadModel;

/// Storefront composition facade over canonical Forum read-model and read-state owners.
///
/// This service never decides storefront visibility. Callers first obtain a
/// bounded topic page through the owner storefront-visible topic contract, then
/// pass only those topic IDs here for canonical unread enrichment.
pub struct ForumStorefrontReadStateService {
    db: DatabaseConnection,
}

impl ForumStorefrontReadStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn summarize_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: Vec<Uuid>,
    ) -> ForumResult<Vec<ForumTopicUnreadSummary>> {
        ForumReadModelService::new(self.db.clone())
            .summarize_topic_ids(tenant_id, security, topic_ids)
            .await
    }

    /// Marks the latest approved reply position and immutable topic revision
    /// observed by the owner service. Content published after this snapshot
    /// remains unread instead of being accidentally acknowledged.
    pub async fn mark_topic_read_current(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        if security.user_id.is_none() {
            return Err(ForumError::forbidden(
                "Authenticated user context is required to mark a storefront topic read",
            ));
        }

        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        let last_read_position = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic.id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .order_by_desc(forum_reply::Column::Position)
            .one(&self.db)
            .await?
            .map(|reply| reply.position)
            .unwrap_or(0);
        let last_read_revision = forum_topic_revision::Entity::find()
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic.id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .one(&self.db)
            .await?
            .map(|revision| revision.id)
            .unwrap_or(0);

        ForumTopicReadStateService::new(self.db.clone())
            .mark_topic_read(
                tenant_id,
                topic_id,
                security,
                MarkForumTopicReadInput {
                    last_read_position,
                    last_read_revision,
                },
            )
            .await
    }
}
