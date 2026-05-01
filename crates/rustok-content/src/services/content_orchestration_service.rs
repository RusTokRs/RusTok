use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use serde_json::{json, Value};
use uuid::Uuid;

use rustok_core::{
    Action, DomainEvent, InputValidator, PermissionScope, Resource, SecurityContext,
    ValidationResult,
};
use rustok_outbox::TransactionalEventBus;

use crate::entities::{canonical_url, orchestration_audit_log, orchestration_operation, url_alias};
use crate::error::{ContentError, ContentResult};
use crate::normalize_locale_code;

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

    fn normalize_target_kind(&self, target_kind: &str) -> ContentResult<String> {
        let normalized = target_kind.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(ContentError::validation("target_kind must not be empty"));
        }
        if normalized.len() > 64 {
            return Err(ContentError::validation("target_kind must be <= 64 chars"));
        }
        if !normalized
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
        {
            return Err(ContentError::validation(
                "target_kind may contain only lowercase ascii letters, digits, `_`, or `.`",
            ));
        }
        Ok(normalized)
    }

    fn normalize_route_url(&self, field: &str, value: &str) -> ContentResult<String> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(ContentError::validation(format!(
                "{field} must not be empty"
            )));
        }
        if normalized.len() > 512 {
            return Err(ContentError::validation(format!(
                "{field} must be <= 512 chars"
            )));
        }
        if !normalized.starts_with('/') {
            return Err(ContentError::validation(format!(
                "{field} must start with `/`"
            )));
        }
        if normalized.chars().any(char::is_whitespace) || normalized.contains("://") {
            return Err(ContentError::validation(format!(
                "{field} must be a relative route without whitespace or scheme"
            )));
        }
        Ok(normalized.to_string())
    }

    async fn apply_canonical_url_mutations(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        updates: &[CanonicalUrlMutation],
    ) -> ContentResult<()> {
        for update in updates {
            let target_kind = self.normalize_target_kind(&update.target_kind)?;
            let locale = normalize_locale_code(&update.locale)
                .ok_or_else(|| ContentError::validation("canonical locale must not be empty"))?;
            let canonical_route =
                self.normalize_route_url("canonical_url", update.canonical_url.as_str())?;

            let mut alias_urls = BTreeSet::new();
            for alias in &update.alias_urls {
                let alias = self.normalize_route_url("alias_url", alias)?;
                if alias != canonical_route {
                    alias_urls.insert(alias);
                }
            }

            for retired in &update.retired_targets {
                let retired_kind = self.normalize_target_kind(&retired.target_kind)?;
                let retired_locale = normalize_locale_code(&retired.locale).ok_or_else(|| {
                    ContentError::validation("retired canonical locale must not be empty")
                })?;

                let retired_canonical = canonical_url::Entity::find()
                    .filter(canonical_url::Column::TenantId.eq(tenant_id))
                    .filter(canonical_url::Column::TargetKind.eq(retired_kind.clone()))
                    .filter(canonical_url::Column::TargetId.eq(retired.target_id))
                    .filter(canonical_url::Column::Locale.eq(retired_locale.clone()))
                    .one(txn)
                    .await?;

                if let Some(retired_canonical) = retired_canonical {
                    if retired_canonical.canonical_url != canonical_route {
                        alias_urls.insert(retired_canonical.canonical_url.clone());
                    }
                    canonical_url::Entity::delete_by_id(retired_canonical.id)
                        .exec(txn)
                        .await?;
                }

                let retired_aliases = url_alias::Entity::find()
                    .filter(url_alias::Column::TenantId.eq(tenant_id))
                    .filter(url_alias::Column::TargetKind.eq(retired_kind))
                    .filter(url_alias::Column::TargetId.eq(retired.target_id))
                    .filter(url_alias::Column::Locale.eq(retired_locale))
                    .all(txn)
                    .await?;
                for retired_alias in retired_aliases {
                    if retired_alias.alias_url != canonical_route {
                        alias_urls.insert(retired_alias.alias_url.clone());
                    }
                    url_alias::Entity::delete_by_id(retired_alias.id)
                        .exec(txn)
                        .await?;
                }
            }

            let existing_canonical = canonical_url::Entity::find()
                .filter(canonical_url::Column::TenantId.eq(tenant_id))
                .filter(canonical_url::Column::TargetKind.eq(target_kind.clone()))
                .filter(canonical_url::Column::TargetId.eq(update.target_id))
                .filter(canonical_url::Column::Locale.eq(locale.clone()))
                .one(txn)
                .await?;

            let now = Utc::now();
            let mut mapping_changed = false;
            if let Some(existing_canonical) = existing_canonical {
                if existing_canonical.canonical_url != canonical_route {
                    alias_urls.insert(existing_canonical.canonical_url.clone());
                    let mut active: canonical_url::ActiveModel = existing_canonical.into();
                    active.canonical_url = Set(canonical_route.clone());
                    active.updated_at = Set(now.into());
                    active.update(txn).await?;
                    mapping_changed = true;
                }
            } else {
                canonical_url::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    tenant_id: Set(tenant_id),
                    target_kind: Set(target_kind.clone()),
                    target_id: Set(update.target_id),
                    locale: Set(locale.clone()),
                    canonical_url: Set(canonical_route.clone()),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
                mapping_changed = true;
            }

            let alias_urls = alias_urls.into_iter().collect::<Vec<_>>();

            url_alias::Entity::delete_many()
                .filter(url_alias::Column::TenantId.eq(tenant_id))
                .filter(url_alias::Column::Locale.eq(locale.clone()))
                .filter(url_alias::Column::AliasUrl.eq(canonical_route.clone()))
                .exec(txn)
                .await?;

            for alias in &alias_urls {
                let existing_alias = url_alias::Entity::find()
                    .filter(url_alias::Column::TenantId.eq(tenant_id))
                    .filter(url_alias::Column::Locale.eq(locale.clone()))
                    .filter(url_alias::Column::AliasUrl.eq(alias.clone()))
                    .one(txn)
                    .await?;

                if let Some(existing_alias) = existing_alias {
                    let mut active: url_alias::ActiveModel = existing_alias.into();
                    active.target_kind = Set(target_kind.clone());
                    active.target_id = Set(update.target_id);
                    active.canonical_url = Set(canonical_route.clone());
                    active.updated_at = Set(now.into());
                    active.update(txn).await?;
                } else {
                    url_alias::ActiveModel {
                        id: Set(rustok_core::generate_id()),
                        tenant_id: Set(tenant_id),
                        target_kind: Set(target_kind.clone()),
                        target_id: Set(update.target_id),
                        locale: Set(locale.clone()),
                        alias_url: Set(alias.clone()),
                        canonical_url: Set(canonical_route.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(txn)
                    .await?;
                }
            }

            if mapping_changed || !alias_urls.is_empty() {
                self.event_bus
                    .publish_in_tx(
                        txn,
                        tenant_id,
                        actor_id,
                        DomainEvent::CanonicalUrlChanged {
                            target_id: update.target_id,
                            target_kind: target_kind.clone(),
                            locale: locale.clone(),
                            new_canonical_url: canonical_route.clone(),
                            old_urls: alias_urls.clone(),
                        },
                    )
                    .await?;

                if !alias_urls.is_empty() {
                    self.event_bus
                        .publish_in_tx(
                            txn,
                            tenant_id,
                            actor_id,
                            DomainEvent::UrlAliasPurged {
                                target_id: update.target_id,
                                target_kind: target_kind.clone(),
                                locale,
                                urls: alias_urls,
                            },
                        )
                        .await?;
                }
            }
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
