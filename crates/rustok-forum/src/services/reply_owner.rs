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

/// Exact Forum-owned facts produced by the transactional reply removal path.
///
/// The state/counter/solution mutation stays inside `ReplyService`; callers may
/// use these facts only to publish the established owner event/projection in the
/// same transaction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReplyRemovalOutcome {
    pub(crate) topic_id: Uuid,
    pub(crate) category_id: Uuid,
    pub(crate) old_status: ReplyStatus,
    pub(crate) was_public: bool,
}

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
        let outcome = Self::remove_in_tx(&txn, tenant_id, reply_id).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id: outcome.topic_id,
                    old_status: outcome.old_status.to_string(),
                    new_status: ReplyStatus::Deleted.to_string(),
                    moderator_id: security.user_id,
                },
            )
            .await?;
        if outcome.was_public {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                outcome.category_id,
            )
            .await?;
        }

        txn.commit().await?;
        Ok(())
    }

    /// Applies the complete Forum-owned reply removal mutation inside an
    /// existing owner transaction.
    ///
    /// This is the single state path for soft-delete/tombstone capture,
    /// accepted-solution cleanup and public/author/solution accounting. It does
    /// not perform authorization or publish events; the caller must publish the
    /// established `ForumReplyStatusChanged` and category projection in this
    /// same transaction using the returned facts.
    pub(crate) async fn remove_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<ReplyRemovalOutcome> {
        claim_reply_delete_in_tx(txn, tenant_id, reply_id).await?;
        let reply = reply::ReplyService::find_reply_in_tx(txn, tenant_id, reply_id).await?;
        if reply.status == ReplyStatus::Deleted {
            return Err(ForumError::ReplyDeleted);
        }
        reply.status.validate_transition(&ReplyStatus::Deleted)?;

        let topic = TopicService::find_topic_in_tx(txn, tenant_id, reply.topic_id).await?;
        let solution_removed = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(reply.topic_id))
            .one(txn)
            .await?
            .is_some_and(|solution| solution.reply_id == reply_id);

        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::ReplyId.eq(reply_id))
            .exec(txn)
            .await?;
        mark_reply_deleted_in_tx(txn, tenant_id, reply_id).await?;

        let was_public = reply.status == ReplyStatus::Approved;
        if was_public {
            TopicService::adjust_reply_count_in_tx(txn, tenant_id, reply.topic_id, -1).await?;
            CategoryService::adjust_counters_in_tx(txn, tenant_id, topic.category_id, 0, -1)
                .await?;
            UserStatsService::adjust_reply_count_in_tx(txn, tenant_id, reply.author_id, -1).await?;
        }
        if solution_removed {
            UserStatsService::adjust_solution_count_in_tx(txn, tenant_id, reply.author_id, -1)
                .await?;
        }

        Ok(ReplyRemovalOutcome {
            topic_id: reply.topic_id,
            category_id: topic.category_id,
            old_status: reply.status,
            was_public,
        })
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
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE forum_topics \
                     SET next_reply_position = next_reply_position + 1 \
                     WHERE tenant_id = ? AND id = ? \
                     RETURNING next_reply_position - 1 AS position",
                    vec![tenant_id.into(), topic_id.into()],
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
    let statement = tenant_scoped_reply_statement(
        txn.get_database_backend(),
        "UPDATE forum_replies \
         SET updated_at = updated_at \
         WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
        "UPDATE forum_replies \
         SET updated_at = updated_at \
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        tenant_id,
        reply_id,
    )?;
    let result = txn.execute_raw(statement).await?;
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
    let statement = tenant_scoped_reply_statement(
        txn.get_database_backend(),
        "UPDATE forum_replies \
         SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
        "UPDATE forum_replies \
         SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        tenant_id,
        reply_id,
    )?;
    let result = txn.execute_raw(statement).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::ReplyDeleted);
    }
    Ok(())
}

fn tenant_scoped_reply_statement(
    backend: DatabaseBackend,
    sqlite_sql: &str,
    postgres_sql: &str,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<Statement> {
    match backend {
        DatabaseBackend::Postgres => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            postgres_sql,
            vec![tenant_id.into(), reply_id.into()],
        )),
        DatabaseBackend::Sqlite => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sqlite_sql,
            vec![tenant_id.into(), reply_id.into()],
        )),
        backend => Err(ForumError::Validation(format!(
            "Forum reply mutations do not support database backend {backend:?}"
        ))),
    }
}
