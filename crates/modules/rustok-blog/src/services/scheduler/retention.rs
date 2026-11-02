//! The Blog comment projection ledger retention worker.
//!
//! Extracted from the scheduler entry file to keep each worker's discovery,
//! compaction and registration close to its own safety argument; the public
//! registration path stays `crate::services::scheduler::...` via the parent's
//! re-export.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource};
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistration, ModuleWorkScheduler};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::blog_comment_projection_delivery;
use super::{tenant_scoped_work_item, work_item_tenant};

/// Stable scheduler identity for Blog comment projection ledger retention.
pub const BLOG_COMMENT_PROJECTION_RETENTION_WORKER: &str = "blog_comment_projection_retention";
/// Deliveries processed before this window carry no replay information the
/// projection still consults (see the retention worker's safety argument).
const COMMENT_PROJECTION_DELIVERY_RETENTION_DAYS: i64 = 30;
/// Upper bound of ledger rows one retention iteration removes for one tenant.
const COMMENT_PROJECTION_RETENTION_BATCH: u64 = 200;

pub struct BlogCommentProjectionRetentionWorkAdapter {
    database: DatabaseConnection,
    // Fairness hint only, like the scheduled-publish cursor: the stale-delivery
    // predicate remains the durable source of truth.
    last_claimed_tenant: Arc<tokio::sync::Mutex<Option<Uuid>>>,
}

impl BlogCommentProjectionRetentionWorkAdapter {
    pub fn new(database: DatabaseConnection) -> Self {
        Self {
            database,
            last_claimed_tenant: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    /// Rotate over tenants that hold delivery rows past the retention window.
    async fn next_due_tenant(&self) -> Result<Option<Uuid>, ModuleWorkError> {
        let cutoff = (Utc::now() - Duration::days(COMMENT_PROJECTION_DELIVERY_RETENTION_DAYS))
            .fixed_offset();
        // An async mutex is held over discovery to prevent concurrent local
        // claims from advancing the fairness cursor based on the same state.
        let mut last = self.last_claimed_tenant.lock().await;
        let stale_tenants_query = || {
            blog_comment_projection_delivery::Entity::find()
                .select_only()
                .column(blog_comment_projection_delivery::Column::TenantId)
                .filter(blog_comment_projection_delivery::Column::ProcessedAt.lt(cutoff))
                .group_by(blog_comment_projection_delivery::Column::TenantId)
                .order_by_asc(blog_comment_projection_delivery::Column::TenantId)
        };

        let next = if let Some(last_tenant) = *last {
            let higher = stale_tenants_query()
                .filter(blog_comment_projection_delivery::Column::TenantId.gt(last_tenant))
                .into_tuple::<Uuid>()
                .one(&self.database)
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
            match higher {
                Some(tenant_id) => Some(tenant_id),
                None => stale_tenants_query()
                    .into_tuple::<Uuid>()
                    .one(&self.database)
                    .await
                    .map_err(|error| ModuleWorkError::Source(error.to_string()))?,
            }
        } else {
            stale_tenants_query()
                .into_tuple::<Uuid>()
                .one(&self.database)
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?
        };

        if let Some(tenant_id) = next {
            *last = Some(tenant_id);
        }
        Ok(next)
    }

    /// Compact up to [`COMMENT_PROJECTION_RETENTION_BATCH`] stale ledger rows of one
    /// tenant, always keeping each comment's frontier row.
    async fn compact_tenant(&self, tenant_id: Uuid) -> Result<u64, ModuleWorkError> {
        let cutoff = (Utc::now() - Duration::days(COMMENT_PROJECTION_DELIVERY_RETENTION_DAYS))
            .fixed_offset();

        let stale = blog_comment_projection_delivery::Entity::find()
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(tenant_id))
            .filter(blog_comment_projection_delivery::Column::ProcessedAt.lt(cutoff))
            .order_by_asc(blog_comment_projection_delivery::Column::ProcessedAt)
            .limit(COMMENT_PROJECTION_RETENTION_BATCH)
            .all(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        if stale.is_empty() {
            return Ok(0);
        }

        let comment_ids: std::collections::BTreeSet<Uuid> =
            stale.iter().map(|row| row.comment_id).collect();
        // The frontier is computed across the comment's whole history, not only
        // the stale window: a row past the cutoff is deletable only when an even
        // newer (recent or stale) delivery exists for the same comment.
        let frontiers: Vec<(Uuid, Uuid)> = blog_comment_projection_delivery::Entity::find()
            .select_only()
            .column(blog_comment_projection_delivery::Column::CommentId)
            .column_as(
                blog_comment_projection_delivery::Column::EventId.max(),
                "max_event_id",
            )
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(tenant_id))
            .filter(
                blog_comment_projection_delivery::Column::CommentId.is_in(
                    comment_ids.into_iter().collect::<Vec<_>>(),
                ),
            )
            .group_by(blog_comment_projection_delivery::Column::CommentId)
            .into_tuple::<(Uuid, Uuid)>()
            .all(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        let frontier_by_comment: std::collections::BTreeMap<Uuid, Uuid> =
            frontiers.into_iter().collect();

        let deletable: Vec<Uuid> = stale
            .iter()
            .filter(|row| {
                frontier_by_comment
                    .get(&row.comment_id)
                    .is_some_and(|frontier| row.event_id < *frontier)
            })
            .map(|row| row.event_id)
            .collect();
        if deletable.is_empty() {
            return Ok(0);
        }

        // No transaction is needed: the delete predicate (a row that was below its
        // comment frontier) is stable under concurrent projection activity, and a
        // missed row is picked up by the next iteration.
        let deleted = blog_comment_projection_delivery::Entity::delete_many()
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(tenant_id))
            .filter(blog_comment_projection_delivery::Column::EventId.is_in(deletable))
            .exec(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        Ok(deleted.rows_affected)
    }
}

pub struct BlogCommentProjectionRetentionWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for BlogCommentProjectionRetentionWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        BlogCommentProjectionRetentionWorkAdapter::new(host.db_clone())
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for BlogCommentProjectionRetentionWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != BLOG_COMMENT_PROJECTION_RETENTION_WORKER {
            return Ok(None);
        }
        Ok(self.next_due_tenant().await?.map(|tenant_id| {
            tenant_scoped_work_item(BLOG_COMMENT_PROJECTION_RETENTION_WORKER, tenant_id)
        }))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The delivery ledger is the durable state: a purged row is no longer stale.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for BlogCommentProjectionRetentionWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        BLOG_COMMENT_PROJECTION_RETENTION_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let tenant_id = work_item_tenant(&item, BLOG_COMMENT_PROJECTION_RETENTION_WORKER)?;
        let purged = self.compact_tenant(tenant_id).await?;
        if purged > 0 {
            tracing::info!(
                tenant_id = %tenant_id,
                purged,
                "Blog comment projection retention compacted stale deliveries"
            );
        }
        Ok(ModuleWorkOutcome::Completed)
    }
}
