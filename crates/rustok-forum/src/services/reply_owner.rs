use std::ops::Deref;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, Statement, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::{prepare_content_payload, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{CreateReplyInput, ReplyResponse, UpdateReplyInput};
use crate::entities::{forum_reply, forum_reply_body, forum_solution};
use crate::error::{ForumError, ForumResult};
use crate::mentions::ForumContentTarget;
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category::CategoryService;
use super::mention_relation::MentionRelationService;
use super::rbac::{enforce_owned_scope, enforce_scope};
use super::reply;
use super::topic_owner::TopicService;
use super::user_stats::UserStatsService;

/// Public owner service for reply commands.
///
/// The wrapped persistence service remains available as a compatibility path.
/// Root-service lifecycle decisions live here so database triggers are
/// invariant guards rather than the primary workflow engine.
pub struct ReplyService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    relations: MentionRelationService,
    inner: reply::ReplyService,
}

impl ReplyService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: reply::ReplyService::new(db.clone(), event_bus.clone()),
            relations: MentionRelationService::new(db.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        enforce_scope(&security, Resource::ForumReplies, Action::Create)?;
        let locale = normalize_locale(&input.locale)?;
        let prepared_body = prepare_content_payload(
            Some(&input.content_format),
            Some(&input.content),
            input.content_json.as_ref(),
            &locale,
            "Reply content",
        )
        .map_err(ForumError::Validation)?;
        let reply_id = Uuid::new_v4();
        let prepared_relations = self
            .relations
            .prepare(
                tenant_id,
                ForumContentTarget::reply(reply_id),
                &locale,
                &prepared_body.body,
                &prepared_body.format,
                &security,
                std::iter::empty(),
            )
            .await?;

        let txn = self.db.begin().await?;
        let topic = TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
        match topic.status {
            TopicStatus::Closed => return Err(ForumError::TopicClosed),
            TopicStatus::Archived => return Err(ForumError::TopicArchived),
            TopicStatus::Open => {}
        }
        if topic.is_locked {
            return Err(ForumError::TopicLocked);
        }

        let category =
            CategoryService::find_category_in_tx(&txn, tenant_id, topic.category_id).await?;

        if let Some(parent_reply_id) = input.parent_reply_id {
            let parent =
                reply::ReplyService::find_reply_in_tx(&txn, tenant_id, parent_reply_id).await?;
            if parent.topic_id != topic_id {
                return Err(ForumError::Validation(
                    "Parent reply belongs to another topic".to_string(),
                ));
            }
            if parent.status == ReplyStatus::Deleted {
                return Err(ForumError::Validation(
                    "Deleted reply cannot be used as a parent".to_string(),
                ));
            }
        }

        let position = allocate_reply_position_in_tx(&txn, tenant_id, topic_id).await?;
        let status = if category.moderated {
            ReplyStatus::Pending
        } else {
            ReplyStatus::Approved
        };
        let now = Utc::now();

        forum_reply::ActiveModel {
            id: Set(reply_id),
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            author_id: Set(security.user_id),
            parent_reply_id: Set(input.parent_reply_id),
            status: Set(status),
            position: Set(position),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(reply_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            body: Set(prepared_body.body),
            body_format: Set(prepared_body.format),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        self.relations
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        if status == ReplyStatus::Approved {
            TopicService::adjust_reply_count_in_tx(&txn, tenant_id, topic_id, 1).await?;
            CategoryService::adjust_counters_in_tx(&txn, tenant_id, topic.category_id, 0, 1)
                .await?;
            UserStatsService::adjust_reply_count_in_tx(&txn, tenant_id, security.user_id, 1)
                .await?;

            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::ForumTopicReplied {
                        topic_id,
                        reply_id,
                        author_id: security.user_id,
                    },
                )
                .await?;
        }

        txn.commit().await?;
        self.inner.get(tenant_id, security, reply_id, &locale).await
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        self.inner
            .update_with_relations(tenant_id, reply_id, security, input)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let existing = self.inner.find_reply(tenant_id, reply_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Delete,
            existing.author_id,
        )?;

        let txn = self.db.begin().await?;
        claim_reply_delete_in_tx(&txn, tenant_id, reply_id).await?;
        let reply = reply::ReplyService::find_reply_in_tx(&txn, tenant_id, reply_id).await?;
        if reply.status == ReplyStatus::Deleted {
            return Err(ForumError::ReplyDeleted);
        }
        reply.status.validate_transition(&ReplyStatus::Deleted)?;

        let topic = TopicService::find_topic_in_tx(&txn, tenant_id, reply.topic_id).await?;
        let solution_removed = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(reply.topic_id))
            .one(&txn)
            .await?
            .is_some_and(|solution| solution.reply_id == reply_id);

        redact_reply_body_in_tx(&txn, tenant_id, reply_id).await?;
        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::ReplyId.eq(reply_id))
            .exec(&txn)
            .await?;
        mark_reply_deleted_in_tx(&txn, tenant_id, reply_id).await?;

        if reply.status == ReplyStatus::Approved {
            TopicService::adjust_reply_count_in_tx(&txn, tenant_id, reply.topic_id, -1).await?;
            CategoryService::adjust_counters_in_tx(&txn, tenant_id, topic.category_id, 0, -1)
                .await?;
            UserStatsService::adjust_reply_count_in_tx(&txn, tenant_id, reply.author_id, -1)
                .await?;
        }
        if solution_removed {
            UserStatsService::adjust_solution_count_in_tx(&txn, tenant_id, reply.author_id, -1)
                .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id: reply.topic_id,
                    old_status: reply.status.to_string(),
                    new_status: ReplyStatus::Deleted.to_string(),
                    moderator_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;
        Ok(())
    }

    pub(crate) async fn find_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        self.inner.find_reply(tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        reply::ReplyService::find_reply_in_tx(txn, tenant_id, reply_id).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        status: ReplyStatus,
    ) -> ForumResult<forum_reply::Model> {
        reply::ReplyService::set_status_in_tx(txn, tenant_id, reply_id, status).await
    }
}

impl Deref for ReplyService {
    type Target = reply::ReplyService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

async fn allocate_reply_position_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<i64> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            // FORUM-07/FORUM-08B PostgreSQL triggers replace this provisional
            // value with the monotonic per-topic allocation before INSERT.
            Ok(1)
        }
        DatabaseBackend::Sqlite => {
            let row = txn
                .query_one(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    format!(
                        "UPDATE forum_topics \
                         SET next_reply_position = next_reply_position + 1 \
                         WHERE tenant_id = '{tenant_id}' AND id = '{topic_id}' \
                         RETURNING next_reply_position - 1 AS position"
                    ),
                ))
                .await?
                .ok_or(ForumError::TopicNotFound(topic_id))?;
            Ok(row.try_get("", "position")?)
        }
        backend => Err(ForumError::Validation(format!(
            "Unsupported forum database backend: {backend:?}"
        ))),
    }
}

async fn claim_reply_delete_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<()> {
    let result = txn
        .execute_unprepared(&format!(
            "UPDATE forum_replies \
             SET updated_at = updated_at \
             WHERE tenant_id = '{tenant_id}' AND id = '{reply_id}' AND deleted_at IS NULL"
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::ReplyDeleted);
    }
    Ok(())
}

async fn redact_reply_body_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<()> {
    txn.execute_unprepared(&format!(
        "UPDATE forum_reply_bodies \
         SET body = '[deleted]', body_format = 'markdown', updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = '{tenant_id}' AND reply_id = '{reply_id}'"
    ))
    .await?;
    Ok(())
}

async fn mark_reply_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<()> {
    let result = txn
        .execute_unprepared(&format!(
            "UPDATE forum_replies \
             SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = '{tenant_id}' AND id = '{reply_id}' AND deleted_at IS NULL"
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::ReplyDeleted);
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}
