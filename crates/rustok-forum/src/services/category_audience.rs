use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
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
use crate::dto::{MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES};
use crate::entities::{
    forum_category, forum_category_audience_channel, forum_category_audience_group,
    forum_category_audience_policy, forum_category_audience_role,
    forum_category_audience_user::{self, ForumCategoryAudienceUserEffect},
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryAudiencePolicyLayer {
    pub category_id: Uuid,
    pub constraints: ForumAudienceConstraints,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryAudiencePolicy {
    pub category_id: Uuid,
    pub configured_constraints: Option<ForumAudienceConstraints>,
    /// Root-to-target non-empty layers. Every layer must allow the viewer.
    pub effective_layers: Vec<ForumCategoryAudiencePolicyLayer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SetForumCategoryAudiencePolicyInput {
    /// An empty constraint set clears the local layer and restores inheritance.
    pub constraints: ForumAudienceConstraints,
}

/// Forum-owned persistence for richer category audience layers.
///
/// Each category contributes at most one normalized local layer. Layers inherit
/// root-to-leaf as a conjunction, so a child layer can narrow but never broaden
/// an ancestor. Evaluation and owner-read composition remain separate slices.
pub struct ForumCategoryAudiencePolicyService {
    db: DatabaseConnection,
}

impl ForumCategoryAudiencePolicyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Policy details may contain explicit user and group identifiers and are
    /// therefore restricted to category managers rather than public readers.
    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumCategoryAudiencePolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let result = load_category_audience_policy(&txn, tenant_id, category_id).await?;
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
    forum_category_audience_role::Entity::insert_many(
        constraints
            .roles_any
            .iter()
            .cloned()
            .map(|role| forum_category_audience_role::ActiveModel {
                tenant_id: Set(tenant_id),
                category_id: Set(category_id),
                role: Set(role),
            })
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
    forum_category_audience_channel::Entity::insert_many(
        constraints
            .channel_members_any
            .iter()
            .cloned()
            .map(
                |channel_slug| forum_category_audience_channel::ActiveModel {
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
    forum_category_audience_group::Entity::insert_many(
        constraints
            .group_members_any
            .iter()
            .copied()
            .map(|group_id| forum_category_audience_group::ActiveModel {
                tenant_id: Set(tenant_id),
                category_id: Set(category_id),
                group_id: Set(group_id),
            })
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
        forum_category_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            category_id: Set(category_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Allow),
        }
    }));
    rows.extend(constraints.deny_user_ids.iter().copied().map(|user_id| {
        forum_category_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            category_id: Set(category_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Deny),
        }
    }));
    if rows.is_empty() {
        return Ok(());
    }
    forum_category_audience_user::Entity::insert_many(rows)
        .exec(txn)
        .await?;
    Ok(())
}

pub(crate) async fn load_category_audience_policy<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<ForumCategoryAudiencePolicy>
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
                .map(|constraints| ForumCategoryAudiencePolicyLayer {
                    category_id: ancestor_id,
                    constraints,
                })
        })
        .collect();

    Ok(ForumCategoryAudiencePolicy {
        category_id,
        configured_constraints,
        effective_layers,
    })
}

pub(crate) async fn load_category_ancestor_ids<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<Vec<Uuid>>
where
    C: ConnectionTrait,
{
    let categories = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(forum_category::Column::Id)
        .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
        .all(db)
        .await?;
    if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum category audience tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
        )));
    }

    let parents = categories
        .into_iter()
        .map(|category| (category.id, category.parent_id))
        .collect::<HashMap<_, _>>();
    if !parents.contains_key(&category_id) {
        return Err(ForumError::CategoryNotFound(category_id));
    }

    let mut current = Some(category_id);
    let mut ancestors = Vec::new();
    let mut visited = HashSet::new();
    let mut depth = 0usize;
    while let Some(current_id) = current {
        if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
            return Err(ForumError::Validation(format!(
                "Forum category audience tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
            )));
        }
        if !visited.insert(current_id) {
            return Err(ForumError::Validation(
                "Forum category audience tree contains a hierarchy cycle".to_string(),
            ));
        }
        ancestors.push(current_id);
        current = parents.get(&current_id).copied().ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category audience tree references missing or foreign category {current_id}"
            ))
        })?;
        depth += 1;
    }
    ancestors.reverse();
    Ok(ancestors)
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

    let policies = forum_category_audience_policy::Entity::find()
        .filter(forum_category_audience_policy::Column::TenantId.eq(tenant_id))
        .filter(forum_category_audience_policy::Column::CategoryId.is_in(category_ids.to_vec()))
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
                        "Forum category audience storage contains an invalid trust level"
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
    let roles = forum_category_audience_role::Entity::find()
        .filter(forum_category_audience_role::Column::TenantId.eq(tenant_id))
        .filter(forum_category_audience_role::Column::CategoryId.is_in(category_ids.to_vec()))
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
    let channels = forum_category_audience_channel::Entity::find()
        .filter(forum_category_audience_channel::Column::TenantId.eq(tenant_id))
        .filter(forum_category_audience_channel::Column::CategoryId.is_in(category_ids.to_vec()))
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
    let groups = forum_category_audience_group::Entity::find()
        .filter(forum_category_audience_group::Column::TenantId.eq(tenant_id))
        .filter(forum_category_audience_group::Column::CategoryId.is_in(category_ids.to_vec()))
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
    let users = forum_category_audience_user::Entity::find()
        .filter(forum_category_audience_user::Column::TenantId.eq(tenant_id))
        .filter(forum_category_audience_user::Column::CategoryId.is_in(category_ids.to_vec()))
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
                "Forum category audience storage contains an empty local layer".to_string(),
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
            "Forum category audience relation is missing its local policy layer".to_string(),
        )
    })
}

fn ensure_storage_bound(actual: usize, maximum: usize, label: &str) -> ForumResult<()> {
    if actual > maximum {
        return Err(ForumError::Validation(format!(
            "Forum category audience storage exceeds the bounded {label} limit of {maximum}"
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

pub(crate) async fn lock_category_tree_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [tenant_id.to_string().into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum category audience policy does not support {backend:?}"
        ))),
    }
}
