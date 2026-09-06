use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait,
    sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES};
use crate::entities::{forum_category, forum_category_policy};
use crate::error::{ForumError, ForumResult};
use crate::services::projection_invalidation::publish_forum_projection_scope_direct_in_tx;
use crate::services::rbac::enforce_scope;
use crate::visibility::ForumCategoryVisibility;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryVisibilityPolicy {
    pub category_id: Uuid,
    pub configured_visibility: Option<ForumCategoryVisibility>,
    pub effective_visibility: ForumCategoryVisibility,
    pub effective_from_category_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SetForumCategoryVisibilityPolicyInput {
    pub visibility: ForumCategoryVisibility,
}

/// Forum-owned category visibility policy.
///
/// The root default is public. A nullable row override may only narrow a category
/// and its descendants to authenticated users. Clearing a local override is
/// allowed only when the effective parent remains public, so a child can never
/// broaden an authenticated ancestor.
pub struct ForumCategoryVisibilityPolicyService {
    db: DatabaseConnection,
}

impl ForumCategoryVisibilityPolicyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumCategoryVisibilityPolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        CategoryVisibilitySnapshot::load(&self.db, tenant_id)
            .await?
            .policy(category_id)
    }

    pub async fn set(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: SetForumCategoryVisibilityPolicyInput,
    ) -> ForumResult<ForumCategoryVisibilityPolicy> {
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;

        let txn = self.db.begin().await?;
        super::category_audience::lock_category_tree_in_tx(&txn, tenant_id).await?;
        let snapshot = CategoryVisibilitySnapshot::load(&txn, tenant_id).await?;
        snapshot.policy(category_id)?;

        let requested_override = match input.visibility {
            ForumCategoryVisibility::Authenticated => Some(ForumCategoryVisibility::Authenticated),
            ForumCategoryVisibility::Public => {
                if snapshot.parent_effective(category_id)?.effective_visibility
                    == ForumCategoryVisibility::Authenticated
                {
                    return Err(ForumError::Validation(
                        "Forum category visibility cannot broaden an authenticated ancestor"
                            .to_string(),
                    ));
                }
                None
            }
        };

        let existing = forum_category_policy::Entity::find_by_id(category_id)
            .filter(forum_category_policy::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?;

        if requested_override.is_some() || existing.is_some() {
            forum_category_policy::Entity::insert(forum_category_policy::ActiveModel {
                category_id: Set(category_id),
                tenant_id: Set(tenant_id),
                allows_topics: existing
                    .as_ref()
                    .map(|policy| Set(policy.allows_topics))
                    .unwrap_or(NotSet),
                visibility_override: Set(requested_override),
                updated_at: Set(Utc::now().into()),
            })
            .on_conflict(
                OnConflict::column(forum_category_policy::Column::CategoryId)
                    .update_columns([
                        forum_category_policy::Column::TenantId,
                        forum_category_policy::Column::VisibilityOverride,
                        forum_category_policy::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(&txn)
            .await?;
        }

        let result = CategoryVisibilitySnapshot::load(&txn, tenant_id)
            .await?
            .policy(category_id)?;
        publish_forum_projection_scope_direct_in_tx(&txn, tenant_id, security.user_id).await?;
        txn.commit().await?;
        Ok(result)
    }

    pub(crate) async fn hidden_category_ids_for_viewer(
        &self,
        tenant_id: Uuid,
        is_authenticated: bool,
    ) -> ForumResult<Vec<Uuid>> {
        if is_authenticated {
            return Ok(Vec::new());
        }

        let snapshot = CategoryVisibilitySnapshot::load(&self.db, tenant_id).await?;
        let mut hidden = Vec::new();
        for category_id in snapshot.parents.keys().copied() {
            if snapshot.resolve(category_id)?.effective_visibility
                == ForumCategoryVisibility::Authenticated
            {
                hidden.push(category_id);
            }
        }
        hidden.sort_unstable();
        Ok(hidden)
    }

    /// Resolves one exact category without exposing whether a denied target exists.
    pub(crate) async fn is_category_visible_to_viewer(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        is_authenticated: bool,
    ) -> ForumResult<bool> {
        if is_authenticated {
            return Ok(forum_category::Entity::find_by_id(category_id)
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await?
                .is_some());
        }

        is_category_public_to_anonymous(&self.db, tenant_id, category_id).await
    }
}

pub(crate) async fn is_category_public_to_anonymous<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<bool>
where
    C: ConnectionTrait,
{
    let snapshot = CategoryVisibilitySnapshot::load(db, tenant_id).await?;
    match snapshot.resolve(category_id) {
        Ok(resolved) => Ok(resolved.effective_visibility == ForumCategoryVisibility::Public),
        Err(ForumError::CategoryNotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

struct CategoryVisibilitySnapshot {
    parents: HashMap<Uuid, Option<Uuid>>,
    overrides: HashMap<Uuid, ForumCategoryVisibility>,
}

impl CategoryVisibilitySnapshot {
    async fn load<C>(db: &C, tenant_id: Uuid) -> ForumResult<Self>
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
                "Forum category visibility tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        let parents = categories
            .into_iter()
            .map(|category| (category.id, category.parent_id))
            .collect::<HashMap<_, _>>();
        let mut overrides = HashMap::new();
        for policy in forum_category_policy::Entity::find()
            .filter(forum_category_policy::Column::TenantId.eq(tenant_id))
            .all(db)
            .await?
        {
            if let Some(visibility) = policy.visibility_override {
                if visibility != ForumCategoryVisibility::Authenticated {
                    return Err(ForumError::Validation(
                        "Forum category visibility storage contains a broadening override"
                            .to_string(),
                    ));
                }
                overrides.insert(policy.category_id, visibility);
            }
        }

        let snapshot = Self { parents, overrides };
        for category_id in snapshot.parents.keys().copied().collect::<Vec<_>>() {
            snapshot.resolve(category_id)?;
        }
        Ok(snapshot)
    }

    fn policy(&self, category_id: Uuid) -> ForumResult<ForumCategoryVisibilityPolicy> {
        let resolved = self.resolve(category_id)?;
        Ok(ForumCategoryVisibilityPolicy {
            category_id,
            configured_visibility: self.overrides.get(&category_id).copied(),
            effective_visibility: resolved.effective_visibility,
            effective_from_category_id: resolved.effective_from_category_id,
        })
    }

    fn parent_effective(&self, category_id: Uuid) -> ForumResult<ResolvedVisibility> {
        let parent_id = self
            .parents
            .get(&category_id)
            .copied()
            .ok_or(ForumError::CategoryNotFound(category_id))?;
        match parent_id {
            Some(parent_id) => self.resolve(parent_id),
            None => Ok(ResolvedVisibility {
                effective_visibility: ForumCategoryVisibility::Public,
                effective_from_category_id: None,
            }),
        }
    }

    fn resolve(&self, category_id: Uuid) -> ForumResult<ResolvedVisibility> {
        if !self.parents.contains_key(&category_id) {
            return Err(ForumError::CategoryNotFound(category_id));
        }

        let mut current = Some(category_id);
        let mut visited = HashSet::new();
        let mut depth = 0usize;
        while let Some(current_id) = current {
            if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
                return Err(ForumError::Validation(format!(
                    "Forum category visibility tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
                )));
            }
            if !visited.insert(current_id) {
                return Err(ForumError::Validation(
                    "Forum category visibility tree contains a hierarchy cycle".to_string(),
                ));
            }
            if let Some(visibility) = self.overrides.get(&current_id).copied() {
                return Ok(ResolvedVisibility {
                    effective_visibility: visibility,
                    effective_from_category_id: Some(current_id),
                });
            }
            current = self.parents.get(&current_id).copied().ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category visibility tree references missing category {current_id}"
                ))
            })?;
            depth += 1;
        }

        Ok(ResolvedVisibility {
            effective_visibility: ForumCategoryVisibility::Public,
            effective_from_category_id: None,
        })
    }
}

#[derive(Clone, Copy)]
struct ResolvedVisibility {
    effective_visibility: ForumCategoryVisibility,
    effective_from_category_id: Option<Uuid>,
}
