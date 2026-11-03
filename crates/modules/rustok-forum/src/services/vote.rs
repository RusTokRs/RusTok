use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QuerySelect, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use sea_orm::sea_query::OnConflict;

use rustok_core::SecurityContext;

use rustok_api::{Action, PortContext, Resource};

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::{forum_reply, forum_reply_vote, forum_topic, forum_topic_vote};
use crate::error::{ForumError, ForumResult};
use crate::services::category_audience::lock_category_tree_in_tx;
use crate::services::engagement_mode::{ForumEngagementMode, ForumSettingsProviders};
use crate::services::projection_invalidation::publish_forum_topic_projection_direct_in_tx;
use crate::services::rbac::enforce_scope;
use crate::services::topic_vote_lock::{
    lock_active_topic_vote_write_in_tx, lock_topic_vote_scopes_in_tx,
};
use crate::services::topic_write_audience::topic_write_audience_allows_in_tx;
use crate::state_machine::{ReplyStatus, TopicStatus};

#[derive(Debug, Clone, Copy, Default)]
pub struct VoteSummary {
    pub score: i32,
    pub current_user_vote: Option<i32>,
}

/// SQL `SUM(value)` widens to `i64`; the public score contract is `i32`.
/// Clamp instead of wrapping so an extreme aggregate saturates visibly
/// rather than flipping sign.
fn clamp_vote_score(score: i64) -> i32 {
    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
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
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Tenant ID must not be nil".to_string(),
            ));
        }
        if topic_id.is_nil() {
            return Err(ForumError::Validation(
                "Topic ID must not be nil".to_string(),
            ));
        }
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        validate_vote_value(value)?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        if !topic_write_audience_allows_in_tx(
            &self.db,
            &txn,
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
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Tenant ID must not be nil".to_string(),
            ));
        }
        if topic_id.is_nil() {
            return Err(ForumError::Validation(
                "Topic ID must not be nil".to_string(),
            ));
        }
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;

        let txn = self.db.begin().await?;
        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        // This is an actor-scoped blind delete, not a topic mutation.
        // The active-topic lock rejects archived or missing topics, which
        // exposes target existence and strands votes after archival.
        // Deletion by the exact vote tuple is atomic without a topic probe.
        let deleted = forum_topic_vote::Entity::delete_many()
            .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_vote::Column::TopicId.eq(topic_id))
            .filter(forum_topic_vote::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        if deleted.rows_affected > 0 {
            publish_forum_topic_projection_direct_in_tx(&txn, tenant_id, Some(user_id), topic_id)
                .await?;
        }
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
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Tenant ID must not be nil".to_string(),
            ));
        }
        if reply_id.is_nil() {
            return Err(ForumError::Validation(
                "Reply ID must not be nil".to_string(),
            ));
        }
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        validate_vote_value(value)?;

        // Load the immutable reply -> topic binding within the same
        // transaction as authorization and the vote write. Never disclose
        // whether a reply exists when its parent is audience-denied.
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let parent = forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))?;
        if !topic_write_audience_allows_in_tx(
            &self.db,
            &txn,
            self.audience_facts.clone(),
            tenant_id,
            parent.topic_id,
            &security,
            context,
        )
        .await?
        {
            return Err(ForumError::ReplyNotFound(reply_id));
        }

        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        let policy = ForumVotePolicy::resolve_in_tx(&self.settings, &txn, tenant_id).await?;
        policy.require_value_allowed(value)?;
        // Match topic-vote rules and the category -> topic -> reply lock
        // order. An approved reply must not be votable after its parent
        // topic has been archived while authorization was evaluated.
        let parent_topic = crate::services::TopicService::find_topic_for_update_in_tx(
            &txn,
            tenant_id,
            parent.topic_id,
        )
        .await?;
        if parent_topic.status == TopicStatus::Archived {
            return Err(ForumError::Validation(
                "Forum reply votes cannot be changed after topic archival".to_string(),
            ));
        }
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
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Tenant ID must not be nil".to_string(),
            ));
        }
        if reply_id.is_nil() {
            return Err(ForumError::Validation(
                "Reply ID must not be nil".to_string(),
            ));
        }
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        let txn = self.db.begin().await?;
        ForumEngagementMode::resolve_in_tx(&self.settings, &txn, tenant_id)
            .await?
            .require_internal_voting()?;
        // Blindly delete only this principal's vote. Requiring the reply row
        // here leaks whether an inaccessible or deleted reply still exists
        // and contradicts the accepted vote-clear owner contract.
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
            Err(ForumError::CapabilityUnavailable { .. }) => {
                // `use_reactions = true` with the reactions module disabled
                // fails vote writes closed; keep reads from silently hiding
                // the vote UI without an operator-visible signal.
                tracing::warn!(
                    tenant_id = %tenant_id,
                    "forum vote summaries hidden: use_reactions is enabled but the reactions module is unavailable"
                );
                Ok(false)
            }
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

        // Aggregate in SQL: a popular topic's full vote history must not be
        // loaded into memory just to render one list page. The current-user
        // lookup stays a separate bounded query (one row per requested
        // topic at most).
        let rows = forum_topic_vote::Entity::find()
            .select_only()
            .column(forum_topic_vote::Column::TopicId)
            .column_as(forum_topic_vote::Column::Value.sum(), "score")
            .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_vote::Column::TopicId.is_in(topic_ids.to_vec()))
            .group_by(forum_topic_vote::Column::TopicId)
            .into_tuple::<(Uuid, Option<i64>)>()
            .all(&self.db)
            .await?;
        let mut summaries: HashMap<Uuid, VoteSummary> = rows
            .into_iter()
            .map(|(topic_id, score)| {
                (
                    topic_id,
                    VoteSummary {
                        score: clamp_vote_score(score.unwrap_or(0)),
                        current_user_vote: None,
                    },
                )
            })
            .collect();
        if let Some(user_id) = user_id {
            let user_votes = forum_topic_vote::Entity::find()
                .select_only()
                .columns([
                    forum_topic_vote::Column::TopicId,
                    forum_topic_vote::Column::Value,
                ])
                .filter(forum_topic_vote::Column::TenantId.eq(tenant_id))
                .filter(forum_topic_vote::Column::TopicId.is_in(topic_ids.to_vec()))
                .filter(forum_topic_vote::Column::UserId.eq(user_id))
                .into_tuple::<(Uuid, i32)>()
                .all(&self.db)
                .await?;
            for (topic_id, value) in user_votes {
                summaries.entry(topic_id).or_default().current_user_vote = Some(value);
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

        // See `topic_vote_summaries` for why this aggregates in SQL.
        let rows = forum_reply_vote::Entity::find()
            .select_only()
            .column(forum_reply_vote::Column::ReplyId)
            .column_as(forum_reply_vote::Column::Value.sum(), "score")
            .filter(forum_reply_vote::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_vote::Column::ReplyId.is_in(reply_ids.to_vec()))
            .group_by(forum_reply_vote::Column::ReplyId)
            .into_tuple::<(Uuid, Option<i64>)>()
            .all(&self.db)
            .await?;
        let mut summaries: HashMap<Uuid, VoteSummary> = rows
            .into_iter()
            .map(|(reply_id, score)| {
                (
                    reply_id,
                    VoteSummary {
                        score: clamp_vote_score(score.unwrap_or(0)),
                        current_user_vote: None,
                    },
                )
            })
            .collect();
        if let Some(user_id) = user_id {
            let user_votes = forum_reply_vote::Entity::find()
                .select_only()
                .columns([
                    forum_reply_vote::Column::ReplyId,
                    forum_reply_vote::Column::Value,
                ])
                .filter(forum_reply_vote::Column::TenantId.eq(tenant_id))
                .filter(forum_reply_vote::Column::ReplyId.is_in(reply_ids.to_vec()))
                .filter(forum_reply_vote::Column::UserId.eq(user_id))
                .into_tuple::<(Uuid, i32)>()
                .all(&self.db)
                .await?;
            for (reply_id, value) in user_votes {
                summaries.entry(reply_id).or_default().current_user_vote = Some(value);
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
