impl CategoryTreeService {
    pub(super) async fn read_with_hidden_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryTreeQuery,
        hidden_category_ids: &[Uuid],
    ) -> ForumResult<CategoryTreeResponse> {
        let mut response = self.read(tenant_id, security, query).await?;
        if hidden_category_ids.is_empty() || response.roots.is_empty() {
            return Ok(response);
        }

        let hidden = hidden_category_ids.iter().copied().collect::<HashSet<_>>();
        let (total_nodes, max_depth) = retain_visible_category_nodes(&mut response.roots, &hidden);
        response.total_nodes = total_nodes;
        response.max_depth = max_depth;
        Ok(response)
    }
}

fn retain_visible_category_nodes(
    nodes: &mut Vec<CategoryTreeNode>,
    hidden: &HashSet<Uuid>,
) -> (u32, u16) {
    nodes.retain(|node| !hidden.contains(&node.id));

    let mut total_nodes = 0u32;
    let mut max_depth = 0u16;
    for node in nodes {
        let (child_total, child_max_depth) =
            retain_visible_category_nodes(&mut node.children, hidden);
        node.children_count = node.children.len() as u32;
        node.has_children = !node.children.is_empty();
        total_nodes = total_nodes.saturating_add(1).saturating_add(child_total);
        max_depth = max_depth.max(node.depth).max(child_max_depth);
    }

    (total_nodes, max_depth)
}
