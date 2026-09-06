use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::{ForumSubscriptionPolicyResponse, UpdateForumSubscriptionPolicyInput};
use crate::entities::forum_subscription_policy;
use crate::error::{ForumError, ForumResult};

use super::SubscriptionService;
use super::helpers::{
    INITIAL_REVISION, default_policy, enforce_policy_scope, ensure_revision_update,
    policy_response, validate_expected_revision, validate_new_revision, validate_policy,
};

impl SubscriptionService {
    #[instrument(skip(self, security))]
    pub async fn get_policy(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumSubscriptionPolicyResponse> {
        enforce_policy_scope(&security)?;
        let model = forum_subscription_policy::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?;
        Ok(model
            .map(policy_response)
            .unwrap_or_else(|| default_policy(tenant_id)))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_policy(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: UpdateForumSubscriptionPolicyInput,
    ) -> ForumResult<ForumSubscriptionPolicyResponse> {
        enforce_policy_scope(&security)?;
        validate_policy(&input)?;
        let txn = self.db.begin().await?;
        let existing = forum_subscription_policy::Entity::find_by_id(tenant_id)
            .one(&txn)
            .await?;
        let now = Utc::now();
        let model = match existing {
            Some(existing) => {
                validate_expected_revision(input.expected_revision, existing.revision)?;
                let previous_revision = existing.revision;
                let next_revision = previous_revision.saturating_add(1);
                let result = forum_subscription_policy::Entity::update_many()
                    .filter(forum_subscription_policy::Column::TenantId.eq(tenant_id))
                    .filter(forum_subscription_policy::Column::Revision.eq(previous_revision))
                    .set(forum_subscription_policy::ActiveModel {
                        auto_subscribe_topic_authors: Set(input.auto_subscribe_topic_authors),
                        topic_author_level: Set(input.topic_author_level),
                        auto_subscribe_reply_participants: Set(
                            input.auto_subscribe_reply_participants
                        ),
                        reply_participant_level: Set(input.reply_participant_level),
                        revision: Set(next_revision),
                        updated_at: Set(now.into()),
                        ..Default::default()
                    })
                    .exec(&txn)
                    .await?;
                ensure_revision_update(result.rows_affected)?;
                forum_subscription_policy::Entity::find_by_id(tenant_id)
                    .one(&txn)
                    .await?
                    .ok_or_else(|| {
                        ForumError::Validation(
                            "Forum subscription policy disappeared during update".to_string(),
                        )
                    })?
            }
            None => {
                validate_new_revision(input.expected_revision)?;
                forum_subscription_policy::ActiveModel {
                    tenant_id: Set(tenant_id),
                    auto_subscribe_topic_authors: Set(input.auto_subscribe_topic_authors),
                    topic_author_level: Set(input.topic_author_level),
                    auto_subscribe_reply_participants: Set(input.auto_subscribe_reply_participants),
                    reply_participant_level: Set(input.reply_participant_level),
                    revision: Set(INITIAL_REVISION),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?
            }
        };
        txn.commit().await?;
        Ok(policy_response(model))
    }
}
