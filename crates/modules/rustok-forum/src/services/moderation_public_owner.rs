use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_api::PortContext;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::error::ForumResult;

/// Public moderation facade that keeps legacy and transactional owner types
/// private while preserving the established method surface.
pub struct ModerationService {
    inner: super::moderation_owner::ModerationService,
}

impl ModerationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: super::moderation_owner::ModerationService::new(db, event_bus),
        }
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        facts: SharedForumAudienceFactsPort,
    ) -> Self {
        Self {
            inner: super::moderation_owner::ModerationService::with_audience_facts(
                db, event_bus, facts,
            ),
        }
    }

    pub async fn approve_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner
            .approve_reply(tenant_id, reply_id, topic_id, security)
            .await
    }

    pub async fn approve_reply_with_audience_context(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .approve_reply_with_audience_context(tenant_id, reply_id, topic_id, security, context)
            .await
    }

    pub async fn reject_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner
            .reject_reply(tenant_id, reply_id, topic_id, security)
            .await
    }

    pub async fn reject_reply_with_audience_context(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .reject_reply_with_audience_context(tenant_id, reply_id, topic_id, security, context)
            .await
    }

    pub async fn hide_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner
            .hide_reply(tenant_id, reply_id, topic_id, security)
            .await
    }

    pub async fn hide_reply_with_audience_context(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .hide_reply_with_audience_context(tenant_id, reply_id, topic_id, security, context)
            .await
    }

    pub async fn pin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.pin_topic(tenant_id, topic_id, security).await
    }

    pub async fn pin_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .pin_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn unpin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.unpin_topic(tenant_id, topic_id, security).await
    }

    pub async fn unpin_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .unpin_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn lock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.lock_topic(tenant_id, topic_id, security).await
    }

    pub async fn lock_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .lock_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn unlock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.unlock_topic(tenant_id, topic_id, security).await
    }

    pub async fn unlock_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .unlock_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn close_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.close_topic(tenant_id, topic_id, security).await
    }

    pub async fn close_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .close_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn reopen_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner.reopen_topic(tenant_id, topic_id, security).await
    }

    pub async fn reopen_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .reopen_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn archive_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner
            .archive_topic(tenant_id, topic_id, security)
            .await
    }

    pub async fn archive_topic_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .archive_topic_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }

    pub async fn mark_solution(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner
            .mark_solution(tenant_id, topic_id, reply_id, security)
            .await
    }

    pub async fn mark_solution_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .mark_solution_with_audience_context(tenant_id, topic_id, reply_id, security, context)
            .await
    }

    pub async fn clear_solution(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.inner
            .clear_solution(tenant_id, topic_id, security)
            .await
    }

    pub async fn clear_solution_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
    ) -> ForumResult<()> {
        self.inner
            .clear_solution_with_audience_context(tenant_id, topic_id, security, context)
            .await
    }
}
