impl ForumCategoryAudienceReadService {
    /// Returns the canonical public category tree after every richer inherited
    /// category audience layer has been evaluated.
    pub async fn tree_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryTreeResponse> {
        let security = SecurityContext::public_read();
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let viewer = ForumCategoryAudienceViewer::public();
        let mut tree = self
            .category_service
            .tree(
                tenant_id,
                security,
                CategoryTreeQuery {
                    locale: Some(locale.to_string()),
                    fallback_locale: fallback_locale.map(ToOwned::to_owned),
                },
            )
            .await?;

        let mut category_ids = Vec::with_capacity(tree.total_nodes as usize);
        collect_category_ids(&tree.roots, &mut category_ids);
        if category_ids.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category audience tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        let mut visible_ids = HashSet::with_capacity(category_ids.len());
        for category_id in category_ids {
            if self
                .visibility
                .is_category_visible(tenant_id, category_id, &viewer)
                .await?
            {
                visible_ids.insert(category_id);
            }
        }

        tree.roots = prune_category_nodes(tree.roots, &visible_ids);
        let (total_nodes, max_depth) = category_tree_stats(&tree.roots);
        tree.total_nodes = total_nodes;
        tree.max_depth = max_depth;
        Ok(tree)
    }
}
