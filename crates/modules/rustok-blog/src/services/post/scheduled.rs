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
    /// Publish up to `limit` successful due drafts for **one tenant**.
    ///
    /// Failed rows remain scheduled for retry, but do not use up the success
    /// budget or trap later due drafts behind the first fixed-size page.
    /// Walking by (scheduled_at, id) bounds each database read and keeps
    /// forward progress even when the oldest rows cannot be published.
    /// A run can inspect more than `limit` rows if there are failures.
    pub async fn publish_scheduled_posts(
        &self,
        tenant_id: Uuid,
        limit: u32,
    ) -> BlogResult<Vec<Uuid>> {
        let limit = limit.clamp(1, MAX_SCHEDULED_PUBLISH_BATCH);
        let now = chrono::Utc::now();
        let mut published = Vec::new();
        let mut after: Option<(chrono::DateTime<chrono::FixedOffset>, Uuid)> = None;

        loop {
            let mut select = blog_post::Entity::find()
                .filter(blog_post::Column::TenantId.eq(tenant_id))
                .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Draft)))
                .filter(blog_post::Column::ScheduledAt.is_not_null())
                .filter(blog_post::Column::ScheduledAt.lte(now));

            if let Some((scheduled_at, id)) = &after {
                // A failed post is still due, so an offset-based retry would
                // repeatedly fetch it and could also skip rows removed by
                // successful publishes. Seek beyond the last *attempted* row.
                select = select.filter(
                    Condition::any()
                        .add(blog_post::Column::ScheduledAt.gt(scheduled_at.to_owned()))
                        .add(
                            Condition::all()
                                .add(blog_post::Column::ScheduledAt.eq(scheduled_at.to_owned()))
                                .add(blog_post::Column::Id.gt(*id)),
                        ),
                );
            }

            let due = select
                .order_by_asc(blog_post::Column::ScheduledAt)
                .order_by_asc(blog_post::Column::Id)
                .limit(u64::from(limit))
                .all(&self.db)
                .await
                .map_err(BlogError::from)?;
            if due.is_empty() {
                break;
            }
            let full_page = due.len() == limit as usize;

            for post in due {
                let scheduled_at = post.scheduled_at.ok_or_else(|| {
                    BlogError::invariant("Due Blog post has no scheduled_at timestamp")
                })?;
                after = Some((scheduled_at, post.id));
                let txn = self.db.begin().await.map_err(BlogError::from)?;

                // Same transition and version CAS as a manual publish.
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
                    match error {
                        BlogError::Conflict(message) => tracing::warn!(
                            tenant_id = %post.tenant_id,
                            post_id = %post.id,
                            error = %message,
                            "Scheduled Blog post changed concurrently; leaving it for retry"
                        ),
                        error => tracing::error!(
                            tenant_id = %post.tenant_id,
                            post_id = %post.id,
                            error = %error,
                            "Scheduled Blog post could not be published; leaving it for retry"
                        ),
                    }
                    drop(txn);
                    continue;
                }

                // Status transition and outbox announcement commit atomically.
                if let Err(error) = self
                    .event_bus
                    .publish_in_tx(
                        &txn,
                        post.tenant_id,
                        None,
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
                        "Scheduled Blog post announcement failed; leaving it for retry"
                    );
                    drop(txn);
                    continue;
                }

                if let Err(error) = txn.commit().await {
                    tracing::error!(
                        tenant_id = %post.tenant_id,
                        post_id = %post.id,
                        error = %error,
                        "Scheduled Blog post commit failed; leaving it for retry"
                    );
                    continue;
                }
                published.push(post.id);
                if published.len() == limit as usize {
                    return Ok(published);
                }
            }

            if !full_page {
                break;
            }
        }

        Ok(published)
    }

    /// Count due drafts for one tenant.
    pub async fn count_due_scheduled_posts(&self, tenant_id: Uuid) -> BlogResult<u64> {
        use sea_orm::PaginatorTrait;
        let now = chrono::Utc::now();
        blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Draft)))
            .filter(blog_post::Column::ScheduledAt.is_not_null())
            .filter(blog_post::Column::ScheduledAt.lte(now))
            .count(&self.db)
            .await
            .map_err(BlogError::from)
    }

    /// List all distinct tenants with due scheduled drafts.
    pub async fn list_tenants_with_due_scheduled_posts(&self) -> BlogResult<Vec<Uuid>> {
        let now = chrono::Utc::now();
        let posts = blog_post::Entity::find()
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Draft)))
            .filter(blog_post::Column::ScheduledAt.is_not_null())
            .filter(blog_post::Column::ScheduledAt.lte(now))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        let mut tenants: Vec<Uuid> = posts.into_iter().map(|p| p.tenant_id).collect();
        tenants.sort_unstable();
        tenants.dedup();
        Ok(tenants)
    }
}
