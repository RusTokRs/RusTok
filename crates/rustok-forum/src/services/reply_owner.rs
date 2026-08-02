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
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{CreateReplyInput, ReplyResponse, UpdateReplyInput};
use crate::entities::{forum_reply, forum_reply_body, forum_solution};
use crate::error::{ForumError, ForumResult};
use crate::mentions::ForumContentTarget;
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category::CategoryService;
use super::mention_relation::MentionRelationService;
use super::projection_invalidation::publish_forum_category_projection_in_tx;
use super::rbac::{enforce_owned_scope, enforce_scope};
use super::reply;
use super::topic_owner::TopicService;
use super::user_stats::UserStatsService;

/// Public owner service for reply commands.
///
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
        self.create_command(tenant_id, security, topic_id, input.into())
            .await
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
            .update_with_inline_relations(tenant_id, reply_id, security, input.into())
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
        if reply.status == ReplyStatus::Approved {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic.category_id,
            )
            .await?;
        }

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

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
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
