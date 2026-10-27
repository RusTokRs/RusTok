//! The scheduler-facing Blog owner command.
//!
//! `blog_scheduled_publish` (`src/services/scheduler.rs`) claims one tenant at a time and
//! calls this command, so the module owns the due-post scan while the host owns the timer
//! and the worker mode. Keeping the command next to its caller's contract is why it lives
//! here rather than with the request-path commands in `commands.rs`: it is operational,
//! never reachable from a transport, and the scheduler passes the system actor itself.

use super::commands::apply_status_transition_in_tx;
use super::*;

impl PostService {
    /// Publish the scheduled drafts of **one tenant** whose `scheduled_at` has passed.
    ///
    /// The host scheduler drives this command through the `blog_scheduled_publish`
    /// module-work registration (`src/services/scheduler.rs`) in host modes `full` and
    /// `worker`, one tenant per work item, so the command is shaped to be called
    /// repeatedly and safely:
    ///
    /// - the tenant is an explicit argument, so a scheduler iteration can never
    ///   reach another tenant's rows;
    /// - the batch is bounded by `limit` (`MAX_SCHEDULED_PUBLISH_BATCH`), oldest
    ///   schedule first, so a backlog drains across bounded iterations;
    /// - each post is published in its own transaction, and **any** per-post
    ///   failure (a concurrent edit conflict, a storage invariant, a transient
    ///   database error) skips that post instead of aborting the whole batch;
    ///   the failing post keeps its due schedule and is retried on the next
    ///   iteration, so one poisoned row cannot head-of-line block the tenant;
    /// - the actor is the system actor, so it must not be reachable from a
    ///   request path or an end-user transport.
    pub async fn publish_scheduled_posts(
        &self,
        tenant_id: Uuid,
        limit: u32,
    ) -> BlogResult<Vec<Uuid>> {
        let limit = limit.clamp(1, MAX_SCHEDULED_PUBLISH_BATCH);
        let now = chrono::Utc::now();

        let due = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Draft)))
            .filter(blog_post::Column::ScheduledAt.is_not_null())
            .filter(blog_post::Column::ScheduledAt.lte(now))
            .order_by_asc(blog_post::Column::ScheduledAt)
            .order_by_asc(blog_post::Column::Id)
            .limit(u64::from(limit))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let mut published = Vec::new();

        for post in due {
            let txn = self.db.begin().await.map_err(BlogError::from)?;

            // Same transition logic as a manual publish: it carries the version CAS,
            // so a post edited after the scan is skipped rather than overwritten.
            if let Err(error) = apply_status_transition_in_tx(
                &txn,
                post.tenant_id,
                post.id,
                post.version,
                BlogPostStatus::Published,
                Some(post.published_at.map(Into::into).unwrap_or(now)),
                None,
            )
            .await
            {
                // The batch is best-effort per post. Returning here would leave every
                // remaining due post of the tenant unpublished behind one bad row
                // (head-of-line blocking); the failed post keeps `scheduled_at`, so the
                // next iteration retries it after the rest of the backlog drains.
                match error {
                    BlogError::Conflict(message) => tracing::warn!(
                        tenant_id = %post.tenant_id,
                        post_id = %post.id,
                        error = %message,
                        "Scheduled Blog post changed concurrently; leaving it for the next scheduler iteration"
                    ),
                    error => tracing::error!(
                        tenant_id = %post.tenant_id,
                        post_id = %post.id,
                        error = %error,
                        "Scheduled Blog post could not be published; leaving it for the next scheduler iteration"
                    ),
                }
                drop(txn);
                continue;
            }

            // The status write and its announcement commit atomically: a rollback of
            // the event rolls the status back too, so a post is never silently
            // published without the event its consumers rely on.
            if let Err(error) = self
                .event_bus
                .publish_in_tx(
                    &txn,
                    post.tenant_id,
                    None, // System actor: this command is operational, never user-facing.
                    DomainEvent::BlogPostPublished {
                        post_id: post.id,
                        author_id: Some(post.author_id),
                    },
                )
                .await
            {
                tracing::error!(
                    tenant_id = %post.tenant_id,
                    post_id = %post.id,
                    error = %error,
                    "Scheduled Blog post could not be announced; leaving it for the next scheduler iteration"
                );
                drop(txn);
                continue;
            }

            if let Err(error) = txn.commit().await {
                tracing::error!(
                    tenant_id = %post.tenant_id,
                    post_id = %post.id,
                    error = %error,
                    "Scheduled Blog post commit failed; leaving it for the next scheduler iteration"
                );
                continue;
            }
            published.push(post.id);
        }

        Ok(published)
    }
}
