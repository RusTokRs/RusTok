use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::SecurityContext;

use rustok_api::{Action, Resource};

use crate::entities::{
    forum_category, forum_category_subscription, forum_topic, forum_topic_subscription,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

pub struct SubscriptionService {
    db: DatabaseConnection,
}

impl SubscriptionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security))]
    pub async fn set_category_subscription(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_category(tenant_id, category_id).await?;

        let txn = self.db.begin().await?;
        let existing = forum_category_subscription::Entity::find()
            .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
            .filter(forum_category_subscription::Column::UserId.eq(user_id))
            .one(&txn)
            .await?;

        if existing.is_none() {
            forum_category_subscription::ActiveModel {
                category_id: Set(category_id),
                user_id: Set(user_id),
                tenant_id: Set(tenant_id),
                created_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;
        }

        txn.commit().await?;
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

        let txn = self.db.begin().await?;
        forum_category_subscription::Entity::delete_many()
            .filter(forum_category_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_category_subscription::Column::CategoryId.eq(category_id))
            .filter(forum_category_subscription::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn set_topic_subscription(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_topic(tenant_id, topic_id).await?;

        let txn = self.db.begin().await?;
        let existing = forum_topic_subscription::Entity::find()
            .filter(forum_topic_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_subscription::Column::TopicId.eq(topic_id))
            .filter(forum_topic_subscription::Column::UserId.eq(user_id))
            .one(&txn)
            .await?;

        if existing.is_none() {
            forum_topic_subscription::ActiveModel {
                topic_id: Set(topic_id),
                user_id: Set(user_id),
                tenant_id: Set(tenant_id),
                created_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;
        }

        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn clear_topic_subscription(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = require_authenticated_user(&security)?;
        self.find_topic(tenant_id, topic_id).await?;

        let txn = self.db.begin().await?;
        forum_topic_subscription::Entity::delete_many()
            .filter(forum_topic_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_subscription::Column::TopicId.eq(topic_id))
            .filter(forum_topic_subscription::Column::UserId.eq(user_id))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
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
            .map(|subscription| (subscription.category_id, true))
            .collect())
    }

    pub async fn topic_subscription_flags(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
        user_id: Option<Uuid>,
    ) -> ForumResult<HashMap<Uuid, bool>> {
        let Some(user_id) = user_id else {
            return Ok(HashMap::new());
        };
        if topic_ids.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(forum_topic_subscription::Entity::find()
            .filter(forum_topic_subscription::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_subscription::Column::UserId.eq(user_id))
            .filter(forum_topic_subscription::Column::TopicId.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|subscription| (subscription.topic_id, true))
            .collect())
    }

    async fn find_category(&self, tenant_id: Uuid, category_id: Uuid) -> ForumResult<()> {
        let existing = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;
        existing
            .map(|_| ())
            .ok_or(ForumError::CategoryNotFound(category_id))
    }

    async fn find_topic(&self, tenant_id: Uuid, topic_id: Uuid) -> ForumResult<()> {
        let existing = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;
        existing
            .map(|_| ())
            .ok_or(ForumError::TopicNotFound(topic_id))
    }
}

fn require_authenticated_user(security: &SecurityContext) -> ForumResult<Uuid> {
    security.user_id.ok_or_else(|| {
        ForumError::forbidden("Authenticated user context is required for subscriptions")
    })
}
