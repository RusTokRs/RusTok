use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use sea_orm::sea_query::OnConflict;

use rustok_core::SecurityContext;

use rustok_api::{Action, PortContext, Resource};

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::{forum_reply, forum_reply_vote, forum_topic, forum_topic_vote};
use crate::error::{ForumError, ForumResult};
use crate::services::engagement_mode::{ForumEngagementMode, ForumSettingsProviders};
use crate::services::projection_invalidation::publish_forum_topic_projection_direct_in_tx;
use crate::services::rbac::enforce_scope;
use crate::services::topic_vote_lock::{
    lock_active_topic_vote_write_in_tx, lock_topic_vote_scopes_in_tx,
};
use crate::services::topic_write_audience::topic_write_audience_allows;
use crate::state_machine::ReplyStatus;

#[derive(Debug, Clone, Copy, Default)]
pub struct VoteSummary {
    pub score: i32,
    pub current_user_vote: Option<i32>,
}

/// Tenant vote rules from `ForumModuleSettings`, read inside the vote write transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ForumVotePolicy {
    allow_downvotes: bool,
    allow_self_voting: bool,
}

impl ForumVotePolicy {
    async fn resolve_in_tx(
        providers: &ForumSettingsProviders,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let settings = providers.module_settings_in_tx(txn, tenant_id).await?;
        Ok(Self {
            allow_downvotes: settings.allow_downvotes,
            allow_self_voting: settings.allow_self_voting,
        })
    }

    fn require_value_allowed(self, value: i32) -> ForumResult<()> {
        if value == -1 && !self.allow_downvotes {
            return Err(ForumError::Validation(
                "Forum downvotes are disabled for this tenant".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct VoteService {
    db: DatabaseConnection,
    settings: ForumSettingsProviders,
    audience_facts: Option<SharedForumAudienceFactsPort>,
}

impl VoteService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            settings: ForumSettingsProviders::default(),
            audience_facts: None,
        }
    }

    pub fn with_settings_providers(mut self, settings: ForumSettingsProviders) -> Self {
        self.settings = settings;
        self
    }

    /// Supplies the host audience facts port used by vote writes when a required trust,
    /// channel, or group layer cannot be decided from the caller alone.
    pub fn with_audience_facts(mut self, facts_port: SharedForumAudienceFactsPort) -> Self {
        self.audience_facts = Some(facts_port);
        self
    }

    /// Records a topic vote. The caller must be able to read the topic through the owner
    /// audience contract, and must not be the topic author. Clearing a vote is not gated.
    #[instrument(skip(self, security, context))]
    pub async fn set_topic_vote(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        value: i32,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        validate_vote_value(value)?;
        if !topic_write_audience_allows(
            &self.db,
            self.audience_facts.clone(),
            tenant_id,
            topic_id,
            &security,
            context,
        )
        .await?
        {
            return Err(ForumError::TopicNotFound(topic_id));
        }

        let txn = self.db.begin().await?;
        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        let policy = ForumVotePolicy::resolve_in_tx(&self.settings, &txn, tenant_id).await?;
        policy.require_value_allowed(value)?;
        lock_active_topic_vote_write_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_vote_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;
        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        if topic.author_id == Some(user_id) && !policy.allow_self_voting {
            return Err(ForumError::forbidden(
                "Forum topic authors cannot vote on their own topic",
            ));
        }
        self.upsert_topic_vote_in_tx(&txn, tenant_id, topic_id, user_id, value)
            .await?;
        publish_forum_topic_projection_direct_in_tx(&txn, tenant_id, Some(user_id), topic_id)
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

        let txn = self.db.begin().await?;
        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        lock_active_topic_vote_write_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_vote_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;
        forum_topic_vote::Entity::delete_many()
            .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_vote::Column::TopicId.eq(topic_id))
            .filter(forum_topic_vote::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        publish_forum_topic_projection_direct_in_tx(&txn, tenant_id, Some(user_id), topic_id)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    /// Records a reply vote. The reply's parent topic must be readable by the caller
    /// through the owner audience contract, and the caller must not be the reply author.
    /// Denied and missing replies both return `ReplyNotFound`.
    #[instrument(skip(self, security, context))]
    pub async fn set_reply_vote(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        value: i32,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        validate_vote_value(value)?;

        // The parent topic id is immutable for a reply, so the audience gate can use it
        // before the transaction. Status and author are re-read under the row lock below.
        let preflight = forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))?;
        if !topic_write_audience_allows(
            &self.db,
            self.audience_facts.clone(),
            tenant_id,
            preflight.topic_id,
            &security,
            context,
        )
        .await?
        {
            return Err(ForumError::ReplyNotFound(reply_id));
        }

        let txn = self.db.begin().await?;
        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        let policy = ForumVotePolicy::resolve_in_tx(&self.settings, &txn, tenant_id).await?;
        policy.require_value_allowed(value)?;
        let reply =
            crate::services::ReplyService::find_reply_for_update_in_tx(&txn, tenant_id, reply_id)
                .await?;
        if reply.status != ReplyStatus::Approved {
            return Err(ForumError::Validation(
                "Only approved replies can receive votes".to_string(),
            ));
        }
        if reply.author_id == Some(user_id) && !policy.allow_self_voting {
            return Err(ForumError::forbidden(
                "Forum reply authors cannot vote on their own reply",
            ));
        }

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
        let txn = self.db.begin().await?;
        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        crate::services::ReplyService::find_reply_for_update_in_tx(&txn, tenant_id, reply_id)
            .await?;
        forum_reply_vote::Entity::delete_many()
            .filter(forum_reply_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_vote::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_vote::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn internal_voting_enabled_for_read(&self, tenant_id: Uuid) -> ForumResult<bool> {
        match ForumEngagementMode::resolve(&self.settings, tenant_id).await {
            Ok(mode) => Ok(mode.is_internal_voting()),
            Err(ForumError::CapabilityUnavailable { .. }) => Ok(false),
            Err(error) => Err(error),
        }
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

        if !self.internal_voting_enabled_for_read(tenant_id).await? {
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

        if !self.internal_voting_enabled_for_read(tenant_id).await? {
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

    async fn upsert_topic_vote_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        user_id: Uuid,
        value: i32,
    ) -> ForumResult<()> {
        let now = Utc::now();
        forum_topic_vote::Entity::insert(forum_topic_vote::ActiveModel {
            topic_id: Set(topic_id),
            user_id: Set(user_id),
            tenant_id: Set(tenant_id),
            value: Set(value),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        })
        .on_conflict(
            OnConflict::columns([
                forum_topic_vote::Column::TopicId,
                forum_topic_vote::Column::UserId,
                forum_topic_vote::Column::TenantId,
            ])
            .update_columns([
                forum_topic_vote::Column::Value,
                forum_topic_vote::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

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
        let now = Utc::now();
        forum_reply_vote::Entity::insert(forum_reply_vote::ActiveModel {
            reply_id: Set(reply_id),
            user_id: Set(user_id),
            tenant_id: Set(tenant_id),
            value: Set(value),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        })
        .on_conflict(
            OnConflict::columns([
                forum_reply_vote::Column::ReplyId,
                forum_reply_vote::Column::UserId,
                forum_reply_vote::Column::TenantId,
            ])
            .update_columns([
                forum_reply_vote::Column::Value,
                forum_reply_vote::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

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
