use std::ops::Deref;

use flex::delete_attached_localized_values;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{CreateTopicInput, TopicResponse, UpdateTopicInput};
use crate::entities::{forum_reply, forum_solution};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category::CategoryService;
use super::rbac::enforce_owned_scope;
use super::topic;
use super::user_stats::UserStatsService;

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
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .create_with_relations(tenant_id, security, input)
            .await
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .update_with_relations(tenant_id, topic_id, security, input)
            .await
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
        claim_topic_delete_in_tx(&txn, tenant_id, topic_id).await?;
        let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;

        let replies = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .all(&txn)
            .await?;
        let public_reply_author_ids = replies
            .iter()
            .filter(|reply| reply.status == ReplyStatus::Approved)
            .map(|reply| reply.author_id)
            .collect::<Vec<_>>();
        let public_reply_count = i32::try_from(public_reply_author_ids.len()).map_err(|_| {
            ForumError::Validation("Forum reply count exceeds supported range".to_string())
        })?;

        let solution_author_id = if let Some(solution) = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&txn)
            .await?
        {
            replies
                .iter()
                .find(|reply| reply.id == solution.reply_id)
                .and_then(|reply| reply.author_id)
        } else {
            None
        };

        redact_topic_content_in_tx(&txn, tenant_id, topic_id).await?;
        delete_attached_localized_values(&txn, tenant_id, "topic", topic_id)
            .await
            .map_err(map_flex_cleanup_error)?;
        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;
        mark_topic_thread_deleted_in_tx(&txn, tenant_id, topic_id).await?;

        CategoryService::adjust_counters_in_tx(
            &txn,
            tenant_id,
            topic.category_id,
            -1,
            -public_reply_count,
        )
        .await?;
        UserStatsService::decrement_topic_thread_in_tx(
            &txn,
            tenant_id,
            topic.author_id,
            &public_reply_author_ids,
            solution_author_id,
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

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        delta: i32,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        topic::TopicService::adjust_reply_count_in_tx(txn, tenant_id, topic_id, delta).await
    }

    pub(crate) async fn set_pinned_in_tx(
        txn: &DatabaseTransaaction,
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

async fn claim_topic_delete_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let result = txn
        .execute_unprepared(&format!(
            "UPDATE forum_topics \
             SET updated_at = updated_at \
             WHERE tenant_id = '{tenant_id}' AND id = '{topic_id}' AND deleted_at IS NULL"
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicDeleted);
    }
    Ok(())
}

async fn redact_topic_content_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    for statement in [
        format!(
            "UPDATE forum_topic_translations \
             SET title = '[deleted]', slug = NULL, body = '[deleted]', \
                 body_format = 'markdown', updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}'"
        ),
        format!(
            "UPDATE forum_reply_bodies \
             SET body = '[deleted]', body_format = 'markdown', updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = '{tenant_id}' AND reply_id IN (\
                 SELECT id FROM forum_replies \
                 WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}' AND deleted_at IS NULL\
             )"
        ),
    ] {
        txn.execute_unprepared(&statement).await?;
    }
    Ok(())
}

async fn mark_topic_thread_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    txn.execute_unprepared(&format!(
        "UPDATE forum_replies \
         SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}' AND deleted_at IS NULL"
    ))
    .await?;

    let result = txn
        .execute_unprepared(&format!(
            "UPDATE forum_topics \
             SET status = 'archived', is_locked = TRUE, reply_count = 0, last_reply_at = NULL, \
                 deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = '{tenant_id}' AND id = '{topic_id}' AND deleted_at IS NULL"
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicDeleted);
    }
    Ok(())
}

fn map_flex_cleanup_error(error: rustok_core::field_schema::FlexError) -> ForumError {
    match error {
        rustok_core::field_schema::FlexError::Database(message) => {
            ForumError::Database(sea_orm::DbErr::Custom(message))
        }
        other => ForumError::Validation(other.to_string()),
    }
}
