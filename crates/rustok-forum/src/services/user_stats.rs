use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use rustok_api::{Action, Resource};

use crate::dto::ForumUserStatsResponse;
use crate::entities::forum_user_stat;
use crate::error::ForumResult;
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
        Self::adjust_counts_in_tx(txn, tenant_id, user_id, 0, 0, delta).await
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

        let now = Utc::now();
        let existing = forum_user_stat::Entity::find_by_id((tenant_id, user_id))
            .one(txn)
            .await?;

        match existing {
            Some(existing) => {
                let mut active: forum_user_stat::ActiveModel = existing.into();
                let current_topic = match active.topic_count.clone() {
                    sea_orm::ActiveValue::Set(value) => value,
                    sea_orm::ActiveValue::Unchanged(value) => value,
                    sea_orm::ActiveValue::NotSet => 0,
                };
                let current_reply = match active.reply_count.clone() {
                    sea_orm::ActiveValue::Set(value) => value,
                    sea_orm::ActiveValue::Unchanged(value) => value,
                    sea_orm::ActiveValue::NotSet => 0,
                };
                let current_solution = match active.solution_count.clone() {
                    sea_orm::ActiveValue::Set(value) => value,
                    sea_orm::ActiveValue::Unchanged(value) => value,
                    sea_orm::ActiveValue::NotSet => 0,
                };
                active.topic_count = Set((current_topic + topic_delta).max(0));
                active.reply_count = Set((current_reply + reply_delta).max(0));
                active.solution_count = Set((current_solution + solution_delta).max(0));
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                forum_user_stat::ActiveModel {
                    tenant_id: Set(tenant_id),
                    user_id: Set(user_id),
                    topic_count: Set(topic_delta.max(0)),
                    reply_count: Set(reply_delta.max(0)),
                    solution_count: Set(solution_delta.max(0)),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

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

        let approved_count = format!(
            "SELECT COUNT(*) FROM forum_replies AS replies \
             WHERE replies.tenant_id = '{tenant_id}' \
               AND replies.topic_id = '{topic_id}' \
               AND replies.author_id = forum_user_stats.user_id \
               AND replies.status = 'approved' \
               AND replies.deleted_at IS NULL"
        );
        txn.execute_unprepared(&format!(
            "UPDATE forum_user_stats \
             SET reply_count = CASE \
                 WHEN reply_count > ({approved_count}) \
                 THEN reply_count - ({approved_count}) \
                 ELSE 0 \
             END, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = '{tenant_id}' \
               AND EXISTS (\
                   SELECT 1 FROM forum_replies AS replies \
                   WHERE replies.tenant_id = '{tenant_id}' \
                     AND replies.topic_id = '{topic_id}' \
                     AND replies.author_id = forum_user_stats.user_id \
                     AND replies.status = 'approved' \
                     AND replies.deleted_at IS NULL\
               )"
        ))
        .await?;

        Self::adjust_solution_count_in_tx(txn, tenant_id, solution_author_id, -1).await?;
        Ok(())
    }

    pub(crate) async fn decrement_topic_thread_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_author_id: Option<Uuid>,
        reply_author_ids: &[Option<Uuid>],
        solution_author_id: Option<Uuid>,
    ) -> ForumResult<()> {
        Self::adjust_topic_count_in_tx(txn, tenant_id, topic_author_id, -1).await?;
        for reply_author_id in reply_author_ids {
            Self::adjust_reply_count_in_tx(txn, tenant_id, *reply_author_id, -1).await?;
        }
        Self::adjust_solution_count_in_tx(txn, tenant_id, solution_author_id, -1).await?;
        Ok(())
    }
}
