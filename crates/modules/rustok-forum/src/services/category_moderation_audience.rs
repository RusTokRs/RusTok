use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::audience::{
    ForumAudienceConstraints, MAX_FORUM_AUDIENCE_CHANNELS, MAX_FORUM_AUDIENCE_EXPLICIT_USERS,
    MAX_FORUM_AUDIENCE_GROUPS, MAX_FORUM_AUDIENCE_ROLES,
};
use crate::entities::{
    forum_category_audience_user::ForumCategoryAudienceUserEffect,
    forum_category_moderation_audience_channel, forum_category_moderation_audience_group,
    forum_category_moderation_audience_policy, forum_category_moderation_audience_role,
    forum_category_moderation_audience_user,
};
use crate::error::{ForumError, ForumResult};

use super::category_audience::{load_category_ancestor_ids, lock_category_tree_in_tx};
use super::rbac::enforce_scope;
use crate::services::audience_policy::{AudienceLayer, AudiencePolicy, CategoryModerationScope};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryModerationAudiencePolicyLayer {
    pub category_id: Uuid,
    pub constraints: ForumAudienceConstraints,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryModerationAudiencePolicy {
    pub category_id: Uuid,
    pub configured_constraints: Option<ForumAudienceConstraints>,
    /// Ordered root-to-target non-empty layers. Every layer remains required.
    pub effective_layers: Vec<ForumCategoryModerationAudiencePolicyLayer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SetForumCategoryModerationAudiencePolicyInput {
    /// An empty constraint set clears the local layer and restores inheritance.
    pub constraints: ForumAudienceConstraints,
}

/// Forum-owned persistence for category moderation audience layers.
///
/// This policy is separate from content visibility and posting eligibility. Every
/// category contributes at most one normalized local moderation layer, and the
/// effective root-to-leaf policy is a conjunction.
pub struct ForumCategoryModerationAudiencePolicyService {
    db: DatabaseConnection,
}

impl ForumCategoryModerationAudiencePolicyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumCategoryModerationAudiencePolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        lock_category_moderation_audience_in_tx(&txn, tenant_id, category_id).await?;
        let result = load_category_moderation_audience_policy(&txn, tenant_id, category_id).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: SetForumCategoryModerationAudiencePolicyInput,
    ) -> ForumResult<ForumCategoryModerationAudiencePolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let constraints = input.constraints.normalize()?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        lock_category_moderation_audience_in_tx(&txn, tenant_id, category_id).await?;
        load_category_ancestor_ids(&txn, tenant_id, category_id).await?;

        forum_category_moderation_audience_policy::Entity::delete_many()
            .filter(forum_category_moderation_audience_policy::Column::TenantId.eq(tenant_id))
            .filter(forum_category_moderation_audience_policy::Column::CategoryId.eq(category_id))
            .exec(&txn)
            .await?;

        if !constraints_are_empty(&constraints) {
            forum_category_moderation_audience_policy::ActiveModel {
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

        let result = load_category_moderation_audience_policy(&txn, tenant_id, category_id).await?;
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
        "forum_category_moderation_audience_roles",
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
        "forum_category_moderation_audience_channels",
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
        "forum_category_moderation_audience_groups",
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
        "forum_category_moderation_audience_users",
        tenant_id,
        subject_id,
        "category_id",
        &constraints.allow_user_ids,
        "allow",
    )
    .await?;
    crate::services::audience_storage::insert_users_raw(
        txn,
        "forum_category_moderation_audience_users",
        tenant_id,
        subject_id,
        "category_id",
        &constraints.deny_user_ids,
        "deny",
    )
    .await
}

pub(crate) async fn load_category_moderation_audience_policy<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<ForumCategoryModerationAudiencePolicy>
where
    C: ConnectionTrait,
{
    let generic = load_category_moderation_audience_policy_generic(db, tenant_id, category_id).await?;
    Ok(ForumCategoryModerationAudiencePolicy {
        category_id: generic.subject_id,
        configured_constraints: generic.configured_constraints,
        effective_layers: generic
            .effective_layers
            .into_iter()
            .map(|l| ForumCategoryModerationAudiencePolicyLayer {
                category_id: l.subject_id,
                constraints: l.constraints,
            })
            .collect(),
    })
}

/// Ideal generic store: single `AudiencePolicy<Scope>` replaces 4× copy-paste `ForumCategoryModerationAudiencePolicy`.
pub(crate) async fn load_category_moderation_audience_policy_generic<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<AudiencePolicy<CategoryModerationScope>>
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
    Ok(AudiencePolicy {
        subject_id: category_id,
        configured_constraints,
        effective_layers,
        _marker: std::marker::PhantomData,
    })
}

async fn load_local_layers<C>(
    db: &C,
    tenant_id: Uuid,
    subject_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, ForumAudienceConstraints>>
where
    C: ConnectionTrait,
{
    crate::services::audience_storage::load_audience_layers_raw(db, tenant_id, subject_ids, "forum_category_moderation_audience", "category_id").await
}

fn layer_mut(
    layers: &mut HashMap<Uuid, ForumAudienceConstraints>,
    category_id: Uuid,
) -> ForumResult<&mut ForumAudienceConstraints> {
    layers.get_mut(&category_id).ok_or_else(|| {
        ForumError::Validation(
            "Forum category moderation audience relation is missing its local policy layer"
                .to_string(),
        )
    })
}

fn ensure_storage_bound(actual: usize, maximum: usize, label: &str) -> ForumResult<()> {
    if actual > maximum {
        return Err(ForumError::Validation(format!(
            "Forum category moderation audience storage exceeds the bounded {label} limit of {maximum}"
        )));
    }
    Ok(())
}

fn constraints_are_empty(constraints: &ForumAudienceConstraints) -> bool {
    constraints.roles_any.is_empty()
        && constraints.minimum_trust_level.is_none()
        && constraints.channel_members_any.is_empty()
        && constraints.group_members_any.is_empty()
        && constraints.allow_user_ids.is_empty()
        && constraints.deny_user_ids.is_empty()
}

async fn lock_category_moderation_audience_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 5))",
                [format!("{tenant_id}:{category_id}:moderation").into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum category moderation audience policy does not support {backend:?}"
        ))),
    }
}
