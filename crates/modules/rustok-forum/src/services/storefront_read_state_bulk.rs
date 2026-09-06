impl ForumStorefrontReadStateService {
    /// Marks one bounded raw page from the current exact-visible category subtree.
    pub async fn mark_category_read_audience_visible(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        input: crate::services::read_tracking::MarkForumTopicsReadBatchInput,
    ) -> ForumResult<crate::services::read_tracking::MarkForumTopicsReadBatchResult> {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                crate::services::read_tracking::ForumVisibilityScopedReadStateService::with_audience_facts(
                    self.db.clone(),
                    facts,
                )
            }
            None => crate::services::read_tracking::ForumVisibilityScopedReadStateService::new(
                self.db.clone(),
            ),
        };
        service
            .mark_category_read_with_audience_context(
                tenant_id,
                category_id,
                security,
                context,
                input,
            )
            .await
    }

    /// Marks one bounded raw page from the current exact-visible tenant scope.
    pub async fn mark_all_read_audience_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        input: crate::services::read_tracking::MarkForumTopicsReadBatchInput,
    ) -> ForumResult<crate::services::read_tracking::MarkForumTopicsReadBatchResult> {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                crate::services::read_tracking::ForumVisibilityScopedReadStateService::with_audience_facts(
                    self.db.clone(),
                    facts,
                )
            }
            None => crate::services::read_tracking::ForumVisibilityScopedReadStateService::new(
                self.db.clone(),
            ),
        };
        service
            .mark_all_read_with_audience_context(tenant_id, security, context, input)
            .await
    }
}
