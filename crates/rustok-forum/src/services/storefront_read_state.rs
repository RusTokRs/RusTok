use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{ListTopicsFilter, TopicListItem, TopicUnreadSummaryReadModel};
use crate::entities::{forum_reply, forum_topic, forum_topic_revision};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::read_model::ForumReadModelService;
use crate::services::read_tracking::{
    ForumTopicReadState, ForumTopicReadStateService, MarkForumTopicReadInput,
};
use crate::services::topic_facade::TopicService;
use crate::state_machine::ReplyStatus;

pub type ForumTopicUnreadSummary = TopicUnreadSummaryReadModel;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ForumStorefrontUnreadTopic {
    pub topic: TopicListItem,
    pub read_state_explicit: bool,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub unread_count: i64,
    pub has_unread_topic_revision: bool,
    pub is_unread: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ForumStorefrontUnreadTopicPage {
    pub items: Vec<ForumStorefrontUnreadTopic>,
    pub total: u64,
}

/// Visibility-safe storefront composition over canonical Forum owner services.
pub struct ForumStorefrontReadStateService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumStorefrontReadStateService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    /// Selects the storefront-visible topic page before enriching those exact IDs
    /// through the canonical unread aggregate. Raw arbitrary-ID enrichment is not
    /// exposed outside the Forum owner crate.
    pub async fn list_topics_with_unread(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<ForumStorefrontUnreadTopicPage> {
        let (topics, total) = TopicService::new(self.db.clone(), self.event_bus.clone())
            .list_storefront_visible_with_locale_fallback(
                tenant_id,
                security.clone(),
                filter,
                fallback_locale,
                channel_slug,
            )
            .await?;
        let summaries = ForumReadModelService::new(self.db.clone())
            .summarize_topic_ids(
                tenant_id,
                security,
                topics.iter().map(|topic| topic.id).collect(),
            )
            .await?
            .into_iter()
            .map(|summary| (summary.topic_id, summary))
            .collect::<HashMap<_, _>>();
        let items = topics
            .into_iter()
            .map(|topic| {
                let summary = summaries.get(&topic.id).copied().ok_or_else(|| {
                    ForumError::Internal(
                        "Forum storefront unread summary is unavailable".to_string(),
                    )
                })?;
                Ok(ForumStorefrontUnreadTopic {
                    topic,
                    read_state_explicit: summary.read_state_explicit,
                    last_read_position: summary.last_read_position,
                    last_read_revision: summary.last_read_revision,
                    unread_count: summary.unread_count,
                    has_unread_topic_revision: summary.has_unread_topic_revision,
                    is_unread: summary.is_unread,
                })
            })
            .collect::<ForumResult<Vec<_>>>()?;

        Ok(ForumStorefrontUnreadTopicPage { items, total })
    }

    /// Rechecks storefront visibility before marking the latest approved reply
    /// position and immutable topic revision observed by the owner service.
    pub async fn mark_topic_read_current_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<ForumTopicReadState> {
        let visible = TopicService::new(self.db.clone(), self.event_bus.clone())
            .get_storefront_visible_with_locale_fallback(
                tenant_id,
                security.clone(),
                topic_id,
                locale,
                fallback_locale,
                channel_slug,
            )
            .await?;
        if visible.is_none() {
            return Err(ForumError::TopicNotFound(topic_id));
        }
        self.mark_topic_read_current(tenant_id, topic_id, security)
            .await
    }

    async fn mark_topic_read_current(
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
