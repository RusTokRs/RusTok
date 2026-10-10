use super::*;

impl PostService {
    /// Bulk transition multiple posts to a target status.
    ///
    /// Each post is processed independently. Posts that cannot be transitioned
    /// (wrong current status, permission denied, etc.) are recorded as failures
    /// but do not abort the entire operation.
    pub async fn bulk_transition_posts(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
        target_status: BlogPostStatus,
        security: SecurityContext,
    ) -> BlogResult<BulkOperationResult> {
        let mut result = BulkOperationResult::new();

        for &post_id in post_ids {
            match self.transition_post(tenant_id, post_id, target_status, security.clone()).await {
                Ok(()) => result.record_success(post_id),
                Err(e) => result.record_failure(post_id, e.to_string()),
            }
        }

        Ok(result)
    }

    /// Bulk delete multiple posts.
    ///
    /// Only posts in Draft or Archived status can be deleted. Published posts
    /// must be unpublished first. Each post is processed independently.
    pub async fn bulk_delete_posts(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
        security: SecurityContext,
    ) -> BlogResult<BulkOperationResult> {
        let mut result = BulkOperationResult::new();

        for &post_id in post_ids {
            match self.delete_post(tenant_id, post_id, security.clone()).await {
                Ok(()) => result.record_success(post_id),
                Err(e) => result.record_failure(post_id, e.to_string()),
            }
        }

        Ok(result)
    }

    /// Transition a single post to a target status (internal helper for bulk operations).
    async fn transition_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        target_status: BlogPostStatus,
        security: SecurityContext,
    ) -> BlogResult<()> {
        match target_status {
            BlogPostStatus::Published => {
                self.publish_post(tenant_id, post_id, security).await
            }
            BlogPostStatus::Draft => {
                self.unpublish_post(tenant_id, post_id, security).await
            }
            BlogPostStatus::Archived => {
                self.archive_post(tenant_id, post_id, security, None).await
            }
        }
    }
}
