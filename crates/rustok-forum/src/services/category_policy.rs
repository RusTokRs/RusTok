use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
    sea_query::OnConflict,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{CategoryTopicPolicyResponse, UpdateCategoryTopicPolicyInput};
use crate::entities::{forum_category, forum_category_policy};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

pub(super) struct CategoryTopicPolicyService {
    db: DatabaseConnection,
}

impl CategoryTopicPolicyService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(super) async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategoryTopicPolicyResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        ensure_category_exists(&self.db, tenant_id, category_id).await?;
        let policy = forum_category_policy::Entity::find_by_id(category_id)
            .filter(forum_category_policy::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;
        Ok(CategoryTopicPolicyResponse {
            category_id,
            allows_topics: policy.map(|policy| policy.allows_topics).unwrap_or(true),
        })
    }

    pub(super) async fn set(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryTopicPolicyInput,
    ) -> ForumResult<CategoryTopicPolicyResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let txn = self.db.begin().await?;
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        forum_category_policy::Entity::insert(forum_category_policy::ActiveModel {
            category_id: Set(category.id),
            tenant_id: Set(tenant_id),
            allows_topics: Set(input.allows_topics),
            updated_at: Set(Utc::now().into()),
        })
        .on_conflict(
            OnConflict::column(forum_category_policy::Column::CategoryId)
                .update_columns([
                    forum_category_policy::Column::TenantId,
                    forum_category_policy::Column::AllowsTopics,
                    forum_category_policy::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(&txn)
        .await?;
        txn.commit().await?;

        Ok(CategoryTopicPolicyResponse {
            category_id,
            allows_topics: input.allows_topics,
        })
    }

    pub(crate) async fn flags_for_categories(
        &self,
        tenant_id: Uuid,
        category_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, bool>> {
        if category_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let policies = forum_category_policy::Entity::find()
            .filter(forum_category_policy::Column::TenantId.eq(tenant_id))
            .filter(forum_category_policy::Column::CategoryId.is_in(category_ids.to_vec()))
            .all(&self.db)
            .await?;
        Ok(policies
            .into_iter()
            .map(|policy| (policy.category_id, policy.allows_topics))
            .collect())
    }
}

async fn ensure_category_exists(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let exists = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    if exists.is_none() {
        return Err(ForumError::CategoryNotFound(category_id));
    }
    Ok(())
}
