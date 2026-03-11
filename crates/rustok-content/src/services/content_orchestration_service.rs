use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde_json::{json, Value};
use uuid::Uuid;

use rustok_core::{
    Action, DomainEvent, InputValidator, PermissionScope, Resource, SecurityContext,
    ValidationResult,
};

use crate::dto::{BodyInput, CreateNodeInput, NodeTranslationInput};
use crate::entities::{
    body, node, node_translation, orchestration_audit_log, orchestration_operation,
};
use crate::error::{ContentError, ContentResult};
use crate::services::orchestration_mapping::{
    map_post_to_topic_input, map_topic_to_post_input, stamp_audit_metadata, AuditStamp, KIND_POST,
    KIND_TOPIC,
};
use crate::NodeService;

const COMMENT_KINDS: [&str; 2] = ["forum_reply", "comment"];

#[derive(Debug, Clone)]
pub struct PromoteTopicToPostInput {
    pub topic_id: Uuid,
    pub locale: String,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct DemotePostToTopicInput {
    pub post_id: Uuid,
    pub locale: String,
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

#[derive(Debug, Clone)]
pub struct OrchestrationResult {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub moved_comments: u64,
}

pub struct ContentOrchestrationService {
    node_service: NodeService,
}

impl ContentOrchestrationService {
    pub fn new(node_service: NodeService) -> Self {
        Self { node_service }
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

        let txn = self.node_service.db().begin().await?;
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

        let topic = self
            .find_node_in_tx(&txn, tenant_id, input.topic_id, KIND_TOPIC)
            .await?;
        let (translations, bodies, effective_locale) = self
            .extract_locale_payload_in_tx(&txn, topic.id, &input.locale)
            .await?;

        let create_input = map_topic_to_post_input(&topic, translations, bodies);
        let post_id = self
            .node_service
            .create_node_in_tx(&txn, tenant_id, security.clone(), create_input)
            .await?;

        let moved_comments = self
            .rebind_children_in_tx(&txn, tenant_id, topic.id, post_id, None)
            .await?;

        self.mark_source_metadata(
            &txn,
            topic,
            "topic.promoted_to_post",
            post_id,
            security.user_id,
            input.reason.as_deref(),
        )
        .await?;

        self.mark_target_cross_link(&txn, post_id, input.topic_id)
            .await?;

        self.node_service
            .event_bus()
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TopicPromotedToPost {
                    topic_id: input.topic_id,
                    post_id,
                    moved_comments,
                    locale: effective_locale.clone(),
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: input.topic_id,
            target_id: post_id,
            moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            tenant_id,
            "promote_topic_to_post",
            &input.idempotency_key,
            security.user_id,
            &result,
            json!({"locale": effective_locale, "reason": input.reason}),
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

        let txn = self.node_service.db().begin().await?;
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

        let post = self
            .find_node_in_tx(&txn, tenant_id, input.post_id, KIND_POST)
            .await?;
        let (translations, bodies, effective_locale) = self
            .extract_locale_payload_in_tx(&txn, post.id, &input.locale)
            .await?;

        let create_input = map_post_to_topic_input(&post, translations, bodies);
        let topic_id = self
            .node_service
            .create_node_in_tx(&txn, tenant_id, security.clone(), create_input)
            .await?;

        let moved_comments = self
            .rebind_children_in_tx(&txn, tenant_id, post.id, topic_id, None)
            .await?;

        self.mark_source_metadata(
            &txn,
            post,
            "post.demoted_to_topic",
            topic_id,
            security.user_id,
            input.reason.as_deref(),
        )
        .await?;

        self.mark_target_cross_link(&txn, topic_id, input.post_id)
            .await?;

        self.node_service
            .event_bus()
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::PostDemotedToTopic {
                    post_id: input.post_id,
                    topic_id,
                    moved_comments,
                    locale: effective_locale.clone(),
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: input.post_id,
            target_id: topic_id,
            moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            tenant_id,
            "demote_post_to_topic",
            &input.idempotency_key,
            security.user_id,
            &result,
            json!({"locale": effective_locale, "reason": input.reason}),
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

        let txn = self.node_service.db().begin().await?;
        if let Some(existing) = self
            .fetch_idempotent_result(&txn, tenant_id, "split_topic", &input.idempotency_key)
            .await?
        {
            txn.rollback().await?;
            return Ok(existing);
        }

        let topic = self
            .find_node_in_tx(&txn, tenant_id, input.topic_id, KIND_TOPIC)
            .await?;

        let (mut translations, bodies, effective_locale) = self
            .extract_locale_payload_in_tx(&txn, topic.id, &input.locale)
            .await?;

        if let Some(primary) = translations
            .iter_mut()
            .find(|tr| tr.locale == effective_locale)
        {
            primary.title = Some(input.new_title.clone());
            primary.slug = Some(slug::slugify(&input.new_title));
        }

        let new_topic_id = self
            .node_service
            .create_node_in_tx(
                &txn,
                tenant_id,
                security.clone(),
                CreateNodeInput {
                    kind: KIND_TOPIC.to_string(),
                    status: Some(topic.status.clone()),
                    parent_id: topic.parent_id,
                    author_id: topic.author_id,
                    category_id: topic.category_id,
                    position: Some(topic.position + 1),
                    depth: Some(topic.depth),
                    reply_count: Some(0),
                    metadata: topic.metadata.clone(),
                    translations,
                    bodies,
                },
            )
            .await?;

        let moved_comments = self
            .rebind_children_in_tx(
                &txn,
                tenant_id,
                topic.id,
                new_topic_id,
                Some(input.reply_ids.clone()),
            )
            .await?;

        self.mark_source_metadata(
            &txn,
            topic.clone(),
            "topic.split_into",
            new_topic_id,
            security.user_id,
            input.reason.as_deref(),
        )
        .await?;

        self.mark_target_cross_link(&txn, new_topic_id, topic.id)
            .await?;

        self.node_service
            .event_bus()
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TopicSplit {
                    source_topic_id: topic.id,
                    target_topic_id: new_topic_id,
                    moved_comment_ids: input.reply_ids.clone(),
                    moved_comments,
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: topic.id,
            target_id: new_topic_id,
            moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            tenant_id,
            "split_topic",
            &input.idempotency_key,
            security.user_id,
            &result,
            json!({
                "locale": effective_locale,
                "reason": input.reason,
                "reply_ids": input.reply_ids,
                "new_title": input.new_title,
            }),
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

        let txn = self.node_service.db().begin().await?;
        if let Some(existing) = self
            .fetch_idempotent_result(&txn, tenant_id, "merge_topics", &input.idempotency_key)
            .await?
        {
            txn.rollback().await?;
            return Ok(existing);
        }

        let target = self
            .find_node_in_tx(&txn, tenant_id, input.target_topic_id, KIND_TOPIC)
            .await?;

        let mut moved_comments: u64 = 0;

        for source_topic_id in input.source_topic_ids.clone() {
            let source = self
                .find_node_in_tx(&txn, tenant_id, source_topic_id, KIND_TOPIC)
                .await?;

            moved_comments += self
                .rebind_children_in_tx(&txn, tenant_id, source.id, target.id, None)
                .await?;

            self.mark_source_metadata(
                &txn,
                source,
                "topic.merged_into",
                target.id,
                security.user_id,
                input.reason.as_deref(),
            )
            .await?;
        }

        self.mark_target_cross_link(&txn, target.id, target.id)
            .await?;

        self.node_service
            .event_bus()
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::TopicsMerged {
                    target_topic_id: target.id,
                    moved_comments,
                    reason: input.reason.clone(),
                },
            )
            .await?;

        let result = OrchestrationResult {
            source_id: target.id,
            target_id: target.id,
            moved_comments,
        };

        self.persist_orchestration_record(
            &txn,
            tenant_id,
            "merge_topics",
            &input.idempotency_key,
            security.user_id,
            &result,
            json!({"reason": input.reason, "source_topic_ids": input.source_topic_ids}),
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
        match validator.validate_input(value) {
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
        tenant_id: Uuid,
        operation: &str,
        idempotency_key: &str,
        actor_id: Option<Uuid>,
        result: &OrchestrationResult,
        payload: Value,
    ) -> ContentResult<()> {
        let now = Utc::now();

        orchestration_operation::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            operation: Set(operation.to_string()),
            idempotency_key: Set(idempotency_key.to_string()),
            source_id: Set(result.source_id),
            target_id: Set(result.target_id),
            moved_comments: Set(result.moved_comments as i64),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        orchestration_audit_log::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            operation: Set(operation.to_string()),
            idempotency_key: Set(idempotency_key.to_string()),
            actor_id: Set(actor_id),
            source_id: Set(result.source_id),
            target_id: Set(result.target_id),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        Ok(())
    }

    async fn extract_locale_payload_in_tx(
        &self,
        txn: &DatabaseTransaction,
        node_id: Uuid,
        locale: &str,
    ) -> ContentResult<(Vec<NodeTranslationInput>, Vec<BodyInput>, String)> {
        let translations = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .all(txn)
            .await?;

        let effective_locale = if translations.iter().any(|tr| tr.locale == locale) {
            locale.to_string()
        } else if translations.iter().any(|tr| tr.locale == "en") {
            "en".to_string()
        } else if let Some(first) = translations.first() {
            first.locale.clone()
        } else {
            return Err(ContentError::Validation(format!(
                "Node {node_id} has no translations for locale resolution"
            )));
        };

        let bodies = body::Entity::find()
            .filter(body::Column::NodeId.eq(node_id))
            .all(txn)
            .await?;

        if bodies.is_empty() {
            return Err(ContentError::Validation(format!(
                "Node {node_id} has no bodies for locale resolution"
            )));
        }

        let translations = translations
            .into_iter()
            .map(|tr| NodeTranslationInput {
                locale: tr.locale,
                title: tr.title,
                slug: tr.slug,
                excerpt: tr.excerpt,
            })
            .collect();

        let bodies = bodies
            .into_iter()
            .map(|b| BodyInput {
                locale: b.locale,
                body: b.body,
                format: Some(b.format),
            })
            .collect();

        Ok((translations, bodies, effective_locale))
    }

    async fn find_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        expected_kind: &str,
    ) -> ContentResult<node::Model> {
        let model = node::Entity::find_by_id(node_id)
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::DeletedAt.is_null())
            .one(txn)
            .await?
            .ok_or(ContentError::NodeNotFound(node_id))?;

        if model.kind != expected_kind {
            return Err(ContentError::Validation(format!(
                "Node {node_id} must have kind '{expected_kind}', got '{}'",
                model.kind
            )));
        }

        Ok(model)
    }

    async fn rebind_children_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        from_parent_id: Uuid,
        to_parent_id: Uuid,
        limit_ids: Option<Vec<Uuid>>,
    ) -> ContentResult<u64> {
        let mut query = node::Entity::find()
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::ParentId.eq(from_parent_id))
            .filter(node::Column::Kind.is_in(COMMENT_KINDS));

        if let Some(limit_ids) = &limit_ids {
            query = query.filter(node::Column::Id.is_in(limit_ids.clone()));
        }

        let children = query.all(txn).await?;
        let moved = children.len() as u64;

        for child in children {
            let previous_version = child.version;
            let mut active: node::ActiveModel = child.into();
            active.parent_id = Set(Some(to_parent_id));
            active.updated_at = Set(Utc::now().into());
            active.version = Set(previous_version + 1);
            active.update(txn).await?;
        }

        Ok(moved)
    }

    async fn mark_source_metadata(
        &self,
        txn: &DatabaseTransaction,
        source: node::Model,
        op: &str,
        linked_id: Uuid,
        actor_id: Option<Uuid>,
        reason: Option<&str>,
    ) -> ContentResult<()> {
        let mut active: node::ActiveModel = source.clone().into();
        let mut metadata: Value = source.metadata;
        stamp_audit_metadata(
            &mut metadata,
            AuditStamp {
                operation: op,
                actor_id,
                reason,
            },
        );
        let mut obj = metadata.as_object().cloned().unwrap_or_default();
        obj.insert(
            "linked_node_id".to_string(),
            Value::String(linked_id.to_string()),
        );

        active.metadata = Set(Value::Object(obj));
        active.updated_at = Set(Utc::now().into());
        active.version = Set(source.version + 1);
        active.update(txn).await?;
        Ok(())
    }

    async fn mark_target_cross_link(
        &self,
        txn: &DatabaseTransaction,
        target_id: Uuid,
        canonical_id: Uuid,
    ) -> ContentResult<()> {
        let target = node::Entity::find_by_id(target_id)
            .one(txn)
            .await?
            .ok_or(ContentError::NodeNotFound(target_id))?;

        let mut active: node::ActiveModel = target.clone().into();
        let mut metadata: Value = target.metadata;
        let mut obj = metadata.as_object().cloned().unwrap_or_default();
        obj.insert(
            "canonical_node_id".to_string(),
            Value::String(canonical_id.to_string()),
        );
        active.metadata = Set(Value::Object(obj));
        active.updated_at = Set(Utc::now().into());
        active.version = Set(target.version + 1);
        active.update(txn).await?;
        Ok(())
    }
}
