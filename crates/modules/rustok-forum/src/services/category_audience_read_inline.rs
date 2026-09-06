impl ForumCategoryAudienceReadService {
    /// Exact selected-category check for storefront composition under the
    /// category-list permission boundary. This avoids coupling deep-link
    /// category selection to the currently rendered category page.
    pub async fn get_authenticated_storefront_list_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        category_id: Uuid,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let locale = context_locale(&context, "storefront selected category")?;
        let viewer = ForumCategoryAudienceViewer::authenticated(security.clone(), context)?;
        self.get_visible(
            tenant_id,
            security,
            viewer,
            category_id,
            &locale,
            fallback_locale,
        )
        .await
    }
}
