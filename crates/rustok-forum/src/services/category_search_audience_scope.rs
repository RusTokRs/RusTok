use std::collections::HashSet;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{CategoryTreeNode, CategoryTreeQuery, MAX_FORUM_CATEGORY_TREE_NODES};
use crate::error::{ForumError, ForumResult};

use super::category_audience_visibility::{
    ForumCategoryAudienceViewer, ForumCategoryAudienceVisibilityService,
};
use super::category_owner::CategoryService;
use super::category_search_scope::{
    ForumSearchCategoryScope, MAX_FORUM_SEARCH_CATEGORY_ROOTS,
};
use super::rbac::enforce_scope;

/// Forum-owned category subtree scope after the complete delivered category
/// audience decision has been applied for the exact viewer.
pub struct ForumSearchCategoryAudienceScopeService {
    categories: CategoryService,
    visibility: ForumCategoryAudienceVisibilityService,
}

impl ForumSearchCategoryAudienceScopeService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_optional_audience_facts(db, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        facts_port: SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, Some(facts_port))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            categories: CategoryService::new(db.clone()),
            visibility: ForumCategoryAudienceVisibilityService::new(db, facts_port),
        }
    }

    /// Expands public category roots after public/authenticated inheritance,
    /// richer audience layers, archive state, and ancestor pruning.
    pub async fn expand_public_visible_subtrees(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        category_ids: &[Uuid],
    ) -> ForumResult<ForumSearchCategoryScope> {
        self.expand_visible_subtrees(
            tenant_id,
            SecurityContext::public_read(),
            ForumCategoryAudienceViewer::public(),
            locale,
            fallback_locale,
            category_ids,
        )
        .await
    }

    /// Expands authenticated category roots through the exact request-bound
    /// audience facts context. Missing required facts fail closed.
    pub async fn expand_authenticated_visible_subtrees(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        fallback_locale: Option<&str>,
        category_ids: &[Uuid],
    ) -> ForumResult<ForumSearchCategoryScope> {
        let locale = context.locale.trim();
        if locale.is_empty() {
            return Err(ForumError::Validation(
                "Forum Search category audience context locale is unavailable".to_string(),
            ));
        }
        let viewer = ForumCategoryAudienceViewer::authenticated(security.clone(), context)?;
        self.expand_visible_subtrees(
            tenant_id,
            security,
            viewer,
            locale,
            fallback_locale,
            category_ids,
        )
        .await
    }

    async fn expand_visible_subtrees(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        viewer: ForumCategoryAudienceViewer,
        locale: &str,
        fallback_locale: Option<&str>,
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

        let tree = self
            .categories
            .tree(
                tenant_id,
                security,
                CategoryTreeQuery {
                    locale: Some(locale.to_string()),
                    fallback_locale: fallback_locale.map(ToOwned::to_owned),
                },
            )
            .await?;
        if tree.total_nodes > MAX_FORUM_CATEGORY_TREE_NODES as u32 {
            return Err(ForumError::Validation(format!(
                "Forum Search category audience scope exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        let mut candidate_ids = Vec::with_capacity(tree.total_nodes as usize);
        collect_category_ids(&tree.roots, &mut candidate_ids);
        let mut visible_ids = HashSet::with_capacity(candidate_ids.len());
        for category_id in candidate_ids {
            let node = find_category_node(&tree.roots, category_id)
                .expect("category identifier was collected from the same tree");
            if !node.is_archived
                && self
                    .visibility
                    .is_category_visible(tenant_id, category_id, &viewer)
                    .await?
            {
                visible_ids.insert(category_id);
            }
        }

        let visible_roots = prune_category_nodes(tree.roots, &visible_ids);
        let expanded_category_ids = expand_requested_subtrees(
            &visible_roots,
            &requested_category_ids,
        )?;

        Ok(ForumSearchCategoryScope {
            requested_category_ids,
            expanded_category_ids,
        })
    }
}

fn normalize_requested_category_ids(category_ids: &[Uuid]) -> ForumResult<Vec<Uuid>> {
    if category_ids.len() > MAX_FORUM_SEARCH_CATEGORY_ROOTS {
        return Err(ForumError::Validation(format!(
            "Forum Search category audience scope accepts at most {MAX_FORUM_SEARCH_CATEGORY_ROOTS} roots"
        )));
    }

    let mut seen = HashSet::with_capacity(category_ids.len());
    Ok(category_ids
        .iter()
        .copied()
        .filter(|category_id| seen.insert(*category_id))
        .collect())
}

fn collect_category_ids(nodes: &[CategoryTreeNode], output: &mut Vec<Uuid>) {
    for node in nodes {
        output.push(node.id);
        collect_category_ids(&node.children, output);
    }
}

fn find_category_node(nodes: &[CategoryTreeNode], category_id: Uuid) -> Option<&CategoryTreeNode> {
    for node in nodes {
        if node.id == category_id {
            return Some(node);
        }
        if let Some(found) = find_category_node(&node.children, category_id) {
            return Some(found);
        }
    }
    None
}

fn prune_category_nodes(
    nodes: Vec<CategoryTreeNode>,
    visible_ids: &HashSet<Uuid>,
) -> Vec<CategoryTreeNode> {
    nodes
        .into_iter()
        .filter_map(|mut node| {
            if !visible_ids.contains(&node.id) {
                return None;
            }
            node.children = prune_category_nodes(node.children, visible_ids);
            node.children_count = node.children.len() as u32;
            node.has_children = !node.children.is_empty();
            Some(node)
        })
        .collect()
}

fn expand_requested_subtrees(
    roots: &[CategoryTreeNode],
    requested_category_ids: &[Uuid],
) -> ForumResult<Vec<Uuid>> {
    let mut expanded = Vec::new();
    let mut visited = HashSet::new();
    for category_id in requested_category_ids {
        let node = find_category_node(roots, *category_id)
            .ok_or(ForumError::CategoryNotFound(*category_id))?;
        append_preorder(node, &mut visited, &mut expanded)?;
    }
    Ok(expanded)
}

fn append_preorder(
    node: &CategoryTreeNode,
    visited: &mut HashSet<Uuid>,
    expanded: &mut Vec<Uuid>,
) -> ForumResult<()> {
    if !visited.insert(node.id) {
        return Ok(());
    }
    if expanded.len() >= MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum Search category audience scope exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
        )));
    }
    expanded.push(node.id);
    for child in &node.children {
        append_preorder(child, visited, expanded)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        expand_requested_subtrees, normalize_requested_category_ids, prune_category_nodes,
    };
    use crate::dto::CategoryTreeNode;
    use crate::error::ForumError;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    fn node(value: u128, children: Vec<CategoryTreeNode>) -> CategoryTreeNode {
        CategoryTreeNode {
            id: id(value),
            parent_id: None,
            depth: 0,
            position: value as i32,
            requested_locale: "en".to_string(),
            effective_locale: "en".to_string(),
            available_locales: vec!["en".to_string()],
            name: value.to_string(),
            slug: value.to_string(),
            description: None,
            icon: None,
            color: None,
            moderated: false,
            allows_topics: true,
            archived_at: None,
            is_archived: false,
            topic_count: 0,
            reply_count: 0,
            is_subscribed: false,
            has_children: !children.is_empty(),
            children_count: children.len() as u32,
            breadcrumbs: Vec::new(),
            children,
        }
    }

    #[test]
    fn denied_parent_prunes_allowed_descendants() {
        let roots = vec![node(1, vec![node(2, Vec::new())])];
        let pruned = prune_category_nodes(roots, &HashSet::from([id(2)]));
        assert!(pruned.is_empty());
    }

    #[test]
    fn overlapping_visible_roots_expand_once() {
        let roots = vec![node(1, vec![node(2, vec![node(3, Vec::new())])])];
        let expanded = expand_requested_subtrees(&roots, &[id(1), id(2)])
            .expect("visible roots should expand");
        assert_eq!(expanded, vec![id(1), id(2), id(3)]);
    }

    #[test]
    fn denied_selected_root_is_non_oracular() {
        let error = expand_requested_subtrees(&[node(1, Vec::new())], &[id(2)])
            .expect_err("missing visible root must fail closed");
        assert!(matches!(error, ForumError::CategoryNotFound(value) if value == id(2)));
    }

    #[test]
    fn raw_root_bound_is_checked_before_deduplication() {
        let roots = vec![id(1); super::MAX_FORUM_SEARCH_CATEGORY_ROOTS + 1];
        let error = normalize_requested_category_ids(&roots)
            .expect_err("raw roots must remain bounded");
        assert!(matches!(error, ForumError::Validation(message) if message.contains("at most")));
    }
}
