use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::NodeService;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::constants::{reply_status, topic_status, KIND_TOPIC};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

pub struct ModerationService {
    db: DatabaseConnection,
    nodes: NodeService,
    event_bus: TransactionalEventBus,
}

impl ModerationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            nodes: NodeService::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
        }
    }

    // ── Reply moderation ───────────────────────────────────────────────────

    #[instrument(skip(self, security))]
    pub async fn approve_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            reply_status::APPROVED,
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
        self.update_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            reply_status::REJECTED,
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
        self.update_reply_status(
            tenant_id,
            reply_id,
            topic_id,
            security,
            reply_status::HIDDEN,
        )
        .await
    }

    // ── Topic moderation ───────────────────────────────────────────────────

    #[instrument(skip(self, security))]
    pub async fn pin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_pin_flag(tenant_id, topic_id, security, true)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn unpin_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_pin_flag(tenant_id, topic_id, security, false)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn lock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_bool_flag(tenant_id, topic_id, security, "is_locked", true)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn unlock_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_bool_flag(tenant_id, topic_id, security, "is_locked", false)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn close_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_forum_status(tenant_id, topic_id, security, TopicStatus::Closed)
            .await
    }

    /// Reopen a closed or archived topic.
    #[instrument(skip(self, security))]
    pub async fn reopen_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_forum_status(tenant_id, topic_id, security, TopicStatus::Open)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn archive_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_topic_forum_status(tenant_id, topic_id, security, TopicStatus::Archived)
            .await
    }

    // ── Private helpers ────────────────────────────────────────────────────

    async fn update_reply_status(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        new_status: &str,
    ) -> ForumResult<()> {
        let node = self.nodes.get_node(tenant_id, reply_id).await?;
        let mut metadata = node.metadata;
        metadata["reply_status"] = serde_json::json!(new_status);

        let txn = self.db.begin().await?;

        self.nodes
            .update_node_in_tx(
                &txn,
                tenant_id,
                reply_id,
                security.clone(),
                rustok_content::UpdateNodeInput {
                    metadata: Some(metadata),
                    ..Default::default()
                },
            )
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id,
                    new_status: new_status.to_string(),
                    moderator_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;

        Ok(())
    }

    async fn update_topic_pin_flag(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        is_pinned: bool,
    ) -> ForumResult<()> {
        let node = self.nodes.get_node(tenant_id, topic_id).await?;
        if node.kind != KIND_TOPIC {
            return Err(ForumError::TopicNotFound(topic_id));
        }
        let mut metadata = node.metadata;
        metadata["is_pinned"] = serde_json::json!(is_pinned);

        let txn = self.db.begin().await?;

        self.nodes
            .update_node_in_tx(
                &txn,
                tenant_id,
                topic_id,
                security.clone(),
                rustok_content::UpdateNodeInput {
                    metadata: Some(metadata),
                    ..Default::default()
                },
            )
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicPinned {
                    topic_id,
                    is_pinned,
                    moderator_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;

        Ok(())
    }

    async fn update_topic_bool_flag(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        flag: &str,
        value: bool,
    ) -> ForumResult<()> {
        let node = self.nodes.get_node(tenant_id, topic_id).await?;
        if node.kind != KIND_TOPIC {
            return Err(ForumError::TopicNotFound(topic_id));
        }
        let mut metadata = node.metadata;
        metadata[flag] = serde_json::json!(value);

        self.nodes
            .update_node(
                tenant_id,
                topic_id,
                security,
                rustok_content::UpdateNodeInput {
                    metadata: Some(metadata),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    /// Update topic forum status with state machine validation.
    ///
    /// Reads the current status from metadata, validates the transition using
    /// the state machine, and then applies the change atomically.
    async fn update_topic_forum_status(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        target: TopicStatus,
    ) -> ForumResult<()> {
        let node = self.nodes.get_node(tenant_id, topic_id).await?;
        if node.kind != KIND_TOPIC {
            return Err(ForumError::TopicNotFound(topic_id));
        }

        let current_str = node
            .metadata
            .get("forum_status")
            .and_then(|v| v.as_str())
            .unwrap_or(topic_status::OPEN);
        let current = TopicStatus::from_str_value(current_str).unwrap_or(TopicStatus::Open);

        // Validate the state transition using the state machine
        current.validate_transition(&target)?;

        let old_status = current.as_str().to_string();
        let new_status = target.as_str().to_string();

        let mut metadata = node.metadata;
        metadata["forum_status"] = serde_json::json!(&new_status);

        let txn = self.db.begin().await?;

        self.nodes
            .update_node_in_tx(
                &txn,
                tenant_id,
                topic_id,
                security.clone(),
                rustok_content::UpdateNodeInput {
                    metadata: Some(metadata),
                    ..Default::default()
                },
            )
            .await?;

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
