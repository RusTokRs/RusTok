use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use serde_json::{Value, json};
use uuid::Uuid;

use rustok_core::{
    DomainEvent, InputValidator, PermissionScope, SecurityContext, ValidationResult,
};

use rustok_api::{Action, Resource};
use rustok_outbox::TransactionalEventBus;

use super::canonical_url_service::CanonicalUrlWriter;
use crate::entities::{orchestration_audit_log, orchestration_operation};
use crate::error::{ContentError, ContentResult};

#[derive(Debug, Clone)]
pub struct PromoteTopicToPostInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub blog_category_id: Option<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct DemotePostToTopicInput {
    pub post_id: Uuid,
    pub locale: String,
    pub forum_category_id: Uuid,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct SplitTopicInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub reply_ids: Vec<Uuid>,
    pub new_title: String,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct MergeTopicsInput {
    pub target_topic_id: Uuid,
    pub source_topic_ids: Vec<Uuid>,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationResult {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub moved_comments: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetiredCanonicalTarget {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalUrlMutation {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub canonical_url: String,
    pub alias_urls: Vec<String>,
    pub retired_targets: Vec<RetiredCanonicalTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTopicToPostOutput {
    pub topic_id: Uuid,
    pub post_id: Uuid,
    pub moved_comments: u64,
    pub effective_locale: String,
    pub url_updates: Vec<CanonicalUrlMutation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemotePostToTopicOutput {
    pub post_id: Uuid,
    pub topic_id: Uuid,
    pub moved_comments: u64,
    pub effective_locale: String,
    pub url_updates: Vec<CanonicalUrlMutation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitTopicOutput {
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub moved_reply_ids: Vec<Uuid>,
    pub moved_comments: u64,
    pub url_updates: Vec<CanonicalUrlMutation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeTopicsOutput {
    pub target_topic_id: Uuid,
    pub source_topic_ids: Vec<Uuid>,
    pub moved_comments: u64,
    pub url_updates: Vec<CanonicalUrlMutation>,
}

#[async_trait]
pub trait ContentOrchestrationBridge: Send + Sync {
    async fn promote_topic_to_post(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &PromoteTopicToPostInput,
    ) -> ContentResult<PromoteTopicToPostOutput>;

    async fn demote_post_to_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &DemotePostToTopicInput,
    ) -> ContentResult<DemotePostToTopicOutput>;

    async fn split_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &SplitTopicInput,
    ) -> ContentResult<SplitTopicOutput>;

    async fn merge_topics(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &MergeTopicsInput,
    ) -> ContentResult<MergeTopicsOutput>;
}

pub struct ContentOrchestrationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    bridge: Arc<dyn ContentOrchestrationBridge>,
}

struct PersistOrchestrationRecordInput<'a> {
    tenant_id: Uuid,
    operation: &'a str,
    idempotency_key: &'a str,
    actor_id: Option<Uuid>,
    result: &'a OrchestrationResult,
    payload: Value,
}

impl ContentOrchestrationService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        bridge: Arc<dyn ContentOrchestrationBridge>,
    ) -> Self {
        Self {
            db,
            event_bus,
            bridge,
        }
    }

    pub async fn promote_topic_to_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: PromoteTopicToPostInput,
    ) -> ContentResult<OrchestrationResult> {
        self.ensure_scope(security.clone(), Resource::ForumTopics, Action::Moderate)?;
        self.ensure_scope(security.clone(), Resource::BlogPosts, Action::Create)?;
        self.ensure_idempotency_key(&input.idempotency_key)?;
        self.ensure_safe_optional_text("reason", input.reason.as_deref())?;

        let txn = self.db.begin().await?;
        if let Some(existing) = self
            .fetch_idempotent_result(
                &txn,
                tenant_id,
                "promote_topic_to_post",
                &input.idempotency_key,
            )
            .await?
        {
            txn.rollback().await?;
            return Ok(existing);
        }

        let bridge_result = self
            .bridge
            .promote_topic_to_post(&txn, tenant_id, security.user_id, &input)
            .await?;

        self.claim_blog_post_routes_in_tx(
            &txn,
            tenant_id,
            security.user_id,
            &bridge_result.url_updates,
        )
        .await?;

        self.apply_canonical_url_mutations(
            &txn,
            tenant_id,
            security.user_id,
            &bridge_result.url_updates,
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TopicPromotedToPost {
                    topic_id: bridge_result.topic_id,
                    post_id: bridge_result.post_id,
                    moved_comments: bridge_result.moved_comments,
                    locale: bridge_result.effective_locale.clone(),
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: bridge_result.topic_id,
            target_id: bridge_result.post_id,
            moved_comments: bridge_result.moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            PersistOrchestrationRecordInput {
                tenant_id,
                operation: "promote_topic_to_post",
                idempotency_key: &input.idempotency_key,
                actor_id: security.user_id,
                result: &result,
                payload: json!({
                    "locale": bridge_result.effective_locale,
                    "blog_category_id": input.blog_category_id,
                    "reason": input.reason,
                }),
            },
        )
        .await?;

        txn.commit().await?;
        Ok(result)
    }

    pub async fn demote_post_to_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: DemotePostToTopicInput,
    ) -> ContentResult<OrchestrationResult> {
        self.ensure_scope(security.clone(), Resource::BlogPosts, Action::Moderate)?;
        self.ensure_scope(security.clone(), Resource::ForumTopics, Action::Create)?;
        self.ensure_idempotency_key(&input.idempotency_key)?;
        self.ensure_safe_optional_text("reason", input.reason.as_deref())?;

        let txn = self.db.begin().await?;
        if let Some(existing) = self
            .fetch_idempotent_result(
                &txn,
                tenant_id,
                "demote_post_to_topic",
                &input.idempotency_key,
            )
            .await?
        {
            txn.rollback().await?;
            return Ok(existing);
        }

        let bridge_result = self
            .bridge
            .demote_post_to_topic(&txn, tenant_id, security.user_id, &input)
            .await?;

        self.apply_canonical_url_mutations(
            &txn,
            tenant_id,
            security.user_id,
            &bridge_result.url_updates,
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::PostDemotedToTopic {
                    post_id: bridge_result.post_id,
                    topic_id: bridge_result.topic_id,
                    moved_comments: bridge_result.moved_comments,
                    locale: bridge_result.effective_locale.clone(),
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: bridge_result.post_id,
            target_id: bridge_result.topic_id,
            moved_comments: bridge_result.moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            PersistOrchestrationRecordInput {
                tenant_id,
                operation: "demote_post_to_topic",
                idempotency_key: &input.idempotency_key,
                actor_id: security.user_id,
                result: &result,
                payload: json!({
                    "locale": bridge_result.effective_locale,
                    "forum_category_id": input.forum_category_id,
                    "reason": input.reason,
                }),
            },
        )
        .await?;

        txn.commit().await?;
        Ok(result)
    }

    pub async fn split_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: SplitTopicInput,
    ) -> ContentResult<OrchestrationResult> {
        self.ensure_scope(security.clone(), Resource::ForumTopics, Action::Moderate)?;
        self.ensure_idempotency_key(&input.idempotency_key)?;
        self.ensure_safe_text("new_title", &input.new_title)?;
        self.ensure_safe_optional_text("reason", input.reason.as_deref())?;
        if input.reply_ids.is_empty() {
            return Err(ContentError::Validation(
                "split_topic requires at least one reply/comment id".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        if let Some(existing) = self
            .fetch_idempotent_result(&txn, tenant_id, "split_topic", &input.idempotency_key)
            .await?
        {
            txn.rollback().await?;
            return Ok(existing);
        }

        let bridge_result = self
            .bridge
            .split_topic(&txn, tenant_id, security.user_id, &input)
            .await?;

        self.apply_canonical_url_mutations(
            &txn,
            tenant_id,
            security.user_id,
            &bridge_result.url_updates,
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TopicSplit {
                    source_topic_id: bridge_result.source_topic_id,
                    target_topic_id: bridge_result.target_topic_id,
                    moved_comment_ids: bridge_result.moved_reply_ids.clone(),
                    moved_comments: bridge_result.moved_comments,
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: bridge_result.source_topic_id,
            target_id: bridge_result.target_topic_id,
            moved_comments: bridge_result.moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            PersistOrchestrationRecordInput {
                tenant_id,
                operation: "split_topic",
                idempotency_key: &input.idempotency_key,
                actor_id: security.user_id,
                result: &result,
                payload: json!({
                    "locale": input.locale,
                    "reason": input.reason,
                    "reply_ids": bridge_result.moved_reply_ids,
                    "new_title": input.new_title,
                }),
            },
        )
        .await?;

        txn.commit().await?;
        Ok(result)
    }

    pub async fn merge_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: MergeTopicsInput,
    ) -> ContentResult<OrchestrationResult> {
        self.ensure_scope(security.clone(), Resource::ForumTopics, Action::Moderate)?;
        self.ensure_idempotency_key(&input.idempotency_key)?;
        self.ensure_safe_optional_text("reason", input.reason.as_deref())?;
        if input.source_topic_ids.is_empty() {
            return Err(ContentError::Validation(
                "merge_topics requires at least one source topic".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        if let Some(existing) = self
            .fetch_idempotent_result(&txn, tenant_id, "merge_topics", &input.idempotency_key)
            .await?
        {
            txn.rollback().await?;
            return Ok(existing);
        }

        let bridge_result = self
            .bridge
            .merge_topics(&txn, tenant_id, security.user_id, &input)
            .await?;

        self.apply_canonical_url_mutations(
            &txn,
            tenant_id,
            security.user_id,
            &bridge_result.url_updates,
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TopicsMerged {
                    target_topic_id: bridge_result.target_topic_id,
                    moved_comments: bridge_result.moved_comments,
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: bridge_result.target_topic_id,
            target_id: bridge_result.target_topic_id,
            moved_comments: bridge_result.moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            PersistOrchestrationRecordInput {
                tenant_id,
                operation: "merge_topics",
                idempotency_key: &input.idempotency_key,
                actor_id: security.user_id,
                result: &result,
                payload: json!({
                    "reason": input.reason,
                    "source_topic_ids": bridge_result.source_topic_ids,
                }),
            },
        )
        .await?;

        txn.commit().await?;
        Ok(result)
    }

    fn ensure_scope(
        &self,
        security: SecurityContext,
        resource: Resource,
        action: Action,
    ) -> ContentResult<()> {
        match security.get_scope(resource, action) {
            PermissionScope::All => Ok(()),
            PermissionScope::Own => {
                if security.user_id.is_some() {
                    Ok(())
                } else {
                    Err(ContentError::Forbidden("Permission denied".to_string()))
                }
            }
            PermissionScope::None => Err(ContentError::Forbidden("Permission denied".to_string())),
        }
    }

    fn ensure_idempotency_key(&self, idempotency_key: &str) -> ContentResult<()> {
        if idempotency_key.trim().is_empty() {
            return Err(ContentError::Validation(
                "idempotency_key must not be empty".to_string(),
            ));
        }

        self.ensure_safe_text("idempotency_key", idempotency_key)?;

        if idempotency_key.len() > 128 {
            return Err(ContentError::Validation(
                "idempotency_key must be <= 128 chars".to_string(),
            ));
        }

        Ok(())
    }

    fn ensure_safe_text(&self, field: &str, value: &str) -> ContentResult<()> {
        let validator = InputValidator::new();
        match validator.validate(value) {
            ValidationResult::Valid => Ok(()),
            ValidationResult::Invalid { reason } => Err(ContentError::Validation(format!(
                "{field} contains unsafe payload: {reason}"
            ))),
            ValidationResult::Sanitized { .. } => Ok(()),
        }
    }

    fn ensure_safe_optional_text(&self, field: &str, value: Option<&str>) -> ContentResult<()> {
        if let Some(value) = value {
            self.ensure_safe_text(field, value)?;
        }
        Ok(())
    }

    /// A promoted topic takes its Blog route over from any retired alias that an
    /// earlier post left behind, the same claim rule Blog applies to its own slugs.
    async fn claim_blog_post_routes_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        updates: &[CanonicalUrlMutation],
    ) -> ContentResult<()> {
        let writer = CanonicalUrlWriter::new(self.event_bus.clone());
        for update in updates
            .iter()
            .filter(|update| update.target_kind == "blog_post")
        {
            writer
                .release_alias_route_in_tx(txn, tenant_id, actor_id, &update.canonical_url)
                .await?;
        }
        Ok(())
    }

    async fn apply_canonical_url_mutations(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        updates: &[CanonicalUrlMutation],
    ) -> ContentResult<()> {
        CanonicalUrlWriter::new(self.event_bus.clone())
            .apply_canonical_url_mutations(txn, tenant_id, actor_id, updates)
            .await
    }

    async fn fetch_idempotent_result(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        operation: &str,
        idempotency_key: &str,
    ) -> ContentResult<Option<OrchestrationResult>> {
        let existing = orchestration_operation::Entity::find()
            .filter(orchestration_operation::Column::TenantId.eq(tenant_id))
            .filter(orchestration_operation::Column::Operation.eq(operation))
            .filter(orchestration_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(txn)
            .await?;

        Ok(existing.map(|it| OrchestrationResult {
            source_id: it.source_id,
            target_id: it.target_id,
            moved_comments: it.moved_comments as u64,
        }))
    }

    async fn persist_orchestration_record(
        &self,
        txn: &DatabaseTransaction,
        input: PersistOrchestrationRecordInput<'_>,
    ) -> ContentResult<()> {
        let now = Utc::now();

        orchestration_operation::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(input.tenant_id),
            operation: Set(input.operation.to_string()),
            idempotency_key: Set(input.idempotency_key.to_string()),
            source_id: Set(input.result.source_id),
            target_id: Set(input.result.target_id),
            moved_comments: Set(input.result.moved_comments as i64),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        orchestration_audit_log::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(input.tenant_id),
            operation: Set(input.operation.to_string()),
            idempotency_key: Set(input.idempotency_key.to_string()),
            actor_id: Set(input.actor_id),
            source_id: Set(input.result.source_id),
            target_id: Set(input.result.target_id),
            payload: Set(input.payload),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        Ok(())
    }
}
