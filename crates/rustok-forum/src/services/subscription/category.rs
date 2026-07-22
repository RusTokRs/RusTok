use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{
    ForumSubscriptionResponse, ForumSubscriptionTargetType, UpdateForumSubscriptionInput,
};
use crate::entities::{forum_category, forum_category_subscription};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::subscription::ForumSubscriptionLevel;

use super::SubscriptionService;
use super::helpers::{
    INITIAL_REVISION, category_response, ensure_revision_update, implicit_response,
    require_authenticated_user, resolve_preferences, validate_expected_revision,
    validate_new_revision,
};

impl SubscriptionService {
    #[instrument(skip(self, security))]
    pub async fn set_category_subscription(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.update_category_subscription(
            tenant_id,
            category_id,
            security,
            UpdateForumSubscriptionInput::watching(),
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn clear_category_subscription(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_category(tenant_id, category_id).await?;
        let existing = self
            .find_category_subscription(tenant_id, category_id, user_id)
            .await?;
        if let Some(existing) = existing {
            self.update_category_subscription(
                tenant_id,
                category_id,
                security,
                UpdateForumSubscriptionInput {
                    level: ForumSubscriptionLevel::Normal,
                    notify_mentions: None,
                    notify_replies: None,
                    notify_new_topics: None,
                    digest_mode: None,
                    expected_revision: Some(existing.revision),
                },
            )
            .await?;
        }
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn get_category_subscription(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumSubscriptionResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_category(tenant_id, category_id).await?;
        Ok(
            match self
                .find_category_subscription(tenant_id, category_id, user_id)
                .await?
            {
                Some(model) => category_response(model),
                None => implicit_response(
                    tenant_id,
                    ForumSubscriptionTargetType::Category,
                    category_id,
                    user_id,
                ),
            },
        )
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_category_subscription(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateForumSubscriptionInput,
    ) -> ForumResult<ForumSubscriptionResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_category(tenant_id, category_id).await?;
        let preferences = resolve_preferences(&input);
        let txn = self.db.begin().await?;
        let existing = forum_category_subscription::Entity::find()
            .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
            .filter(forum_category_subscription::Column::UserId.eq(user_id))
            .one(&txn)
            .await?;
        let now = Utc::now();

        let model = match existing {
            Some(existing) => {
                validate_expected_revision(input.expected_revision, existing.revision)?;
                let previous_revision = existing.revision;
                let next_revision = previous_revision.saturating_add(1);
                let result = forum_category_subscription::Entity::update_many()
                    .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
                    .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
                    .filter(forum_category_subscription::Column::UserId.eq(user_id))
                    .filter(forum_category_subscription::Column::Revision.eq(previous_revision))
                    .set(forum_category_subscription::ActiveModel {
                        level: Set(input.level),
                        notify_mentions: Set(preferences.notify_mentions),
                        notify_replies: Set(preferences.notify_replies),
                        notify_new_topics: Set(preferences.notify_new_topics),
                        digest_mode: Set(preferences.digest_mode),
                        revision: Set(next_revision),
                        updated_at: Set(now.into()),
                        ..Default::default()
                    })
                    .exec(&txn)
                    .await?;
                ensure_revision_update(result.rows_affected)?;
                forum_category_subscription::Entity::find()
                    .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
                    .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
                    .filter(forum_category_subscription::Column::UserId.eq(user_id))
                    .one(&txn)
                    .await?
                    .ok_or_else(|| {
                        ForumError::Validation(
                            "Forum category subscription disappeared during update".to_string(),
                        )
                    })?
            }
            None => {
                validate_new_revision(input.expected_revision)?;
                forum_category_subscription::ActiveModel {
                    category_id: Set(category_id),
                    user_id: Set(user_id),
                    tenant_id: Set(tenant_id),
                    level: Set(input.level),
                    notify_mentions: Set(preferences.notify_mentions),
                    notify_replies: Set(preferences.notify_replies),
                    notify_new_topics: Set(preferences.notify_new_topics),
                    digest_mode: Set(preferences.digest_mode),
                    last_notified_at: Set(None),
                    revision: Set(INITIAL_REVISION),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?
            }
        };
        txn.commit().await?;
        Ok(category_response(model))
    }

    pub async fn category_subscription_flags(
        &self,
        tenant_id: Uuid,
        category_ids: &[Uuid],
        user_id: Option<Uuid>,
    ) -> ForumResult<HashMap<Uuid, bool>> {
        let Some(user_id) = user_id else {
            return Ok(HashMap::new());
        };
        if category_ids.is_empty() {
            return Ok(HashMap::new());
        }
        Ok(forum_category_subscription::Entity::find()
            .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_category_subscription::Column::UserId.eq(user_id))
            .filter(forum_category_subscription::Column::CategoryId.is_in(category_ids.to_vec()))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|subscription| {
                (
                    subscription.category_id,
                    subscription.level.is_explicitly_subscribed(),
                )
            })
            .collect())
    }

    async fn find_category_subscription(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        user_id: Uuid,
    ) -> ForumResult<Option<forum_category_subscription::Model>> {
        Ok(forum_category_subscription::Entity::find()
            .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
            .filter(forum_category_subscription::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?)
    }

    async fn find_category(&self, tenant_id: Uuid, category_id: Uuid) -> ForumResult<()> {
        forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .map(|_| ())
            .ok_or(ForumError::CategoryNotFound(category_id))
    }
}
