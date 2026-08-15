use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::{MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES};
use crate::entities::{forum_category, forum_category_lifecycle};
use crate::error::{ForumError, ForumResult};

use super::category_visibility::ForumCategoryVisibilityPolicyService;
use super::rbac::enforce_scope;

/// Maximum raw category roots accepted before overlap normalization.
///
/// This matches the existing bounded Search `category_ids` input. The expanded
/// subtree remains bounded by the canonical Forum category-tree node limit.
pub const MAX_FORUM_SEARCH_CATEGORY_ROOTS: usize = 10;

/// Forum-owned category scope that can be copied into Search `category_ids`.
///
/// Search remains owner-neutral: it receives already-authorized identifiers and
/// does not read Forum hierarchy, lifecycle, or visibility state itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumSearchCategoryScope {
    pub requested_category_ids: Vec<Uuid>,
    pub expanded_category_ids: Vec<Uuid>,
}

/// Resolves bounded, visibility-aware Forum category subtrees for Search callers.
pub struct ForumSearchCategoryScopeService {
    db: DatabaseConnection,
    visibility: ForumCategoryVisibilityPolicyService,
}

impl ForumSearchCategoryScopeService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            visibility: ForumCategoryVisibilityPolicyService::new(db.clone()),
            db,
        }
    }

    /// Expands exact category roots into deterministic visible subtree IDs.
    ///
    /// The caller must already have Forum category-list permission. Missing,
    /// foreign, archived, or viewer-hidden roots all fail as `CategoryNotFound`
    /// so this owner boundary does not expose a category-existence oracle.
    pub async fn expand_visible_subtrees(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_ids: &[Uuid],
    ) -> ForumResult<ForumSearchCategoryScope> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let requested_category_ids = normalize_requested_category_ids(category_ids)?;
        if requested_category_ids.is_empty() {
            return Ok(ForumSearchCategoryScope {
                requested_category_ids,
                expanded_category_ids: Vec::new(),
            });
        }

        let categories = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(forum_category::Column::Position)
            .order_by_asc(forum_category::Column::Id)
            .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
            .all(&self.db)
            .await?;
        if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum Search category scope exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        let hierarchy = CategoryHierarchy::from_ordered_nodes(
            categories
                .iter()
                .map(|category| (category.id, category.parent_id)),
        )?;
        let tenant_category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        let archived_category_ids = forum_category_lifecycle::Entity::find()
            .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
            .filter(forum_category_lifecycle::Column::CategoryId.is_in(tenant_category_ids))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|lifecycle| lifecycle.category_id)
            .collect::<HashSet<_>>();
        let hidden_category_ids = self
            .visibility
            .hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        let excluded_category_ids = archived_category_ids
            .union(&hidden_category_ids)
            .copied()
            .collect::<HashSet<_>>();

        let expanded_category_ids =
            hierarchy.expand_visible_subtrees(&requested_category_ids, &excluded_category_ids)?;

        Ok(ForumSearchCategoryScope {
            requested_category_ids,
            expanded_category_ids,
        })
    }
}

fn normalize_requested_category_ids(category_ids: &[Uuid]) -> ForumResult<Vec<Uuid>> {
    if category_ids.len() > MAX_FORUM_SEARCH_CATEGORY_ROOTS {
        return Err(ForumError::Validation(format!(
            "Forum Search category scope accepts at most {MAX_FORUM_SEARCH_CATEGORY_ROOTS} roots"
        )));
    }

    let mut seen = HashSet::with_capacity(category_ids.len());
    Ok(category_ids
        .iter()
        .copied()
        .filter(|category_id| seen.insert(*category_id))
        .collect())
}

struct CategoryHierarchy {
    parents: HashMap<Uuid, Option<Uuid>>,
    children_by_parent: HashMap<Uuid, Vec<Uuid>>,
}

impl CategoryHierarchy {
    fn from_ordered_nodes(
        nodes: impl IntoIterator<Item = (Uuid, Option<Uuid>)>,
    ) -> ForumResult<Self> {
        let ordered_nodes = nodes.into_iter().collect::<Vec<_>>();
        if ordered_nodes.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum Search category scope exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        let parents = ordered_nodes.iter().copied().collect::<HashMap<_, _>>();
        if parents.len() != ordered_nodes.len() {
            return Err(ForumError::Validation(
                "Forum Search category scope contains duplicate category identities".to_string(),
            ));
        }

        let mut children_by_parent = HashMap::<Uuid, Vec<Uuid>>::new();
        for (category_id, parent_id) in ordered_nodes {
            if let Some(parent_id) = parent_id {
                if !parents.contains_key(&parent_id) {
                    return Err(ForumError::Validation(format!(
                        "Forum Search category scope references missing or foreign parent {parent_id} for category {category_id}"
                    )));
                }
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(category_id);
            }
        }

        let hierarchy = Self {
            parents,
            children_by_parent,
        };
        hierarchy.validate()?;
        Ok(hierarchy)
    }

    fn validate(&self) -> ForumResult<()> {
        for category_id in self.parents.keys().copied() {
            let mut current = Some(category_id);
            let mut visited = HashSet::new();
            let mut depth = 0usize;
            while let Some(current_id) = current {
                if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
                    return Err(ForumError::Validation(format!(
                        "Forum Search category scope exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
                    )));
                }
                if !visited.insert(current_id) {
                    return Err(ForumError::Validation(
                        "Forum Search category scope contains a hierarchy cycle".to_string(),
                    ));
                }
                current = self.parents.get(&current_id).copied().ok_or_else(|| {
                    ForumError::Validation(format!(
                        "Forum Search category scope references missing category {current_id}"
                    ))
                })?;
                depth += 1;
            }
        }
        Ok(())
    }

    fn expand_visible_subtrees(
        &self,
        requested_category_ids: &[Uuid],
        excluded_category_ids: &HashSet<Uuid>,
    ) -> ForumResult<Vec<Uuid>> {
        for category_id in requested_category_ids {
            if !self.parents.contains_key(category_id)
                || self.has_excluded_ancestor(*category_id, excluded_category_ids)?
            {
                return Err(ForumError::CategoryNotFound(*category_id));
            }
        }

        let mut expanded = Vec::new();
        let mut visited = HashSet::new();
        for category_id in requested_category_ids {
            self.append_visible_preorder(
                *category_id,
                excluded_category_ids,
                &mut visited,
                &mut expanded,
            )?;
        }
        Ok(expanded)
    }

    fn has_excluded_ancestor(
        &self,
        category_id: Uuid,
        excluded_category_ids: &HashSet<Uuid>,
    ) -> ForumResult<bool> {
        let mut current = Some(category_id);
        while let Some(current_id) = current {
            if excluded_category_ids.contains(&current_id) {
                return Ok(true);
            }
            current = self.parents.get(&current_id).copied().ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum Search category scope references missing category {current_id}"
                ))
            })?;
        }
        Ok(false)
    }

    fn append_visible_preorder(
        &self,
        category_id: Uuid,
        excluded_category_ids: &HashSet<Uuid>,
        visited: &mut HashSet<Uuid>,
        expanded: &mut Vec<Uuid>,
    ) -> ForumResult<()> {
        if excluded_category_ids.contains(&category_id) || !visited.insert(category_id) {
            return Ok(());
        }
        if expanded.len() >= MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum Search category scope exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        expanded.push(category_id);
        if let Some(children) = self.children_by_parent.get(&category_id) {
            for child_id in children {
                self.append_visible_preorder(*child_id, excluded_category_ids, visited, expanded)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CategoryHierarchy, MAX_FORUM_SEARCH_CATEGORY_ROOTS, normalize_requested_category_ids,
    };
    use crate::error::ForumError;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    #[test]
    fn overlapping_roots_expand_once_in_deterministic_preorder() {
        let hierarchy = CategoryHierarchy::from_ordered_nodes([
            (id(1), None),
            (id(2), Some(id(1))),
            (id(3), Some(id(1))),
            (id(4), Some(id(2))),
        ])
        .expect("valid hierarchy");

        let expanded = hierarchy
            .expand_visible_subtrees(&[id(1), id(2)], &HashSet::new())
            .expect("scope expands");

        assert_eq!(expanded, vec![id(1), id(2), id(4), id(3)]);
    }

    #[test]
    fn excluded_branch_is_pruned_with_all_descendants() {
        let hierarchy = CategoryHierarchy::from_ordered_nodes([
            (id(1), None),
            (id(2), Some(id(1))),
            (id(3), Some(id(1))),
            (id(4), Some(id(2))),
        ])
        .expect("valid hierarchy");

        let expanded = hierarchy
            .expand_visible_subtrees(&[id(1)], &HashSet::from([id(2)]))
            .expect("scope expands");

        assert_eq!(expanded, vec![id(1), id(3)]);
    }

    #[test]
    fn selected_descendant_of_excluded_ancestor_is_non_oracular() {
        let hierarchy =
            CategoryHierarchy::from_ordered_nodes([(id(1), None), (id(2), Some(id(1)))])
                .expect("valid hierarchy");

        let error = hierarchy
            .expand_visible_subtrees(&[id(2)], &HashSet::from([id(1)]))
            .expect_err("hidden ancestry must fail closed");

        assert!(matches!(error, ForumError::CategoryNotFound(value) if value == id(2)));
    }

    #[test]
    fn raw_root_bound_is_checked_before_deduplication() {
        let repeated = vec![id(1); MAX_FORUM_SEARCH_CATEGORY_ROOTS + 1];
        let error =
            normalize_requested_category_ids(&repeated).expect_err("raw roots must remain bounded");

        assert!(matches!(error, ForumError::Validation(message) if message.contains("at most")));
    }

    #[test]
    fn hierarchy_cycle_fails_closed() {
        let error =
            CategoryHierarchy::from_ordered_nodes([(id(1), Some(id(2))), (id(2), Some(id(1)))])
                .err()
                .expect("cycle must fail");

        assert!(matches!(error, ForumError::Validation(message) if message.contains("cycle")));
    }
}
