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
    category_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    if constraints.roles_any.is_empty() {
        return Ok(());
    }
    forum_category_moderation_audience_role::Entity::insert_many(
        constraints
            .roles_any
            .iter()
            .cloned()
            .map(
                |role| forum_category_moderation_audience_role::ActiveModel {
                    tenant_id: Set(tenant_id),
                    category_id: Set(category_id),
                    role: Set(role),
                },
            )
            .collect::<Vec<_>>(),
    )
    .exec(txn)
    .await?;
    Ok(())
}

async fn insert_channels(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    if constraints.channel_members_any.is_empty() {
        return Ok(());
    }
    forum_category_moderation_audience_channel::Entity::insert_many(
        constraints
            .channel_members_any
            .iter()
            .cloned()
            .map(
                |channel_slug| forum_category_moderation_audience_channel::ActiveModel {
                    tenant_id: Set(tenant_id),
                    category_id: Set(category_id),
                    channel_slug: Set(channel_slug),
                },
            )
            .collect::<Vec<_>>(),
    )
    .exec(txn)
    .await?;
    Ok(())
}

async fn insert_groups(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    if constraints.group_members_any.is_empty() {
        return Ok(());
    }
    forum_category_moderation_audience_group::Entity::insert_many(
        constraints
            .group_members_any
            .iter()
            .copied()
            .map(
                |group_id| forum_category_moderation_audience_group::ActiveModel {
                    tenant_id: Set(tenant_id),
                    category_id: Set(category_id),
                    group_id: Set(group_id),
                },
            )
            .collect::<Vec<_>>(),
    )
    .exec(txn)
    .await?;
    Ok(())
}

async fn insert_users(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    let mut rows =
        Vec::with_capacity(constraints.allow_user_ids.len() + constraints.deny_user_ids.len());
    rows.extend(constraints.allow_user_ids.iter().copied().map(|user_id| {
        forum_category_moderation_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            category_id: Set(category_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Allow),
        }
    }));
    rows.extend(constraints.deny_user_ids.iter().copied().map(|user_id| {
        forum_category_moderation_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            category_id: Set(category_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Deny),
        }
    }));
    if rows.is_empty() {
        return Ok(());
    }
    forum_category_moderation_audience_user::Entity::insert_many(rows)
        .exec(txn)
        .await?;
    Ok(())
}

pub(crate) async fn load_category_moderation_audience_policy<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<ForumCategoryModerationAudiencePolicy>
where
    C: ConnectionTrait,
{
    let ancestor_ids = load_category_ancestor_ids(db, tenant_id, category_id).await?;
    let layers = load_local_layers(db, tenant_id, &ancestor_ids).await?;
    let configured_constraints = layers.get(&category_id).cloned();
    let effective_layers = ancestor_ids
        .into_iter()
        .filter_map(|ancestor_id| {
            layers.get(&ancestor_id).cloned().map(|constraints| {
                ForumCategoryModerationAudiencePolicyLayer {
                    category_id: ancestor_id,
                    constraints,
                }
            })
        })
        .collect();

    Ok(ForumCategoryModerationAudiencePolicy {
        category_id,
        configured_constraints,
        effective_layers,
    })
}

async fn load_local_layers<C>(
    db: &C,
    tenant_id: Uuid,
    category_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, ForumAudienceConstraints>>
where
    C: ConnectionTrait,
{
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let policies = forum_category_moderation_audience_policy::Entity::find()
        .filter(forum_category_moderation_audience_policy::Column::TenantId.eq(tenant_id))
        .filter(
            forum_category_moderation_audience_policy::Column::CategoryId
                .is_in(category_ids.to_vec()),
        )
        .limit((category_ids.len() + 1) as u64)
        .all(db)
        .await?;
    ensure_storage_bound(policies.len(), category_ids.len(), "policy layers")?;

    let mut layers = HashMap::with_capacity(policies.len());
    for policy in policies {
        let minimum_trust_level = policy
            .minimum_trust_level
            .map(|level| {
                u8::try_from(level).map_err(|_| {
                    ForumError::Validation(
                        "Forum category moderation audience storage contains an invalid trust level"
                            .to_string(),
                    )
                })
            })
            .transpose()?;
        layers.insert(
            policy.category_id,
            ForumAudienceConstraints {
                minimum_trust_level,
                ..ForumAudienceConstraints::default()
            },
        );
    }

    let maximum_roles = category_ids.len() * MAX_FORUM_AUDIENCE_ROLES;
    let roles = forum_category_moderation_audience_role::Entity::find()
        .filter(forum_category_moderation_audience_role::Column::TenantId.eq(tenant_id))
        .filter(
            forum_category_moderation_audience_role::Column::CategoryId
                .is_in(category_ids.to_vec()),
        )
        .limit((maximum_roles + 1) as u64)
        .all(db)
        .await?;
    ensure_storage_bound(roles.len(), maximum_roles, "role relations")?;
    for row in roles {
        layer_mut(&mut layers, row.category_id)?
            .roles_any
            .push(row.role);
    }

    let maximum_channels = category_ids.len() * MAX_FORUM_AUDIENCE_CHANNELS;
    let channels = forum_category_moderation_audience_channel::Entity::find()
        .filter(forum_category_moderation_audience_channel::Column::TenantId.eq(tenant_id))
        .filter(
            forum_category_moderation_audience_channel::Column::CategoryId
                .is_in(category_ids.to_vec()),
        )
        .limit((maximum_channels + 1) as u64)
        .all(db)
        .await?;
    ensure_storage_bound(channels.len(), maximum_channels, "channel relations")?;
    for row in channels {
        layer_mut(&mut layers, row.category_id)?
            .channel_members_any
            .push(row.channel_slug);
    }

    let maximum_groups = category_ids.len() * MAX_FORUM_AUDIENCE_GROUPS;
    let groups = forum_category_moderation_audience_group::Entity::find()
        .filter(forum_category_moderation_audience_group::Column::TenantId.eq(tenant_id))
        .filter(
            forum_category_moderation_audience_group::Column::CategoryId
                .is_in(category_ids.to_vec()),
        )
        .limit((maximum_groups + 1) as u64)
        .all(db)
        .await?;
    ensure_storage_bound(groups.len(), maximum_groups, "group relations")?;
    for row in groups {
        layer_mut(&mut layers, row.category_id)?
            .group_members_any
            .push(row.group_id);
    }

    let maximum_users = category_ids.len() * MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2;
    let users = forum_category_moderation_audience_user::Entity::find()
        .filter(forum_category_moderation_audience_user::Column::TenantId.eq(tenant_id))
        .filter(
            forum_category_moderation_audience_user::Column::CategoryId
                .is_in(category_ids.to_vec()),
        )
        .limit((maximum_users + 1) as u64)
        .all(db)
        .await?;
    ensure_storage_bound(users.len(), maximum_users, "explicit user relations")?;
    for row in users {
        let layer = layer_mut(&mut layers, row.category_id)?;
        match row.effect {
            ForumCategoryAudienceUserEffect::Allow => layer.allow_user_ids.push(row.user_id),
            ForumCategoryAudienceUserEffect::Deny => layer.deny_user_ids.push(row.user_id),
        }
    }

    for constraints in layers.values_mut() {
        *constraints = constraints.clone().normalize()?;
        if constraints_are_empty(constraints) {
            return Err(ForumError::Validation(
                "Forum category moderation audience storage contains an empty local layer"
                    .to_string(),
            ));
        }
    }

    Ok(layers)
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
