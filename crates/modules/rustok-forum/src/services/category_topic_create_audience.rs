use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::audience::ForumAudienceConstraints;
use crate::entities::forum_category_topic_create_audience_policy;
use crate::error::ForumResult;
use crate::services::audience_policy::{AudienceLayer, AudiencePolicy, CategoryTopicCreateScope};
use crate::services::category_audience::{load_category_ancestor_ids, lock_category_tree_in_tx};
use crate::services::rbac::enforce_scope;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryTopicCreateAudiencePolicyLayer {
    pub category_id: Uuid,
    pub constraints: ForumAudienceConstraints,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryTopicCreateAudiencePolicy {
    pub category_id: Uuid,
    pub configured_constraints: Option<ForumAudienceConstraints>,
    /// Ordered root-to-target non-empty layers. Every layer remains required.
    pub effective_layers: Vec<ForumCategoryTopicCreateAudiencePolicyLayer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SetForumCategoryTopicCreateAudiencePolicyInput {
    /// An empty constraint set clears the local layer and restores inheritance.
    pub constraints: ForumAudienceConstraints,
}

/// Forum-owned persistence for category topic-creation audience layers.
///
/// This policy is separate from content visibility. Every category contributes at
/// most one normalized local topic-create layer, and root-to-leaf inheritance is a
/// conjunction. This slice publishes managed owner persistence only; command-time
/// enforcement remains a separate composition step.
pub struct ForumCategoryTopicCreateAudiencePolicyService {
    db: DatabaseConnection,
}

impl ForumCategoryTopicCreateAudiencePolicyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumCategoryTopicCreateAudiencePolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let result =
            load_category_topic_create_audience_policy(&txn, tenant_id, category_id).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: SetForumCategoryTopicCreateAudiencePolicyInput,
    ) -> ForumResult<ForumCategoryTopicCreateAudiencePolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let constraints = input.constraints.normalize()?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        load_category_ancestor_ids(&txn, tenant_id, category_id).await?;

        forum_category_topic_create_audience_policy::Entity::delete_many()
            .filter(forum_category_topic_create_audience_policy::Column::TenantId.eq(tenant_id))
            .filter(forum_category_topic_create_audience_policy::Column::CategoryId.eq(category_id))
            .exec(&txn)
            .await?;

        if !constraints_are_empty(&constraints) {
            forum_category_topic_create_audience_policy::ActiveModel {
                tenant_id: Set(tenant_id),
                category_id: Set(category_id),
                minimum_trust_level: Set(constraints.minimum_trust_level.map(i16::from)),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;

            insert_roles(&txn, tenant_id, category_id, &constraints).await?;
            insert_channels(&txn, tenant_id, category_id, &constraints).await?;
            insert_groups(&txn, tenant_id, category_id, &constraints).await?;
            insert_users(&txn, tenant_id, category_id, &constraints).await?;
        }

        let result =
            load_category_topic_create_audience_policy(&txn, tenant_id, category_id).await?;
        txn.commit().await?;
        Ok(result)
    }
}

async fn insert_roles(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    subject_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    crate::services::audience_storage::insert_roles_raw(
        txn,
        "forum_category_topic_create_audience_roles",
        tenant_id,
        subject_id,
        "category_id",
        &constraints
            .roles_any
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>(),
    )
    .await
}

async fn insert_channels(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    subject_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    crate::services::audience_storage::insert_channels_raw(
        txn,
        "forum_category_topic_create_audience_channels",
        tenant_id,
        subject_id,
        "category_id",
        &constraints.channel_members_any,
    )
    .await
}

async fn insert_groups(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    subject_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    crate::services::audience_storage::insert_groups_raw(
        txn,
        "forum_category_topic_create_audience_groups",
        tenant_id,
        subject_id,
        "category_id",
        &constraints.group_members_any,
    )
    .await
}

async fn insert_users(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    subject_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    crate::services::audience_storage::insert_users_raw(
        txn,
        "forum_category_topic_create_audience_users",
        tenant_id,
        subject_id,
        "category_id",
        &constraints.allow_user_ids,
        "allow",
    )
    .await?;
    crate::services::audience_storage::insert_users_raw(
        txn,
        "forum_category_topic_create_audience_users",
        tenant_id,
        subject_id,
        "category_id",
        &constraints.deny_user_ids,
        "deny",
    )
    .await
}

pub(crate) async fn load_category_topic_create_audience_policy<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<ForumCategoryTopicCreateAudiencePolicy>
where
    C: ConnectionTrait,
{
    let generic =
        load_category_topic_create_audience_policy_generic(db, tenant_id, category_id).await?;
    Ok(ForumCategoryTopicCreateAudiencePolicy {
        category_id: generic.subject_id,
        configured_constraints: generic.configured_constraints,
        effective_layers: generic
            .effective_layers
            .into_iter()
            .map(|l| ForumCategoryTopicCreateAudiencePolicyLayer {
                category_id: l.subject_id,
                constraints: l.constraints,
            })
            .collect(),
    })
}

/// Ideal generic store: single `AudiencePolicy<Scope>` replaces 4× copy-paste `ForumCategoryTopicCreateAudiencePolicy`.
pub(crate) async fn load_category_topic_create_audience_policy_generic<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<AudiencePolicy<CategoryTopicCreateScope>>
where
    C: ConnectionTrait,
{
    let ancestor_ids = load_category_ancestor_ids(db, tenant_id, category_id).await?;
    let layers = load_local_layers(db, tenant_id, &ancestor_ids).await?;
    let configured_constraints = layers.get(&category_id).cloned();
    let effective_layers = ancestor_ids
        .into_iter()
        .filter_map(|ancestor_id| {
            layers
                .get(&ancestor_id)
                .cloned()
                .map(|constraints| AudienceLayer::new(ancestor_id, constraints))
        })
        .collect();
    Ok(AudiencePolicy::new(
        category_id,
        configured_constraints,
        effective_layers,
    ))
}

async fn load_local_layers<C>(
    db: &C,
    tenant_id: Uuid,
    subject_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, ForumAudienceConstraints>>
where
    C: ConnectionTrait,
{
    crate::services::audience_storage::load_audience_layers_raw(
        db,
        tenant_id,
        subject_ids,
        "forum_category_topic_create_audience",
        "category_id",
    )
    .await
}

fn constraints_are_empty(constraints: &ForumAudienceConstraints) -> bool {
    constraints.roles_any.is_empty()
        && constraints.minimum_trust_level.is_none()
        && constraints.channel_members_any.is_empty()
        && constraints.group_members_any.is_empty()
        && constraints.allow_user_ids.is_empty()
        && constraints.deny_user_ids.is_empty()
}
