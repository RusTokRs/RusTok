use std::collections::HashSet;

use super::*;

/// Duplicate ids in one bulk request are processed once: applying the same
/// lifecycle command twice would record a phantom second failure (or a
/// double delete attempt) for work that was asked for once.
fn deduped(post_ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    post_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect()
}

/// Upper bound of one bulk request. Bulk work is bounded so a single request cannot
/// amplify into an unbounded number of transactions and lifecycle events; see
/// `BulkOperationInput` for the transport-level validator that mirrors this bound.
pub(crate) const MAX_BULK_POST_IDS: usize = 100;

/// One bulk lifecycle command. Commands — not target statuses — are the unit
/// of bulk semantics: `Draft` is the target of two different commands
/// (unpublish from Published, restore from Archived), and conflating them
/// made a bulk *unpublish* silently restore archived posts.
#[derive(Clone, Copy)]
enum BulkLifecycleCommand {
    Publish,
    Unpublish,
    Archive,
    Restore,
}

impl PostService {
    /// Every bulk command rejects an oversized request before touching the database.
    fn ensure_bulk_batch_is_bounded(post_ids: &[Uuid]) -> BlogResult<()> {
        if post_ids.len() > MAX_BULK_POST_IDS {
            return Err(BlogError::validation(format!(
                "Bulk operations accept at most {MAX_BULK_POST_IDS} post ids per request"
            )));
        }
        Ok(())
    }

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
        let command = match target_status {
            BlogPostStatus::Published => BulkLifecycleCommand::Publish,
            // A target-status request for Draft means unpublish. Restore is a
            // distinct command (`bulk_restore_posts`): an unpublish request
            // must fail for archived posts, never resurrect them.
            BlogPostStatus::Draft => BulkLifecycleCommand::Unpublish,
            BlogPostStatus::Archived => BulkLifecycleCommand::Archive,
        };
        self.bulk_apply(tenant_id, post_ids, command, security)
            .await
    }

    /// Bulk unpublish: only Published posts move to Draft; archived posts are
    /// recorded as failures, exactly as the single unpublish command behaves.
    pub async fn bulk_unpublish_posts(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
        security: SecurityContext,
    ) -> BlogResult<BulkOperationResult> {
        self.bulk_apply(
            tenant_id,
            post_ids,
            BulkLifecycleCommand::Unpublish,
            security,
        )
        .await
    }

    /// Bulk restore: only Archived posts move to Draft.
    pub async fn bulk_restore_posts(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
        security: SecurityContext,
    ) -> BlogResult<BulkOperationResult> {
        self.bulk_apply(tenant_id, post_ids, BulkLifecycleCommand::Restore, security)
            .await
    }

    async fn bulk_apply(
        &self,
        tenant_id: Uuid,
        post_ids: &[Uuid],
        command: BulkLifecycleCommand,
        security: SecurityContext,
    ) -> BlogResult<BulkOperationResult> {
        Self::ensure_bulk_batch_is_bounded(post_ids)?;
        let mut result = BulkOperationResult::new();

        for &post_id in &deduped(post_ids) {
            match self
                .apply_bulk_command(tenant_id, post_id, command, security.clone())
                .await
            {
                Ok(()) => result.record_success(post_id),
                Err(error) => result.record_failure(post_id, error),
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
        Self::ensure_bulk_batch_is_bounded(post_ids)?;
        let mut result = BulkOperationResult::new();

        for &post_id in &deduped(post_ids) {
            match self.delete_post(tenant_id, post_id, security.clone()).await {
                Ok(()) => result.record_success(post_id),
                Err(error) => result.record_failure(post_id, error),
            }
        }

        Ok(result)
    }

    /// Apply one lifecycle command to one post (internal helper for bulk operations).
    async fn apply_bulk_command(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        command: BulkLifecycleCommand,
        security: SecurityContext,
    ) -> BlogResult<()> {
        match command {
            BulkLifecycleCommand::Publish => self.publish_post(tenant_id, post_id, security).await,
            BulkLifecycleCommand::Unpublish => {
                self.unpublish_post(tenant_id, post_id, security).await
            }
            BulkLifecycleCommand::Archive => {
                self.archive_post(tenant_id, post_id, security, None).await
            }
            BulkLifecycleCommand::Restore => self.restore_post(tenant_id, post_id, security).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(count: usize) -> Vec<Uuid> {
        (0..count).map(|_| Uuid::new_v4()).collect()
    }

    #[test]
    fn bulk_batches_are_bounded_before_any_work_starts() {
        PostService::ensure_bulk_batch_is_bounded(&ids(MAX_BULK_POST_IDS))
            .expect("a batch at the bound is accepted");
        let error = PostService::ensure_bulk_batch_is_bounded(&ids(MAX_BULK_POST_IDS + 1))
            .expect_err("a batch above the bound is refused");
        assert!(matches!(error, BlogError::Validation(_)));
        PostService::ensure_bulk_batch_is_bounded(&[]).expect("an empty batch is accepted");
    }

    /// A bulk response is a public transport: every failure it carries has to be the
    /// redacted public error, never `BlogError` display text. The invariant and database
    /// arms are the interesting ones — their display text names storage internals.
    #[test]
    fn bulk_failures_are_reported_through_the_public_boundary() {
        let post_id = Uuid::new_v4();
        let mut result = BulkOperationResult::new();

        result.record_failure(
            post_id,
            BlogError::invariant("Blog tag usage projection underflow for Taxonomy term 3f1c"),
        );
        result.record_failure(
            post_id,
            BlogError::Database(sea_orm::DbErr::Custom(
                "postgresql://secret-host/private".to_string(),
            )),
        );
        result.record_failure(post_id, BlogError::PostNotFound(post_id));

        assert_eq!(result.failed, 3);
        assert_eq!(result.succeeded, 0);
        assert!(result.succeeded_ids.is_empty());
        assert_eq!(result.failures.len(), 3);

        let invariant = &result.failures[0];
        assert_eq!(
            invariant.code,
            rustok_core::error::ErrorKind::Internal.error_code(),
            "internal failures collapse onto the single internal code"
        );
        assert!(
            !invariant.code.contains("BLOG_INVARIANT_VIOLATION"),
            "the internal error code is not a public code: {}",
            invariant.code
        );
        assert!(
            !invariant.message.contains("projection") && !invariant.message.contains("Taxonomy"),
            "invariant detail leaked: {}",
            invariant.message
        );

        let database = &result.failures[1];
        assert_eq!(
            database.code,
            rustok_core::error::ErrorKind::Internal.error_code()
        );
        assert!(
            !database.message.contains("postgresql") && !database.message.contains("secret-host"),
            "database detail leaked: {}",
            database.message
        );

        let not_found = &result.failures[2];
        assert_eq!(not_found.code, "POST_NOT_FOUND");
        assert!(
            !not_found.message.contains(&post_id.to_string()),
            "the failing id belongs in the structured field, not in the message: {}",
            not_found.message
        );
        assert!(
            result
                .failures
                .iter()
                .all(|failure| failure.post_id == post_id),
            "every failure stays addressable by its post id"
        );
    }
}
