use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, Statement, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_content::{normalize_locale_code, resolve_by_locale_with_fallback};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{ListRepliesFilter, ReplyListItem, ReplyResponse, bounded_forum_read_limit};
use crate::entities::{
    forum_reply, forum_reply_body, forum_solution, forum_topic_merge_operation,
};
use crate::error::{ForumError, ForumResult};
use crate::mentions::ForumContentTarget;
use crate::state_machine::{ReplyStatus, TopicStatus};
use crate::richtext::project_stored_discussion;
use crate::services::engagement_mode::ForumSettingsProviders;
use crate::services::vote::{VoteService, VoteSummary};

use super::category::CategoryService;
use super::category_lifecycle::{ensure_category_restore_target_is_active_in_tx, lock_category_tree_in_tx};
use super::reply_create_audience_authorization::ForumReplyCreateAudienceAuthorizationService;
use super::topic_reply_create_audience::lock_topic_reply_create_audience_in_tx;
use super::mention_relation::MentionRelationService;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
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
    pub(crate) solution_removed: bool,
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

    pub fn with_settings_providers(mut self, settings: ForumSettingsProviders) -> Self {
        self.inner = self.inner.with_settings_providers(settings);
        self
    }

    #[instrument(skip(self, security))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
    ) -> ForumResult<ReplyResponse> {
        self.get_with_locale_fallback(tenant_id, security, reply_id, locale, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyResponse> {
        self.inner
            .get_with_locale_fallback(tenant_id, security, reply_id, locale, fallback_locale)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        self.inner
            .list_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
            )
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_response_for_topic_by_statuses_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        self.inner
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let existing = self.find_reply(tenant_id, reply_id).await?;
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
        if outcome.was_public || outcome.solution_removed {
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                outcome.topic_id,
            )
            .await?;
        }

        txn.commit().await?;
        Ok(())
    }

    /// Restore a previously soft-deleted reply from its immutable delete snapshot.
    ///
    /// Deleted remains terminal in the ordinary state machine; restore is an
    /// explicit lifecycle inverse and does not loosen normal moderation transitions.
    #[instrument(skip(self, security))]
    pub async fn restore(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Manage)?;

        let initial_reply = self.inner.find_reply(tenant_id, reply_id).await?;
        let initial_topic_id = initial_reply.topic_id;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let topic =
            TopicService::find_topic_for_update_in_tx(&txn, tenant_id, initial_topic_id).await?;
        let reply =
            Self::find_reply_for_update_in_tx(&txn, tenant_id, reply_id).await?;
        if reply.topic_id != initial_topic_id {
            return Err(ForumError::TopicUpdateConflict(initial_topic_id));
        }
        if reply.status != ReplyStatus::Deleted {
            return Err(ForumError::ReplyRestoreUnavailable(reply_id));
        }
        if topic.status == TopicStatus::Archived
            && forum_topic_merge_operation::Entity::find()
                .filter(forum_topic_merge_operation::Column::TenantId.eq(tenant_id))
                .filter(forum_topic_merge_operation::Column::SourceTopicId.eq(topic.id))
                .one(&txn)
                .await?
                .is_some()
        {
            return Err(ForumError::ReplyRestoreUnavailable(reply_id));
        }
        ensure_category_restore_target_is_active_in_tx(&txn, tenant_id, topic.category_id).await?;
        let snapshot = load_reply_delete_snapshot_in_tx(&txn, tenant_id, reply_id)
            .await?
            .ok_or(ForumError::ReplyRestoreUnavailable(reply_id))?;

        if snapshot.solution_marked_at.is_some()
            && forum_solution::Entity::find()
                .filter(forum_solution::Column::TenantId.eq(tenant_id))
                .filter(forum_solution::Column::TopicId.eq(topic.id))
                .one(&txn)
                .await?
                .is_some()
        {
            return Err(ForumError::ReplyRestoreUnavailable(reply_id));
        }

        restore_reply_from_delete_snapshot_in_tx(&txn, tenant_id, &reply, &snapshot).await?;

        if snapshot.previous_status == ReplyStatus::Approved {
            TopicService::adjust_reply_count_in_tx(&txn, tenant_id, reply.topic_id, 1).await?;
            CategoryService::adjust_counters_in_tx(
                &txn,
                tenant_id,
                topic.category_id,
                0,
                1,
            )
            .await?;
            UserStatsService::adjust_reply_count_in_tx(
                &txn,
                tenant_id,
                snapshot.author_id,
                1,
            )
            .await?;
        }

        if let Some(marked_at) = snapshot.solution_marked_at.as_deref() {
            let marked_at = marked_at
                .parse::<chrono::DateTime<chrono::FixedOffset>>()
                .map_err(|_| ForumError::ReplyRestoreUnavailable(reply_id))?;
            restore_reply_solution_in_tx(
                &txn,
                tenant_id,
                topic.id,
                reply.id,
                snapshot.solution_marked_by_user_id,
                marked_at,
            )
            .await?;
            UserStatsService::adjust_solution_count_in_tx(
                &txn,
                tenant_id,
                snapshot.author_id,
                1,
            )
            .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumReplyStatusChanged {
                    reply_id,
                    topic_id: topic.id,
                    old_status: ReplyStatus::Deleted.to_string(),
                    new_status: snapshot.previous_status.to_string(),
                    moderator_id: security.user_id,
                },
            )
            .await?;

        if snapshot.previous_status == ReplyStatus::Approved {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic.category_id,
            )
            .await?;
        }
        if snapshot.previous_status == ReplyStatus::Approved || snapshot.solution_marked_at.is_some() {
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic.id,
            )
            .await?;
        }

        clear_reply_delete_snapshot_in_tx(&txn, tenant_id, reply_id).await?;

        txn.commit().await?;
        Ok(())
    }

    /// Applies the complete Forum-owned reply removal mutation inside an
    /// existing owner transaction.
    ///
    /// This is the single state path for soft-delete/tombstone capture,
    /// accepted-solution cleanup and public/author/solution accounting. It does
    /// not perform authorization or publish events; the caller must publish the
    /// established `ForumReplyStatusChanged`, category projection, and topic
    /// projection in this same transaction using the returned facts.
    pub(crate) async fn remove_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<ReplyRemovalOutcome> {
        // Keep reply deletion behind the same category-tree lifecycle boundary as
        // topic delete/restore and category archive/restore. This prevents a reply
        // counter mutation from racing a concurrent category lifecycle decision.
        lock_category_tree_in_tx(txn, tenant_id).await?;
        let initial_reply = Self::find_reply_in_tx(txn, tenant_id, reply_id).await?;
        let initial_topic_id = initial_reply.topic_id;

        // All reply-moving owners lock the topic before reply rows. Keep deletion in
        // the same topic -> reply order so concurrent lifecycle mutations cannot
        // deadlock on inverted row-lock acquisition.
        let topic = TopicService::find_topic_for_update_in_tx(txn, tenant_id, initial_topic_id).await?;
        let reply = reply::ReplyService::find_reply_for_update_in_tx(txn, tenant_id, reply_id).await?;
        if reply.topic_id != initial_topic_id {
            return Err(ForumError::TopicUpdateConflict(initial_topic_id));
        }
        if reply.status == ReplyStatus::Deleted {
            return Err(ForumError::ReplyDeleted);
        }
        reply.status.validate_transition(&ReplyStatus::Deleted)?;
        let solution = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(reply.topic_id))
            .one(txn)
            .await?;
        let solution_for_snapshot = solution
            .as_ref()
            .filter(|solution| solution.reply_id == reply_id);
        let solution_removed = solution_for_snapshot.is_some();

        record_reply_delete_snapshot_in_tx(
            txn,
            tenant_id,
            &reply,
            solution_for_snapshot,
        )
        .await?;

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
            solution_removed,
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

    pub(crate) async fn find_reply_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        reply::ReplyService::find_reply_for_update_in_tx(txn, tenant_id, reply_id).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        status: ReplyStatus,
    ) -> ForumResult<forum_reply::Model> {
        Self::set_status_in_tx(txn, tenant_id, reply_id, status).await
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

#[derive(Clone, Debug)]
struct ReplyDeleteSnapshot {
    topic_id: Uuid,
    author_id: Option<Uuid>,
    previous_status: ReplyStatus,
    solution_marked_by_user_id: Option<Uuid>,
    solution_marked_at: Option<String>,
}

async fn record_reply_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply: &forum_reply::Model,
    solution: Option<&forum_solution::Model>,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_reply_delete_snapshots (
                tenant_id, reply_id, topic_id, author_id,
                previous_status, solution_marked_by_user_id, solution_marked_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                reply.id.into(),
                reply.topic_id.into(),
                reply.author_id.into(),
                reply.status.to_string().into(),
                solution.and_then(|value| value.marked_by_user_id).into(),
                solution.map(|value| value.marked_at.to_rfc3339()).into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_reply_delete_snapshots (
                tenant_id, reply_id, topic_id, author_id,
                previous_status, solution_marked_by_user_id, solution_marked_at, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                reply.id.into(),
                reply.topic_id.into(),
                reply.author_id.into(),
                reply.status.to_string().into(),
                solution.and_then(|value| value.marked_by_user_id).into(),
                solution.map(|value| value.marked_at.to_rfc3339()).into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply delete snapshots do not support database backend {backend:?}"
            )))
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn load_reply_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<Option<ReplyDeleteSnapshot>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT topic_id, author_id, previous_status,
                   solution_marked_by_user_id, solution_marked_at
            FROM forum_reply_delete_snapshots
            WHERE tenant_id = $1 AND reply_id = $2
            "#,
            vec![tenant_id.into(), reply_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT topic_id, author_id, previous_status,
                   solution_marked_by_user_id, solution_marked_at
            FROM forum_reply_delete_snapshots
            WHERE tenant_id = ? AND reply_id = ?
            "#,
            vec![tenant_id.into(), reply_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )))
        }
    };
    let Some(row) = txn.query_one_raw(statement).await? else {
        return Ok(None);
    };
    let status_value: String = row.try_get("", "previous_status")?;
    let previous_status = ReplyStatus::from_str_value(&status_value)
        .filter(|status| *status != ReplyStatus::Deleted)
        .ok_or(ForumError::ReplyRestoreUnavailable(reply_id))?;
    Ok(Some(ReplyDeleteSnapshot {
        topic_id: row.try_get("", "topic_id")?,
        author_id: row.try_get("", "author_id")?,
        previous_status,
        solution_marked_by_user_id: row.try_get("", "solution_marked_by_user_id")?,
        solution_marked_at: row.try_get("", "solution_marked_at")?,
    }))
}

async fn restore_reply_from_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply: &forum_reply::Model,
    snapshot: &ReplyDeleteSnapshot,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            UPDATE forum_replies
            SET status = $3, deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1 AND topic_id = $2 AND id = $4 AND deleted_at IS NOT NULL
            "#,
            vec![
                tenant_id.into(),
                snapshot.topic_id.into(),
                snapshot.previous_status.to_string().into(),
                reply.id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE forum_replies
            SET status = ?, deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND topic_id = ? AND id = ? AND deleted_at IS NOT NULL
            "#,
            vec![
                snapshot.previous_status.to_string().into(),
                tenant_id.into(),
                snapshot.topic_id.into(),
                reply.id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )))
        }
    };
    let result = txn.execute_raw(statement).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::ReplyRestoreUnavailable(reply.id));
    }
    Ok(())
}

async fn restore_reply_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    marked_by_user_id: Option<Uuid>,
    marked_at: chrono::DateTime<chrono::FixedOffset>,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_solutions (
                tenant_id, topic_id, reply_id, marked_by_user_id, marked_at
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                reply_id.into(),
                marked_by_user_id.into(),
                marked_at.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_solutions (
                tenant_id, topic_id, reply_id, marked_by_user_id, marked_at
            )
            VALUES (?, ?, ?, ?, ?)
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                reply_id.into(),
                marked_by_user_id.into(),
                marked_at.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )))
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn clear_reply_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply_id: Uuid,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM forum_reply_delete_snapshots WHERE tenant_id = $1 AND reply_id = $2",
            vec![tenant_id.into(), reply_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "DELETE FROM forum_reply_delete_snapshots WHERE tenant_id = ? AND reply_id = ?",
            vec![tenant_id.into(), reply_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply restore does not support database backend {backend:?}"
            )))
        }
    };
    txn.execute_raw(statement).await?;
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
