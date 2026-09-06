use crate::dto::CategoryTreeNode;

/// Converts an already-authorized canonical category tree into the bounded
/// Search category scope without re-evaluating owner visibility policy.
pub(super) fn expand_search_category_scope_from_visible_tree(
    roots: &[CategoryTreeNode],
    category_ids: &[Uuid],
) -> ForumResult<ForumSearchCategoryScope> {
    let requested_category_ids = normalize_requested_category_ids(category_ids)?;
    if requested_category_ids.is_empty() {
        return Ok(ForumSearchCategoryScope {
            requested_category_ids,
            expanded_category_ids: Vec::new(),
        });
    }

    let mut ordered_nodes = Vec::new();
    collect_active_visible_nodes(roots, &mut ordered_nodes);
    let hierarchy = CategoryHierarchy::from_ordered_nodes(ordered_nodes)?;
    let expanded_category_ids =
        hierarchy.expand_visible_subtrees(&requested_category_ids, &HashSet::new())?;

    Ok(ForumSearchCategoryScope {
        requested_category_ids,
        expanded_category_ids,
    })
}

fn collect_active_visible_nodes(
    nodes: &[CategoryTreeNode],
    output: &mut Vec<(Uuid, Option<Uuid>)>,
) {
    for node in nodes {
        if node.is_archived {
            continue;
        }
        output.push((node.id, node.parent_id));
        collect_active_visible_nodes(&node.children, output);
    }
}
