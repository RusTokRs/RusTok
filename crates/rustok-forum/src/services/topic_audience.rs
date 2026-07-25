use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait,
    ConnectionTrait,
    DatabaseBackend,
    DatabaseConnection,
    DatabaseTransaction,
    EntityTrait,
    QueryFilter,
    QueryOrder,
    QuerySelect,
    Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::audience::{
    ForumAudienceConstraints, MAX_FORUM_AUDIENCE_CHANNELS,
    MAX_FORUM_AUDIENCE_EXPLICIT_USERS, MAX_FORUM_AUDIENCE_GROUPS,
    MAX_FORUM_AUDIENCE_ROLES,
};
use crate::dto::{MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES};
use crate::entities::{
    forum_category,
    forum_category_audience_channel,
    forum_category_audience_group,
    forum_category_audience_policy,
    forum_category_audience_role,
    forum_category_audience_user::{self, ForumCategoryAudienceUserEffect},
    forum_topic,
    forum_topic_audience_channel,
    forum_topic_audience_group,
    forum_topic_audience_policy,
    forum_topic_audience_role,
    forum_topic_audience_user,
};
use crate::error::{ForumError, ForumResult};
use crate::services::category_audience::ForumCategoryAudiencePolicyLayer;
use crate::services::rbac::enforce_scope;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicAudiencePolicy {
    pub topic_id: Uuid,
    pub category_id: Uuid,
    /// Ordered root-to-category layers. Every layer remains independently required.
    pub inherited_category_layers: Vec<ForumCategoryAudiencePolicyLayer>,
    /// Optional final topic layer. It can only narrow the inherited category policy.
    pub configured_constraints: Option<ForumAudienceConstraints>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SetForumTopicAudiencePolicyInput {
    /// An empty constraint set clears only the topic layer.
    pub constraints: ForumAudienceConstraints,
}

/// Forum-owned topic narrowing persistence.
///
/// Effective evaluation is the conjunction of every inherited category layer
/// followed by the optional topic layer. A topic rule therefore cannot broaden
/// any category rule, even when its local positive selectors match another actor.
pub struct ForumTopicAudiencePolicyService {
    db: DatabaseConnection,
}

impl ForumTopicAudiencePolicyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Policy details may contain explicit user and group identifiers and are
    /// restricted to topic managers rather than ordinary readers.
    pub async fn get(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumTopicAudiencePolicy> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        lock_topic_audience_in_tx(&txn, tenant_id, topic_id).await?;
        let topic = find_topic(&txn, tenant_id, topic_id).await?;
        let result = load_policy_for_topic(&txn, tenant_id, &topic).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: SetForumTopicAudiencePolicyInput,
    ) -> ForumResult<ForumTopicAudiencePolicy> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let constraints = input.constraints.normalize()?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        lock_topic_audience_in_tx(&txn, tenant_id, topic_id).await?;
        let topic = find_topic(&txn, tenant_id, topic_id).await?;
        load_category_layers(&txn, tenant_id, topic.category_id).await?;

        forum_topic_audience_policy::Entity::delete_many()
            .filter(forum_topic_audience_policy::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_audience_policy::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;

        if !constraints_are_empty(&constraints) {
            forum_topic_audience_policy::ActiveModel {
                tenant_id: Set(tenant_id),
                topic_id: Set(topic_id),
                minimum_trust_level: Set(constraints.minimum_trust_level.map(i16::from)),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&txn)
            .await?;

            insert_roles(&txn, tenant_id, topic_id, &constraints).await?;
            insert_channels(&txn, tenant_id, topic_id, &constraints).await?;
            insert_groups(&txn, tenant_id, topic_id, &constraints).await?;
            insert_users(&txn, tenant_id, topic_id, &constraints).await?;
        }

        let result = load_policy_for_topic(&txn, tenant_id, &topic).await?;
        txn.commit().await?;
        Ok(result)
    }
}

async fn insert_roles(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    if constraints.roles_any.is_empty() {
        return Ok(());
    }
    forum_topic_audience_role::Entity::insert_many(
        constraints
            .roles_any
            .iter()
            .cloned()
            .map(|role| forum_topic_audience_role::ActiveModel {
                tenant_id: Set(tenant_id),
                topic_id: Set(topic_id),
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
    topic_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    if constraints.channel_members_any.is_empty() {
        return Ok(());
    }
    forum_topic_audience_channel::Entity::insert_many(
        constraints
            .channel_members_any
            .iter()
            .cloned()
            .map(|channel_slug| forum_topic_audience_channel::ActiveModel {
                tenant_id: Set(tenant_id),
                topic_id: Set(topic_id),
                channel_slug: Set(channel_slug),
            })
            .collect::<Vec<_>>(),
    )
    .exec(txn)
    .await?;
    Ok(())
}

async fn insert_groups(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    if constraints.group_members_any.is_empty() {
        return Ok(());
    }
    forum_topic_audience_group::Entity::insert_many(
        constraints
            .group_members_any
            .iter()
            .copied()
            .map(|group_id| forum_topic_audience_group::ActiveModel {
                tenant_id: Set(tenant_id),
                topic_id: Set(topic_id),
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
    topic_id: Uuid,
    constraints: &ForumAudienceConstraints,
) -> ForumResult<()> {
    let mut rows = Vec::with_capacity(
        constraints.allow_user_ids.len() + constraints.deny_user_ids.len(),
    );
    rows.extend(constraints.allow_user_ids.iter().copied().map(|user_id| {
        forum_topic_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Allow),
        }
    }));
    rows.extend(constraints.deny_user_ids.iter().copied().map(|user_id| {
        forum_topic_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Deny),
        }
    }));
    if rows.is_empty() {
        return Ok(());
    }
    forum_topic_audience_user::Entity::insert_many(rows)
        .exec(txn)
        .await?;
    Ok(())
}

async fn load_policy_for_topic<C>(
    db: &C,
    tenant_id: Uuid,
    topic: &forum_topic::Model,
) -> ForumResult<ForumTopicAudiencePolicy>
where
    C: ConnectionTrait,
{
    let inherited_category_layers = load_category_layers(db, tenant_id, topic.category_id).await?;
    let configured_constraints = load_topic_layer(db, tenant_id, topic.id).await?;
    Ok(ForumTopicAudiencePolicy {
        topic_id: topic.id,
        category_id: topic.category_id,
        inherited_category_layers,
        configured_constraints,
    })
}

async fn load_category_layers<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<Vec<ForumCategoryAudiencePolicyLayer>>
where
    C: ConnectionTrait,
{
    let ancestor_ids = load_category_ancestor_ids(db, tenant_id, category_id).await?;
    let layers = load_category_local_layers(db, tenant_id, &ancestor_ids).await?;
    Ok(ancestor_ids
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
        .collect())
}

async fn load_category_ancestor_ids<C>(
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
            "Forum topic audience category tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
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
                "Forum topic audience category tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
            )));
        }
        if !visited.insert(current_id) {
            return Err(ForumError::Validation(
                "Forum topic audience category tree contains a hierarchy cycle".to_string(),
            ));
        }
        ancestors.push(current_id);
        current = parents.get(&current_id).copied().ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum topic audience category tree references missing or foreign category {current_id}"
            ))
        })?;
        depth += 1;
    }
    ancestors.reverse();
    Ok(ancestors)
}

async fn load_category_local_layers<C>(
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
    ensure_storage_bound(policies.len(), category_ids.len(), "category policy layers")?;

    let mut layers = HashMap::with_capacity(policies.len());
    for policy in policies {
        layers.insert(
            policy.category_id,
            ForumAudienceConstraints {
                minimum_trust_level: policy
                    .minimum_trust_level
                    .map(|level| {
                        u8::try_from(level).map_err(|_| {
                            ForumError::Validation(
                                "Forum category audience storage contains an invalid trust level"
                                    .to_string(),
                            )
                        })
                    })
                    .transpose()?,
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
    ensure_storage_bound(roles.len(), maximum_roles, "category role relations")?;
    for row in roles {
        category_layer_mut(&mut layers, row.category_id)?.roles_any.push(row.role);
    }

    let maximum_channels = category_ids.len() * MAX_FORUM_AUDIENCE_CHANNELS;
    let channels = forum_category_audience_channel::Entity::find()
        .filter(forum_category_audience_channel::Column::TenantId.eq(tenant_id))
        .filter(forum_category_audience_channel::Column::CategoryId.is_in(category_ids.to_vec()))
        .limit((maximum_channels + 1) as u64)
        .all(db)
        .await?;
    ensure_storage_bound(channels.len(), maximum_channels, "category channel relations")?;
    for row in channels {
        category_layer_mut(&mut layers, row.category_id)?
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
    ensure_storage_bound(groups.len(), maximum_groups, "category group relations")?;
    for row in groups {
        category_layer_mut(&mut layers, row.category_id)?
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
    ensure_storage_bound(users.len(), maximum_users, "category explicit user relations")?;
    for row in users {
        let layer = category_layer_mut(&mut layers, row.category_id)?;
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

async fn load_topic_layer<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<ForumAudienceConstraints>>
where
    C: ConnectionTrait,
{
    let policy = forum_topic_audience_policy::Entity::find_by_id((tenant_id, topic_id))
        .one(db)
        .await?;
    let roles = forum_topic_audience_role::Entity::find()
        .filter(forum_topic_audience_role::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_role::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_ROLES + 1) as u64)
        .all(db)
        .await?;
    let channels = forum_topic_audience_channel::Entity::find()
        .filter(forum_topic_audience_channel::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_channel::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_CHANNELS + 1) as u64)
        .all(db)
        .await?;
    let groups = forum_topic_audience_group::Entity::find()
        .filter(forum_topic_audience_group::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_group::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_GROUPS + 1) as u64)
        .all(db)
        .await?;
    let users = forum_topic_audience_user::Entity::find()
        .filter(forum_topic_audience_user::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_user::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2 + 1) as u64)
        .all(db)
        .await?;

    ensure_storage_bound(roles.len(), MAX_FORUM_AUDIENCE_ROLES, "topic role relations")?;
    ensure_storage_bound(
        channels.len(),
        MAX_FORUM_AUDIENCE_CHANNELS,
        "topic channel relations",
    )?;
    ensure_storage_bound(groups.len(), MAX_FORUM_AUDIENCE_GROUPS, "topic group relations")?;
    ensure_storage_bound(
        users.len(),
        MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2,
        "topic explicit user relations",
    )?;

    let Some(policy) = policy else {
        if !roles.is_empty() || !channels.is_empty() || !groups.is_empty() || !users.is_empty() {
            return Err(ForumError::Validation(
                "Forum topic audience relation is missing its local policy layer".to_string(),
            ));
        }
        return Ok(None);
    };

    let mut constraints = ForumAudienceConstraints {
        minimum_trust_level: policy
            .minimum_trust_level
            .map(|level| {
                u8::try_from(level).map_err(|_| {
                    ForumError::Validation(
                        "Forum topic audience storage contains an invalid trust level".to_string(),
                    )
                })
            })
            .transpose()?,
        ..ForumAudienceConstraints::default()
    };
    constraints.roles_any = roles.into_iter().map(|row| row.role).collect();
    constraints.channel_members_any = channels
        .into_iter()
        .map(|row| row.channel_slug)
        .collect();
    constraints.group_members_any = groups.into_iter().map(|row| row.group_id).collect();
    for row in users {
        match row.effect {
            ForumCategoryAudienceUserEffect::Allow => constraints.allow_user_ids.push(row.user_id),
            ForumCategoryAudienceUserEffect::Deny => constraints.deny_user_ids.push(row.user_id),
        }
    }
    let constraints = constraints.normalize()?;
    if constraints_are_empty(&constraints) {
        return Err(ForumError::Validation(
            "Forum topic audience storage contains an empty local layer".to_string(),
        ));
    }
    Ok(Some(constraints))
}

fn category_layer_mut(
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
            "Forum topic audience storage exceeds the bounded {label} limit of {maximum}"
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

async fn find_topic<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<forum_topic::Model>
where
    C: ConnectionTrait,
{
    forum_topic::Entity::find_by_id(topic_id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(ForumError::TopicNotFound(topic_id))
}

async fn lock_category_tree_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [tenant_id.to_string().into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic audience policy does not support {backend:?}"
        ))),
    }
}

async fn lock_topic_audience_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 5))",
                [format!("{tenant_id}:{topic_id}").into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic audience policy does not support {backend:?}"
        ))),
    }
}
