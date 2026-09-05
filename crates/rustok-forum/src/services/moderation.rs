use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::PortContext;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::audience::SharedForumAudienceFactsPort;
use crate::error::ForumResult;
use crate::services::moderation_audience_authorization::ForumModerationAudienceAuthorizationService;
use crate::services::TopicService;
use crate::state_machine::TopicStatus;

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
        let audience = ForumModerationAudienceAuthorizationService::new(db.clone(), Some(facts));
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
        let topic = TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
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
        txn.commit().await?;
        Ok(())
    }
}
