use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::PortContext;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::forum_solution;
use crate::error::{ForumError, ForumResult};
use crate::services::moderation_audience_authorization::ForumModerationAudienceAuthorizationService;
use crate::services::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use crate::services::topic_solution_lock::lock_topic_solution_scopes_in_tx;
use crate::services::user_stats::UserStatsService;
use crate::services::{CategoryService, ReplyService, TopicService};
use crate::state_machine::{ReplyStatus, TopicStatus};

/// Canonical Forum moderation owner for topic/reply lifecycle and projection effects.
pub struct ModerationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    audience: ForumModerationAudienceAuthorizationService,
}

impl ModerationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let audience =
            ForumModerationAudienceAuthorizationService::without_facts_provider(db.clone());
        Self {
            db,
            event_bus,
            audience,
        }
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        facts: SharedForumAudienceFactsPort,
    ) -> Self {
        let audience =
            ForumModerationAudienceAuthorizationService::new(db.clone(), Some(facts.clone()));
        Self {
            db,
            event_bus,
            audience,
        }
    }

    #[instrument(skip(self, security))]
    pub async fn pin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.set_topic_pinned(tenant_id, topic_id, security, None, true)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn pin_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.set_topic_pinned(tenant_id, topic_id, security, Some(context), true)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn unpin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.set_topic_pinned(tenant_id, topic_id, security, None, false)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn unpin_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.set_topic_pinned(tenant_id, topic_id, security, Some(context), false)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn close_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.moderate_topic_status(tenant_id, topic_id, security, None, TopicStatus::Closed)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn close_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.moderate_topic_status(
            tenant_id,
            topic_id,
            security,
            Some(context),
            TopicStatus::Closed,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn reopen_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.moderate_topic_status(tenant_id, topic_id, security, None, TopicStatus::Open)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn reopen_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.moderate_topic_status(
            tenant_id,
            topic_id,
            security,
            Some(context),
            TopicStatus::Open,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn archive_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.moderate_topic_status(tenant_id, topic_id, security, None, TopicStatus::Archived)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn archive_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.moderate_topic_status(
            tenant_id,
            topic_id,
            security,
            Some(context),
            TopicStatus::Archived,
        )
        .await
    }

    async fn set_topic_pinned(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        pinned: bool,
    ) -> ForumResult<()> {
        self.audience
            .require_topic(tenant_id, topic_id, &security, context)
            .await?;
        let txn = self.db.begin().await?;
        TopicService::set_pinned_in_tx(&txn, tenant_id, topic_id, pinned).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicPinned {
                    topic_id,
                    is_pinned: pinned,
                    moderator_id: security.user_id,
                },
            )
            .await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn moderate_topic_status(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        target: TopicStatus,
    ) -> ForumResult<()> {
        self.audience
            .require_topic(tenant_id, topic_id, &security, context)
            .await?;
        self.update_topic_status(tenant_id, topic_id, security, target)
            .await
    }

    async fn update_topic_status(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        target: TopicStatus,
    ) -> ForumResult<()> {
        let txn = self.db.begin().await?;
        let topic = TopicService::find_topic_for_update_in_tx(&txn, tenant_id, topic_id).await?;
        let current = topic.status;
        current.validate_transition(&target)?;

        let old_status = current.to_string();
        let new_status = target.to_string();

        TopicService::set_status_in_tx(&txn, tenant_id, topic_id, target).await?;
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
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn approve_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.moderate_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            None,
            ReplyStatus::Approved,
        )
        .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn approve_reply_with_audience_context(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.moderate_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            Some(context),
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
        self.moderate_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            None,
            ReplyStatus::Rejected,
        )
        .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn reject_reply_with_audience_context(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.moderate_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            Some(context),
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
        self.moderate_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            None,
            ReplyStatus::Hidden,
        )
        .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn hide_reply_with_audience_context(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.moderate_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            Some(context),
            ReplyStatus::Hidden,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn lock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.set_topic_locked(tenant_id, topic_id, security, None, true)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn lock_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.set_topic_locked(tenant_id, topic_id, security, Some(context), true)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn unlock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.set_topic_locked(tenant_id, topic_id, security, None, false)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn unlock_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.set_topic_locked(tenant_id, topic_id, security, Some(context), false)
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
        self.mark_solution_with_optional_audience_context(
            tenant_id, topic_id, reply_id, security, None,
        )
        .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn mark_solution_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.mark_solution_with_optional_audience_context(
            tenant_id,
            topic_id,
            reply_id,
            security,
            Some(context),
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn clear_solution(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.clear_solution_with_optional_audience_context(tenant_id, topic_id, security, None)
            .await
    }

    #[instrument(skip(self, security, context))]
    pub async fn clear_solution_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.clear_solution_with_optional_audience_context(
            tenant_id,
            topic_id,
            security,
            Some(context),
        )
        .await
    }

    async fn moderate_reply_status(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        target: ReplyStatus,
    ) -> ForumResult<()> {
        self.audience
            .require_reply(tenant_id, reply_id, topic_id, &security, context)
            .await?;
        self.update_reply_status(tenant_id, reply_id, topic_id, security, target)
            .await
    }

    async fn set_topic_locked(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        locked: bool,
    ) -> ForumResult<()> {
        self.audience
            .require_topic(tenant_id, topic_id, &security, context)
            .await?;
        let txn = self.db.begin().await?;
        TopicService::set_locked_in_tx(&txn, tenant_id, topic_id, locked).await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn mark_solution_with_optional_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<()> {
        let topic_service = TopicService::new(self.db.clone(), self.event_bus.clone());
        let topic = topic_service.find_topic(tenant_id, topic_id).await?;
        if !is_exact_topic_author(&security, topic.author_id) {
            self.audience
                .require_topic(tenant_id, topic_id, &security, context)
                .await?;
        }

        let txn = self.db.begin().await?;
        TopicService::find_topic_for_update_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;
        let reply =
            ReplyService::find_reply_for_update_in_tx(&txn, tenant_id, reply_id).await?;
        if reply.topic_id != topic_id {
            return Err(ForumError::Validation(
                "Reply belongs to another topic".to_string(),
            ));
        }
        if reply.status != ReplyStatus::Approved {
            return Err(ForumError::Validation(
                "Only approved replies can be marked as solutions".to_string(),
            ));
        }

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
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn clear_solution_with_optional_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<()> {
        let topic_service = TopicService::new(self.db.clone(), self.event_bus.clone());
        let topic = topic_service.find_topic(tenant_id, topic_id).await?;
        if !is_exact_topic_author(&security, topic.author_id) {
            self.audience
                .require_topic(tenant_id, topic_id, &security, context)
                .await?;
        }

        let txn = self.db.begin().await?;
        TopicService::find_topic_for_update_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;
        let solution = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&txn)
            .await?;
        let solution_author_id = if let Some(solution) = solution.as_ref() {
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
        if solution.is_some() {
            UserStatsService::adjust_solution_count_in_tx(&txn, tenant_id, solution_author_id, -1)
                .await?;
        }
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
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
        TopicService::find_topic_for_update_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;
        let reply = ReplyService::find_reply_in_tx(&txn, tenant_id, reply_id).await?;
        if reply.topic_id != topic_id {
            return Err(ForumError::Validation(
                "Reply belongs to another topic".to_string(),
            ));
        }
        let current = reply.status;
        current.validate_transition(&target)?;

        let became_public = current != ReplyStatus::Approved && target == ReplyStatus::Approved;
        let stopped_being_public =
            current == ReplyStatus::Approved && target != ReplyStatus::Approved;
        let old_status = current.to_string();
        let new_status = target.to_string();

        ReplyService::set_status_in_tx(&txn, tenant_id, reply_id, target).await?;

        if stopped_being_public {
            let solution_removed = forum_solution::Entity::delete_many()
                .filter(forum_solution::Column::TenantId.eq(tenant_id))
                .filter(forum_solution::Column::TopicId.eq(topic_id))
                .filter(forum_solution::Column::ReplyId.eq(reply_id))
                .exec(&txn)
                .await?;

            if solution_removed.rows_affected > 0 {
                UserStatsService::adjust_solution_count_in_tx(
                    &txn,
                    tenant_id,
                    reply.author_id,
                    -1,
                )
                .await?;
            }
        }

        let changed_category_id = if became_public || stopped_being_public {
            let public_delta = if became_public { 1 } else { -1 };
            let topic =
                TopicService::adjust_reply_count_in_tx(&txn, tenant_id, topic_id, public_delta)
                    .await?;
            CategoryService::adjust_counters_in_tx(
                &txn,
                tenant_id,
                topic.category_id,
                0,
                public_delta,
            )
            .await?;
            UserStatsService::adjust_reply_count_in_tx(
                &txn,
                tenant_id,
                reply.author_id,
                public_delta,
            )
            .await?;
            Some(topic.category_id)
        } else {
            None
        };

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

        if became_public {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    reply.author_id,
                    DomainEvent::ForumTopicReplied {
                        topic_id,
                        reply_id,
                        author_id: reply.author_id,
                    },
                )
                .await?;
        }
        if let Some(category_id) = changed_category_id {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                category_id,
            )
            .await?;
        }
        if changed_category_id.is_some() {
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic_id,
            )
            .await?;
        }

        txn.commit().await?;
        Ok(())
    }
}

fn is_exact_topic_author(security: &SecurityContext, topic_author_id: Option<Uuid>) -> bool {
    topic_author_id.is_some() && security.user_id == topic_author_id
}
