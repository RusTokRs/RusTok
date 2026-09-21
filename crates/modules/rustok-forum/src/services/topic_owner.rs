use std::ops::Deref;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, Statement,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::TopicResponse;
use crate::entities::{forum_reply, forum_solution, forum_topic};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};

use self::route_tombstone_visibility::ForumTopicRouteTombstoneVisibilityService;
use super::category::CategoryService;
use super::topic_create_audience_authorization::ForumTopicCreateAudienceAuthorizationService;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::{enforce_owned_scope, enforce_scope};
use super::topic;
use super::topic_route::{
    ForumTopicRouteService, ForumTopicSlugRenameResult, RenameForumTopicSlugInput,
};
use super::user_stats::UserStatsService;

const FORUM_TOPIC_DELETED_ROUTE_REASON: &str = "Topic deleted";

/// Public owner service for topic commands.
///
/// Explicit root-service lifecycle writes happen here. The wrapped persistence
/// service remains a compatibility path, while database triggers provide the
/// final consistency barrier for direct SQL and older deployments.
pub struct TopicService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    inner: topic::TopicService,
}

impl TopicService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: topic::TopicService::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn rename_slug(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: RenameForumTopicSlugInput,
    ) -> ForumResult<ForumTopicSlugRenameResult> {
        let existing = self.inner.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Update,
            existing.author_id,
        )?;

        let txn = self.db.begin().await?;
        let result =
            ForumTopicRouteService::rename_topic_slug_in_tx(&txn, tenant_id, topic_id, &input)
                .await?;
        if result.changed {
            let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
            let mut active: forum_topic::ActiveModel = topic.into();
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await?;
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic_id,
            )
            .await?;
        }
        txn.commit().await?;
        Ok(result)
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let existing = self.inner.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Delete,
            existing.author_id,
        )?;

        let txn = self.db.begin().await?;
        lock_topic_delete_tenant_in_tx(&txn, tenant_id).await?;
        ForumTopicRouteTombstoneVisibilityService::lock_category_scope_in_tx(&txn, tenant_id)
            .await?;
        claim_topic_delete_in_tx(&txn, tenant_id, topic_id).await?;
        ForumTopicRouteTombstoneVisibilityService::lock_topic_audience_scope_in_tx(
            &txn, tenant_id, topic_id,
        )
        .await?;
        let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;

        let public_reply_count = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .count(&txn)
            .await?;
        let public_reply_count = i32::try_from(public_reply_count).map_err(|_| {
            ForumError::Validation("Forum reply count exceeds supported range".to_string())
        })?;

        let solution = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&txn)
            .await?;
        let solution_author_id = match &solution {
            Some(solution) => forum_reply::Entity::find_by_id(solution.reply_id)
                .filter(forum_reply::Column::TenantId.eq(tenant_id))
                .filter(forum_reply::Column::TopicId.eq(topic_id))
                .one(&txn)
                .await?
                .and_then(|reply| reply.author_id),
            None => None,
        };

        record_topic_delete_snapshot_in_tx(
            &txn,
            tenant_id,
            &topic,
            solution.as_ref(),
        )
        .await?;
        ForumTopicRouteTombstoneVisibilityService::record_locked_delete_snapshot_in_tx(
            &txn, tenant_id, &topic,
        )
        .await?;
        ForumTopicRouteService::record_delete_tombstones_in_tx(
            &txn,
            tenant_id,
            topic_id,
            FORUM_TOPIC_DELETED_ROUTE_REASON,
        )
        .await?;

        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;

        mark_topic_thread_deleted_in_tx(&txn, tenant_id, topic_id).await?;
        UserStatsService::decrement_topic_thread_aggregated_in_tx(
            &txn,
            tenant_id,
            topic_id,
            topic.author_id,
            solution_author_id,
        )
        .await?;

        CategoryService::adjust_counters_in_tx(
            &txn,
            tenant_id,
            topic.category_id,
            -1,
            -public_reply_count,
        )
        .await?;

        if topic.status != TopicStatus::Archived {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::ForumTopicStatusChanged {
                        topic_id,
                        old_status: topic.status.to_string(),
                        new_status: TopicStatus::Archived.to_string(),
                        moderator_id: security.user_id,
                    },
                )
                .await?;
        }
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic.category_id,
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    /// Restore a previously soft-deleted topic from its immutable delete snapshot.
    ///
    /// Restore intentionally bypasses the normal reply/topic transition graph:
    /// Deleted is terminal for ordinary moderation commands, while this lifecycle
    /// operation is the explicit inverse of the delete transaction.
    #[instrument(skip(self, security))]
    pub async fn restore(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;

        let txn = self.db.begin().await?;
        lock_topic_delete_tenant_in_tx(&txn, tenant_id).await?;
        ForumTopicRouteTombstoneVisibilityService::lock_category_scope_in_tx(&txn, tenant_id)
            .await?;
        ForumTopicRouteTombstoneVisibilityService::lock_topic_audience_scope_in_tx(
            &txn, tenant_id, topic_id,
        )
        .await?;

        let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
        if !topic_is_deleted_in_tx(&txn, tenant_id, topic_id).await? {
            return Err(ForumError::TopicRestoreUnavailable(topic_id));
        }

        let snapshot = load_topic_delete_snapshot_in_tx(&txn, tenant_id, topic_id)
            .await?
            .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
        let reply_snapshots =
            load_topic_reply_delete_snapshots_in_tx(&txn, tenant_id, topic_id).await?;

        for reply_snapshot in &reply_snapshots {
            let reply = forum_reply::Entity::find_by_id(reply_snapshot.reply_id)
                .filter(forum_reply::Column::TenantId.eq(tenant_id))
                .filter(forum_reply::Column::TopicId.eq(topic_id))
                .one(&txn)
                .await?
                .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
            if reply.status != ReplyStatus::Deleted {
                return Err(ForumError::TopicRestoreUnavailable(topic_id));
            }

            restore_reply_from_delete_snapshot_in_tx(&txn, tenant_id, topic_id, reply_snapshot)
                .await?;

            if reply_snapshot.previous_status == ReplyStatus::Approved {
                UserStatsService::adjust_reply_count_in_tx(
                    &txn,
                    tenant_id,
                    reply_snapshot.author_id,
                    1,
                )
                .await?;
            }
        }

        if let Some(solution_reply_id) = snapshot.solution_reply_id {
            let solution_snapshot = reply_snapshots
                .iter()
                .find(|reply| reply.reply_id == solution_reply_id)
                .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
            if solution_snapshot.previous_status != ReplyStatus::Approved {
                return Err(ForumError::TopicRestoreUnavailable(topic_id));
            }
            let marked_at = snapshot
                .solution_marked_at
                .as_deref()
                .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?
                .parse::<chrono::DateTime<chrono::FixedOffset>>()
                .map_err(|_| ForumError::TopicRestoreUnavailable(topic_id))?;

            restore_solution_in_tx(
                &txn,
                tenant_id,
                topic_id,
                solution_reply_id,
                snapshot.solution_marked_by_user_id,
                marked_at,
            )
            .await?;

            UserStatsService::adjust_solution_count_in_tx(
                &txn,
                tenant_id,
                snapshot.solution_author_id,
                1,
            )
            .await?;
        }

        let public_reply_count =
            count_approved_replies_in_tx(&txn, tenant_id, topic_id).await?;
        if public_reply_count > i32::MAX as i64 {
            return Err(ForumError::Validation(
                "Forum reply count exceeds supported range".to_string(),
            ));
        }

        restore_topic_from_delete_snapshot_in_tx(
            &txn,
            tenant_id,
            topic_id,
            &snapshot,
            public_reply_count as i32,
        )
        .await?;

        UserStatsService::adjust_topic_count_in_tx(&txn, tenant_id, topic.author_id, 1)
            .await?;
        CategoryService::adjust_counters_in_tx(
            &txn,
            tenant_id,
            topic.category_id,
            1,
            public_reply_count as i32,
        )
        .await?;

        if snapshot.previous_status != TopicStatus::Archived {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::ForumTopicStatusChanged {
                        topic_id,
                        old_status: TopicStatus::Archived.to_string(),
                        new_status: snapshot.previous_status.to_string(),
                        moderator_id: security.user_id,
                    },
                )
                .await?;
        }

        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic.category_id,
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    pub(crate) async fn find_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        self.inner.find_topic(tenant_id, topic_id).await
    }

    pub(crate) async fn find_topic_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        topic::TopicService::find_topic_in_tx(txn, tenant_id, topic_id).await
    }

    pub(crate) async fn find_topic_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        topic::TopicService::find_topic_for_update_in_tx(txn, tenant_id, topic_id).await
    }

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        delta: i32,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        topic::TopicService::adjust_reply_count_in_tx(txn, tenant_id, topic_id, delta).await
    }

    pub(crate) async fn set_pinned_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_pinned: bool,
    ) -> ForumResult<()> {
        topic::TopicService::set_pinned_in_tx(txn, tenant_id, topic_id, is_pinned).await
    }

    pub(crate) async fn set_locked_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_locked: bool,
    ) -> ForumResult<()> {
        topic::TopicService::set_locked_in_tx(txn, tenant_id, topic_id, is_locked).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        status: TopicStatus,
    ) -> ForumResult<()> {
        topic::TopicService::set_status_in_tx(txn, tenant_id, topic_id, status).await
    }
}

impl Deref for TopicService {
    type Target = topic::TopicService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Clone, Debug)]
struct TopicDeleteSnapshot {
    previous_status: TopicStatus,
    previous_is_locked: bool,
    solution_reply_id: Option<Uuid>,
    solution_author_id: Option<Uuid>,
    solution_marked_by_user_id: Option<Uuid>,
    solution_marked_at: Option<String>,
}

#[derive(Clone, Debug)]
struct ReplyDeleteSnapshot {
    reply_id: Uuid,
    author_id: Option<Uuid>,
    previous_status: ReplyStatus,
}

async fn record_topic_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic: &forum_topic::Model,
    solution: Option<&forum_solution::Model>,
) -> ForumResult<()> {
    let solution_author_id = match solution {
        Some(solution) => forum_reply::Entity::find_by_id(solution.reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic.id))
            .one(txn)
            .await?
            .and_then(|reply| reply.author_id),
        None => None,
    };

    let solution_marked_at = solution.map(|solution| solution.marked_at.to_rfc3339());
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_delete_snapshots (
                tenant_id, topic_id, previous_status, previous_is_locked,
                solution_reply_id, solution_author_id,
                solution_marked_by_user_id, solution_marked_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                topic.id.into(),
                topic.status.to_string().into(),
                topic.is_locked.into(),
                solution.map(|value| value.reply_id).into(),
                solution_author_id.into(),
                solution.and_then(|value| value.marked_by_user_id).into(),
                solution_marked_at.clone().into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_delete_snapshots (
                tenant_id, topic_id, previous_status, previous_is_locked,
                solution_reply_id, solution_author_id,
                solution_marked_by_user_id, solution_marked_at, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            vec![
                tenant_id.into(),
                topic.id.into(),
                topic.status.to_string().into(),
                topic.is_locked.into(),
                solution.map(|value| value.reply_id).into(),
                solution_author_id.into(),
                solution.and_then(|value| value.marked_by_user_id).into(),
                solution_marked_at.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic delete snapshots do not support database backend {backend:?}"
            )))
        }
    };
    txn.execute_raw(statement).await?;

    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_reply_delete_snapshots (
                tenant_id, topic_id, reply_id, author_id, previous_status
            )
            SELECT tenant_id, topic_id, id, author_id, status
            FROM forum_replies
            WHERE tenant_id = $1
              AND topic_id = $2
              AND deleted_at IS NULL
              AND status <> 'deleted'
            "#,
            vec![tenant_id.into(), topic.id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_reply_delete_snapshots (
                tenant_id, topic_id, reply_id, author_id, previous_status
            )
            SELECT tenant_id, topic_id, id, author_id, status
            FROM forum_replies
            WHERE tenant_id = ?
              AND topic_id = ?
              AND deleted_at IS NULL
              AND status <> 'deleted'
            "#,
            vec![tenant_id.into(), topic.id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic reply delete snapshots do not support database backend {backend:?}"
            )))
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn topic_is_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<bool> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END AS is_deleted FROM forum_topics WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT CASE WHEN deleted_at IS NULL THEN 0 ELSE 1 END AS is_deleted FROM forum_topics WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };
    let row = txn
        .query_one_raw(statement)
        .await?
        .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
    let is_deleted: i64 = row.try_get("", "is_deleted")?;
    Ok(is_deleted == 1)
}

async fn load_topic_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<TopicDeleteSnapshot>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT previous_status,
                   CASE WHEN previous_is_locked THEN 1 ELSE 0 END AS previous_is_locked,
                   solution_reply_id, solution_author_id,
                   solution_marked_by_user_id, solution_marked_at
            FROM forum_topic_delete_snapshots
            WHERE tenant_id = $1 AND topic_id = $2
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT previous_status,
                   CASE WHEN previous_is_locked <> 0 THEN 1 ELSE 0 END AS previous_is_locked,
                   solution_reply_id, solution_author_id,
                   solution_marked_by_user_id, solution_marked_at
            FROM forum_topic_delete_snapshots
            WHERE tenant_id = ? AND topic_id = ?
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };

    let Some(row) = txn.query_one_raw(statement).await? else {
        return Ok(None);
    };
    let previous_status_value: String = row.try_get("", "previous_status")?;
    let previous_status = TopicStatus::from_str_value(&previous_status_value)
        .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
    let previous_is_locked: i64 = row.try_get("", "previous_is_locked")?;
    Ok(Some(TopicDeleteSnapshot {
        previous_status,
        previous_is_locked: previous_is_locked != 0,
        solution_reply_id: row.try_get("", "solution_reply_id")?,
        solution_author_id: row.try_get("", "solution_author_id")?,
        solution_marked_by_user_id: row.try_get("", "solution_marked_by_user_id")?,
        solution_marked_at: row.try_get("", "solution_marked_at")?,
    }))
}

async fn load_topic_reply_delete_snapshots_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<ReplyDeleteSnapshot>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT reply_id, author_id, previous_status
            FROM forum_topic_reply_delete_snapshots
            WHERE tenant_id = $1 AND topic_id = $2
            ORDER BY reply_id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT reply_id, author_id, previous_status
            FROM forum_topic_reply_delete_snapshots
            WHERE tenant_id = ? AND topic_id = ?
            ORDER BY reply_id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };
    txn.query_all_raw(statement)
        .await?
        .into_iter()
        .map(|row| {
            let status_value: String = row.try_get("", "previous_status")?;
            let previous_status = ReplyStatus::from_str_value(&status_value)
                .filter(|status| *status != ReplyStatus::Deleted)
                .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
            Ok(ReplyDeleteSnapshot {
                reply_id: row.try_get("", "reply_id")?,
                author_id: row.try_get("", "author_id")?,
                previous_status,
            })
        })
        .collect()
}

async fn restore_reply_from_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    snapshot: &ReplyDeleteSnapshot,
) -> ForumResult<()> {
    let (statement, values) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            r#"
            UPDATE forum_replies
            SET status = $3, deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1 AND topic_id = $2 AND id = $4 AND deleted_at IS NOT NULL
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                snapshot.previous_status.to_string().into(),
                snapshot.reply_id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => (
            r#"
            UPDATE forum_replies
            SET status = ?, deleted_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND topic_id = ? AND id = ? AND deleted_at IS NOT NULL
            "#,
            vec![
                snapshot.previous_status.to_string().into(),
                tenant_id.into(),
                topic_id.into(),
                snapshot.reply_id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };
    let result = txn
        .execute_raw(Statement::from_sql_and_values(
            txn.get_database_backend(),
            statement,
            values,
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicRestoreUnavailable(topic_id));
    }
    Ok(())
}

async fn restore_solution_in_tx(
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
                tenant_id, topic_id, reply_id,
                marked_by_user_id, marked_at
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
                tenant_id, topic_id, reply_id,
                marked_by_user_id, marked_at
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
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

async fn count_approved_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<i64> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT COUNT(*) AS reply_count FROM forum_replies WHERE tenant_id = $1 AND topic_id = $2 AND status = 'approved' AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS reply_count FROM forum_replies WHERE tenant_id = ? AND topic_id = ? AND status = 'approved' AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };
    let row = txn
        .query_one_raw(statement)
        .await?
        .ok_or(ForumError::TopicRestoreUnavailable(topic_id))?;
    Ok(row.try_get("", "reply_count")?)
}

async fn restore_topic_from_delete_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    snapshot: &TopicDeleteSnapshot,
    reply_count: i32,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            UPDATE forum_topics
            SET status = $3, is_locked = $4, reply_count = $5,
                last_reply_at = (
                    SELECT MAX(created_at)
                    FROM forum_replies
                    WHERE tenant_id = $1
                      AND topic_id = $2
                      AND status = 'approved'
                      AND deleted_at IS NULL
                ),
                deleted_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NOT NULL
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                snapshot.previous_status.to_string().into(),
                snapshot.previous_is_locked.into(),
                reply_count.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE forum_topics
            SET status = ?, is_locked = ?, reply_count = ?,
                last_reply_at = (
                    SELECT MAX(created_at)
                    FROM forum_replies
                    WHERE tenant_id = ?
                      AND topic_id = ?
                      AND status = 'approved'
                      AND deleted_at IS NULL
                ),
                deleted_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND id = ? AND deleted_at IS NOT NULL
            "#,
            vec![
                snapshot.previous_status.to_string().into(),
                snapshot.previous_is_locked.into(),
                reply_count.into(),
                tenant_id.into(),
                topic_id.into(),
                tenant_id.into(),
                topic_id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic restore does not support database backend {backend:?}"
            )))
        }
    };
    let result = txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        statement,
        vec![],
    )).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicRestoreUnavailable(topic_id));
    }
    Ok(())
}

async fn lock_topic_delete_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for (scope, seed) in [
                (format!("forum-topic-delete:{tenant_id}"), 23_i32),
                (tenant_id.to_string(), 0_i32),
            ] {
                txn.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, $2))",
                    vec![scope.into(), seed.into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic delete does not support database backend {backend:?}"
        ))),
    }
}

async fn claim_topic_delete_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let stmt = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_topics \
             SET updated_at = updated_at \
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        _ => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_topics \
             SET updated_at = updated_at \
             WHERE tenant_id = ?1 AND id = ?2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
    };
    let result = txn.execute_raw(stmt).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicDeleted);
    }
    Ok(())
}

async fn mark_topic_thread_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let update_replies_stmt = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_replies \
             SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = $1 AND topic_id = $2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        _ => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_replies \
             SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = ?1 AND topic_id = ?2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
    };
    txn.execute_raw(update_replies_stmt).await?;

    let update_topics_stmt = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_topics \
             SET status = 'archived', is_locked = TRUE, reply_count = 0, last_reply_at = NULL, \
                 deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        _ => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_topics \
             SET status = 'archived', is_locked = TRUE, reply_count = 0, last_reply_at = NULL, \
                 deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = ?1 AND id = ?2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
    };
    let result = txn.execute_raw(update_topics_stmt).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicDeleted);
    }
    Ok(())
}

