use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use rustok_api::{Action, Resource};

use crate::entities::{forum_reply, forum_reply_vote, forum_topic, forum_topic_vote};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::state_machine::{ReplyStatus, TopicStatus};

#[derive(Debug, Clone, Copy, Default)]
pub struct VoteSummary {
    pub score: i32,
    pub current_user_vote: Option<i32>,
}

pub struct VoteService {
    db: DatabaseConnection,
}

impl VoteService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security))]
    pub async fn set_topic_vote(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        value: i32,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        validate_vote_value(value)?;

        let topic = self.find_topic(tenant_id, topic_id).await?;
        if topic.status == TopicStatus::Archived {
            return Err(ForumError::Validation(
                "Archived topics cannot receive votes".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        self.upsert_topic_vote_in_tx(&txn, tenant_id, topic_id, user_id, value)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn clear_topic_vote(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_topic(tenant_id, topic_id).await?;

        let txn = self.db.begin().await?;
        forum_topic_vote::Entity::delete_many()
            .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_vote::Column::TopicId.eq(topic_id))
            .filter(forum_topic_vote::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn set_reply_vote(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        value: i32,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        validate_vote_value(value)?;

        let reply = self.find_reply(tenant_id, reply_id).await?;
        if reply.status != ReplyStatus::Approved {
            return Err(ForumError::Validation(
                "Only approved replies can receive votes".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        self.upsert_reply_vote_in_tx(&txn, tenant_id, reply_id, user_id, value)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn clear_reply_vote(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_reply(tenant_id, reply_id).await?;

        let txn = self.db.begin().await?;
        forum_reply_vote::Entity::delete_many()
            .filter(forum_reply_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_vote::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_vote::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn topic_vote_summary(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        user_id: Option<Uuid>,
    ) -> ForumResult<VoteSummary> {
        Ok(self
            .topic_vote_summaries(tenant_id, &[topic_id], user_id)
            .await?
            .remove(&topic_id)
            .unwrap_or_default())
    }

    pub async fn topic_vote_summaries(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
        user_id: Option<Uuid>,
    ) -> ForumResult<HashMap<Uuid, VoteSummary>> {
        if topic_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let votes = forum_topic_vote::Entity::find()
            .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_vote::Column::TopicId.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?;

        let mut summaries = HashMap::new();
        for vote in votes {
            let entry = summaries
                .entry(vote.topic_id)
                .or_insert_with(VoteSummary::default);
            entry.score += vote.value;
            if Some(vote.user_id) == user_id {
                entry.current_user_vote = Some(vote.value);
            }
        }

        Ok(summaries)
    }

    pub async fn reply_vote_summary(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        user_id: Option<Uuid>,
    ) -> ForumResult<VoteSummary> {
        Ok(self
            .reply_vote_summaries(tenant_id, &[reply_id], user_id)
            .await?
            .remove(&reply_id)
            .unwrap_or_default())
    }

    pub async fn reply_vote_summaries(
        &self,
        tenant_id: Uuid,
        reply_ids: &[Uuid],
        user_id: Option<Uuid>,
    ) -> ForumResult<HashMap<Uuid, VoteSummary>> {
        if reply_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let votes = forum_reply_vote::Entity::find()
            .filter(forum_reply_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_vote::Column::ReplyId.is_in(reply_ids.to_vec()))
            .all(&self.db)
            .await?;

        let mut summaries = HashMap::new();
        for vote in votes {
            let entry = summaries
                .entry(vote.reply_id)
                .or_insert_with(VoteSummary::default);
            entry.score += vote.value;
            if Some(vote.user_id) == user_id {
                entry.current_user_vote = Some(vote.value);
            }
        }

        Ok(summaries)
    }

    async fn find_topic(&self, tenant_id: Uuid, topic_id: Uuid) -> ForumResult<forum_topic::Model> {
        forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))
    }

    async fn find_reply(&self, tenant_id: Uuid, reply_id: Uuid) -> ForumResult<forum_reply::Model> {
        forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))
    }

    async fn upsert_topic_vote_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        user_id: Uuid,
        value: i32,
    ) -> ForumResult<()> {
        let existing = forum_topic_vote::Entity::find()
            .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_vote::Column::TopicId.eq(topic_id))
            .filter(forum_topic_vote::Column::UserId.eq(user_id))
            .one(txn)
            .await?;
        let now = Utc::now();

        match existing {
            Some(existing) => {
                let mut active: forum_topic_vote::ActiveModel = existing.into();
                active.value = Set(value);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                forum_topic_vote::ActiveModel {
                    topic_id: Set(topic_id),
                    user_id: Set(user_id),
                    tenant_id: Set(tenant_id),
                    value: Set(value),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }

    async fn upsert_reply_vote_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        user_id: Uuid,
        value: i32,
    ) -> ForumResult<()> {
        let existing = forum_reply_vote::Entity::find()
            .filter(forum_reply_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_vote::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_vote::Column::UserId.eq(user_id))
            .one(txn)
            .await?;
        let now = Utc::now();

        match existing {
            Some(existing) => {
                let mut active: forum_reply_vote::ActiveModel = existing.into();
                active.value = Set(value);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                forum_reply_vote::ActiveModel {
                    reply_id: Set(reply_id),
                    user_id: Set(user_id),
                    tenant_id: Set(tenant_id),
                    value: Set(value),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }
}

fn require_authenticated_user(security: &SecurityContext) -> ForumResult<Uuid> {
    security
        .user_id
        .ok_or_else(|| ForumError::forbidden("Authenticated user context is required for voting"))
}

fn validate_vote_value(value: i32) -> ForumResult<()> {
    if value == -1 || value == 1 {
        return Ok(());
    }

    Err(ForumError::Validation(
        "Vote value must be either -1 or 1".to_string(),
    ))
}
