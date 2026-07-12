use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use rustok_api::{Action, Resource};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::constants::reply_status;
use crate::entities::forum_solution;
use crate::error::ForumError;
use crate::error::ForumResult;
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::services::user_stats::UserStatsService;
use crate::services::{ReplyService, TopicService};
use crate::state_machine::{ReplyStatus, TopicStatus};

pub struct ModerationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ModerationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security))]
    pub async fn approve_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Moderate)?;
        self.update_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            ReplyStatus::Approved,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn reject_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Moderate)?;
        self.update_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            ReplyStatus::Rejected,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn hide_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Moderate)?;
        self.update_reply_status(tenant_id, reply_id, topic_id, security, ReplyStatus::Hidden)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn pin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        let txn = self.db.begin().await?;
        TopicService::set_pinned_in_tx(&txn, tenant_id, topic_id, true).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicPinned {
                    topic_id,
                    is_pinned: true,
                    moderator_id: security.user_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn unpin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        let txn = self.db.begin().await?;
        TopicService::set_pinned_in_tx(&txn, tenant_id, topic_id, false).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicPinned {
                    topic_id,
                    is_pinned: false,
                    moderator_id: security.user_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn lock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        let txn = self.db.begin().await?;
        TopicService::set_locked_in_tx(&txn, tenant_id, topic_id, true).await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn unlock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        let txn = self.db.begin().await?;
        TopicService::set_locked_in_tx(&txn, tenant_id, topic_id, false).await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn close_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        self.update_topic_status(tenant_id, topic_id, security, TopicStatus::Closed)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn reopen_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        self.update_topic_status(tenant_id, topic_id, security, TopicStatus::Open)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn archive_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Moderate)?;
        self.update_topic_status(tenant_id, topic_id, security, TopicStatus::Archived)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn mark_solution(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let topic_service = TopicService::new(self.db.clone(), self.event_bus.clone());
        let reply_service = ReplyService::new(self.db.clone(), self.event_bus.clone());
        let topic = topic_service.find_topic(tenant_id, topic_id).await?;
        enforce_solution_scope(&security, topic.author_id)?;
        let reply = reply_service.find_reply(tenant_id, reply_id).await?;
        if reply.topic_id != topic_id {
            return Err(ForumError::Validation(
                "Reply belongs to another topic".to_string(),
            ));
        }
        if reply.status != reply_status::APPROVED {
            return Err(ForumError::Validation(
                "Only approved replies can be marked as solutions".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let previous_solution_reply_id = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&txn)
            .await?
            .map(|solution| solution.reply_id);
        let previous_solution_author_id =
            if let Some(previous_reply_id) = previous_solution_reply_id {
                ReplyService::find_reply_in_tx(&txn, tenant_id, previous_reply_id)
                    .await?
                    .author_id
            } else {
                None
            };
        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;
        forum_solution::ActiveModel {
            topic_id: Set(topic_id),
            tenant_id: Set(tenant_id),
            reply_id: Set(reply_id),
            marked_by_user_id: Set(security.user_id),
            marked_at: Set(Utc::now().into()),
        }
        .insert(&txn)
        .await?;
        if previous_solution_reply_id != Some(reply_id) {
            UserStatsService::adjust_solution_count_in_tx(
                &txn,
                tenant_id,
                previous_solution_author_id,
                -1,
            )
            .await?;
            UserStatsService::adjust_solution_count_in_tx(&txn, tenant_id, reply.author_id, 1)
                .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn clear_solution(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let topic_service = TopicService::new(self.db.clone(), self.event_bus.clone());
        let topic = topic_service.find_topic(tenant_id, topic_id).await?;
        enforce_solution_scope(&security, topic.author_id)?;

        let txn = self.db.begin().await?;
        let solution_author_id = if let Some(solution) = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&txn)
            .await?
        {
            ReplyService::find_reply_in_tx(&txn, tenant_id, solution.reply_id)
                .await?
                .author_id
        } else {
            None
        };
        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;
        UserStatsService::adjust_solution_count_in_tx(&txn, tenant_id, solution_author_id, -1)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn update_reply_status(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        target: ReplyStatus,
    ) -> ForumResult<()> {
        let txn = self.db.begin().await?;
        let reply = ReplyService::find_reply_in_tx(&txn, tenant_id, reply_id).await?;
        let current = ReplyStatus::from_str_value(&reply.status).ok_or_else(|| {
            crate::error::ForumError::Validation(format!("Unknown reply status: {}", reply.status))
        })?;
        current.validate_transition(&target)?;

        let old_status = current.as_str().to_string();
        let new_status = target.as_str().to_string();

        ReplyService::set_status_in_tx(&txn, tenant_id, reply_id, &new_status).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id,
                    old_status,
                    new_status,
                    moderator_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;
        Ok(())
    }

    async fn update_topic_status(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        target: TopicStatus,
    ) -> ForumResult<()> {
        let topic_service = TopicService::new(self.db.clone(), self.event_bus.clone());
        let topic = topic_service.find_topic(tenant_id, topic_id).await?;
        let current = TopicStatus::from_str_value(&topic.status).unwrap_or(TopicStatus::Open);
        current.validate_transition(&target)?;

        let old_status = current.as_str().to_string();
        let new_status = target.as_str().to_string();

        let txn = self.db.begin().await?;
        TopicService::set_status_in_tx(&txn, tenant_id, topic_id, &new_status).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicStatusChanged {
                    topic_id,
                    old_status,
                    new_status,
                    moderator_id: security.user_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }
}

fn enforce_solution_scope(
    security: &SecurityContext,
    topic_author_id: Option<Uuid>,
) -> ForumResult<()> {
    if enforce_owned_scope(
        security,
        Resource::ForumTopics,
        Action::Update,
        topic_author_id,
    )
    .is_ok()
    {
        return Ok(());
    }

    enforce_scope(security, Resource::ForumTopics, Action::Moderate)
}
