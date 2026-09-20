use chrono::Utc;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, EntityTrait,
    Statement,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use rustok_api::{Action, Resource};

use crate::dto::ForumUserStatsResponse;
use crate::entities::forum_user_stat;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

pub struct UserStatsService {
    db: DatabaseConnection,
}

impl UserStatsService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        user_id: Uuid,
    ) -> ForumResult<ForumUserStatsResponse> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let row = forum_user_stat::Entity::find_by_id((tenant_id, user_id))
            .one(&self.db)
            .await?;

        Ok(match row {
            Some(row) => ForumUserStatsResponse {
                user_id: row.user_id,
                topic_count: row.topic_count,
                reply_count: row.reply_count,
                solution_count: row.solution_count,
                updated_at: row.updated_at.to_rfc3339(),
            },
            None => ForumUserStatsResponse {
                user_id,
                topic_count: 0,
                reply_count: 0,
                solution_count: 0,
                updated_at: Utc::now().to_rfc3339(),
            },
        })
    }

    pub(crate) async fn adjust_topic_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        delta: i32,
    ) -> ForumResult<()> {
        Self::adjust_counts_in_tx(txn, tenant_id, user_id, delta, 0, 0).await
    }

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        delta: i32,
    ) -> ForumResult<()> {
        Self::adjust_counts_in_tx(txn, tenant_id, user_id, 0, delta, 0).await
    }

    pub(crate) async fn adjust_solution_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        delta: i32,
    ) -> ForumResult<()> {
        if delta == -1 {
            return Self::decrement_solution_count_exact_in_tx(txn, tenant_id, user_id).await;
        }
        Self::adjust_counts_in_tx(txn, tenant_id, user_id, 0, 0, delta).await
    }

    pub(crate) async fn decrement_solution_count_exact_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
    ) -> ForumResult<()> {
        let Some(user_id) = user_id else {
            return Ok(());
        };
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE forum_user_stats SET solution_count = solution_count - 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND user_id = $2 AND solution_count > 0",
                vec![tenant_id.into(), user_id.into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE forum_user_stats SET solution_count = solution_count - 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = ? AND user_id = ? AND solution_count > 0",
                vec![tenant_id.into(), user_id.into()],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum exact solution statistic decrement does not support database backend {backend:?}"
                )));
            }
        };
        if txn.execute_raw(statement).await?.rows_affected() != 1 {
            return Err(ForumError::Validation(
                "Forum solution author statistic is inconsistent".to_string(),
            ));
        }
        Ok(())
    }

    async fn adjust_counts_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        topic_delta: i32,
        reply_delta: i32,
        solution_delta: i32,
    ) -> ForumResult<()> {
        let Some(user_id) = user_id else {
            return Ok(());
        };

        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO forum_user_stats                  (tenant_id, user_id, topic_count, reply_count, solution_count, created_at, updated_at)                  VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)                  ON CONFLICT (tenant_id, user_id) DO UPDATE SET                     topic_count = CASE WHEN forum_user_stats.topic_count + EXCLUDED.topic_count < 0                                        THEN 0 ELSE forum_user_stats.topic_count + EXCLUDED.topic_count END,                     reply_count = CASE WHEN forum_user_stats.reply_count + EXCLUDED.reply_count < 0                                        THEN 0 ELSE forum_user_stats.reply_count + EXCLUDED.reply_count END,                     solution_count = CASE WHEN forum_user_stats.solution_count + EXCLUDED.solution_count < 0                                           THEN 0 ELSE forum_user_stats.solution_count + EXCLUDED.solution_count END,                     updated_at = CURRENT_TIMESTAMP",
                vec![
                    tenant_id.into(),
                    user_id.into(),
                    topic_delta.into(),
                    reply_delta.into(),
                    solution_delta.into(),
                ],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO forum_user_stats                  (tenant_id, user_id, topic_count, reply_count, solution_count, created_at, updated_at)                  VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)                  ON CONFLICT (tenant_id, user_id) DO UPDATE SET                     topic_count = MAX(forum_user_stats.topic_count + excluded.topic_count, 0),                     reply_count = MAX(forum_user_stats.reply_count + excluded.reply_count, 0),                     solution_count = MAX(forum_user_stats.solution_count + excluded.solution_count, 0),                     updated_at = CURRENT_TIMESTAMP",
                vec![
                    tenant_id.into(),
                    user_id.into(),
                    topic_delta.into(),
                    reply_delta.into(),
                    solution_delta.into(),
                ],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum user statistic counter adjustment does not support database backend {backend:?}"
                )));
            }
        };

        txn.execute_raw(statement).await?;
        Ok(())
    }

    /// Decrement all user statistics contributed by a topic without loading its
    /// replies into application memory. The correlated update applies one
    /// bounded SQL statement regardless of thread size.
    pub(crate) async fn decrement_topic_thread_aggregated_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        topic_author_id: Option<Uuid>,
        solution_author_id: Option<Uuid>,
    ) -> ForumResult<()> {
        Self::adjust_topic_count_in_tx(txn, tenant_id, topic_author_id, -1).await?;

        let stmt = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE forum_user_stats \
                 SET reply_count = CASE \
                     WHEN reply_count > ( \
                         SELECT COUNT(*) FROM forum_replies AS replies \
                         WHERE replies.tenant_id = $1 \
                           AND replies.topic_id = $2 \
                           AND replies.author_id = forum_user_stats.user_id \
                           AND replies.status = 'approved' \
                     ) \
                     THEN reply_count - ( \
                         SELECT COUNT(*) FROM forum_replies AS replies \
                         WHERE replies.tenant_id = $1 \
                           AND replies.topic_id = $2 \
                           AND replies.author_id = forum_user_stats.user_id \
                           AND replies.status = 'approved' \
                     ) \
                     ELSE 0 \
                 END, updated_at = CURRENT_TIMESTAMP \
                 WHERE tenant_id = $1 \
                   AND EXISTS ( \
                       SELECT 1 FROM forum_replies AS replies \
                       WHERE replies.tenant_id = $1 \
                         AND replies.topic_id = $2 \
                         AND replies.author_id = forum_user_stats.user_id \
                         AND replies.status = 'approved' \
                   )",
                vec![tenant_id.into(), topic_id.into()],
            ),
            _ => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE forum_user_stats \
                 SET reply_count = CASE \
                     WHEN reply_count > ( \
                         SELECT COUNT(*) FROM forum_replies AS replies \
                         WHERE replies.tenant_id = ?1 \
                           AND replies.topic_id = ?2 \
                           AND replies.author_id = forum_user_stats.user_id \
                           AND replies.status = 'approved' \
                     ) \
                     THEN reply_count - ( \
                         SELECT COUNT(*) FROM forum_replies AS replies \
                         WHERE replies.tenant_id = ?1 \
                           AND replies.topic_id = ?2 \
                           AND replies.author_id = forum_user_stats.user_id \
                           AND replies.status = 'approved' \
                     ) \
                     ELSE 0 \
                 END, updated_at = CURRENT_TIMESTAMP \
                 WHERE tenant_id = ?1 \
                   AND EXISTS ( \
                       SELECT 1 FROM forum_replies AS replies \
                       WHERE replies.tenant_id = ?1 \
                         AND replies.topic_id = ?2 \
                         AND replies.author_id = forum_user_stats.user_id \
                         AND replies.status = 'approved' \
                   )",
                vec![tenant_id.into(), topic_id.into()],
            ),
        };
        txn.execute_raw(stmt).await?;

        Self::adjust_solution_count_in_tx(txn, tenant_id, solution_author_id, -1).await?;
        Ok(())
    }
}

#[path = "member_card.rs"]
mod member_card_file;
pub use member_card_file::{
    ForumMemberCard, ForumMemberCardAudience, ForumMemberCardService, ForumMemberStats,
    MAX_FORUM_MEMBER_CARD_USER_IDS,
};
