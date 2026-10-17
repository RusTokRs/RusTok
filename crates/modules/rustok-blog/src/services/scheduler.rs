//! Blog-owned module work adapters for the host scheduler.
//!
//! Blog publishes three durable queues to the generic platform scheduler instead of
//! running module-local timers. The scheduler owns the poll loop and the deployment
//! stop signal; each adapter here owns discovery (a read-only hint) and hands the
//! authoritative work to an owner command:
//!
//! - `blog_scheduled_publish` drains drafts whose `scheduled_at` has passed through
//!   [`crate::PostService::publish_scheduled_posts`], where the post version CAS
//!   decides which concurrent writer publishes a post;
//! - `blog_preview_token_cleanup` removes expired preview tokens of one tenant;
//! - `blog_route_reconciliation` removes Blog routes whose target post no longer
//!   exists, which is the safety net for a missed `TargetDeleted` delivery.
//!
//! Every queue is tenant-scoped, like every other platform work item: one tenant's
//! backlog cannot starve another tenant.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
use rustok_outbox::TransactionalEventBus;
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistration, ModuleWorkScheduler};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use uuid::Uuid;

use crate::entities::{blog_post, blog_post_route, blog_preview_token};
use crate::services::preview_token::{MAX_PREVIEW_TOKEN_CLEANUP_BATCH, PreviewTokenService};
use crate::services::{
    BLOG_POST_TARGET_TYPE, BlogPostRouteOwner, MAX_SCHEDULED_PUBLISH_BATCH, PostService,
    status_to_storage,
};
use crate::state_machine::BlogPostStatus;

/// Stable scheduler identity for Blog scheduled publishing.
pub const BLOG_SCHEDULED_PUBLISH_WORKER: &str = "blog_scheduled_publish";
/// Stable scheduler identity for Blog preview-token retention.
pub const BLOG_PREVIEW_TOKEN_CLEANUP_WORKER: &str = "blog_preview_token_cleanup";
/// Stable scheduler identity for stale Blog route reconciliation.
pub const BLOG_ROUTE_RECONCILIATION_WORKER: &str = "blog_route_reconciliation";
/// Upper bound of routes one reconciliation iteration removes.
const ROUTE_RECONCILIATION_BATCH: u64 = 200;
/// Upper bound of stale route rows one reconciliation iteration scans.
///
/// The scan bound is separate from the removal bound: one scanned row points at
/// a target that can own more routes than the removal budget allows.
const ROUTE_RECONCILIATION_SCAN: u64 = 200;

/// Builds the tenant-scoped envelope the scheduler claims and hands back.
fn tenant_scoped_work_item(worker_slug: &str, tenant_id: Uuid) -> ModuleWorkItem {
    ModuleWorkItem {
        id: tenant_id,
        tenant_id,
        worker_slug: worker_slug.to_string(),
        // The envelope lease only orders delivery. The authoritative claim is the
        // owner-side predicate (post version CAS, expiry predicate, or existence
        // check), which every handler repeats before it writes.
        lease_token: Uuid::new_v4().to_string(),
        payload: serde_json::json!({ "tenant_id": tenant_id }),
    }
}

/// Reads and validates the tenant carried by an envelope of `expected_slug`.
fn work_item_tenant(item: &ModuleWorkItem, expected_slug: &str) -> Result<Uuid, ModuleWorkError> {
    if item.worker_slug != expected_slug {
        return Err(ModuleWorkError::Handler(format!(
            "blog scheduled work item belongs to worker `{}` but `{expected_slug}` was executed",
            item.worker_slug
        )));
    }
    let value = item
        .payload
        .get("tenant_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ModuleWorkError::Handler("blog work item has no tenant_id".to_string()))?;
    let tenant_id = Uuid::parse_str(value)
        .map_err(|_| ModuleWorkError::Handler("blog work item tenant_id is invalid".to_string()))?;
    if tenant_id != item.id || tenant_id != item.tenant_id {
        return Err(ModuleWorkError::Handler(
            "blog work item identity mismatch".to_string(),
        ));
    }
    Ok(tenant_id)
}

/// Blog-owned bridge between scheduled drafts and the generic scheduler.
///
/// One claim serves one tenant: the tenant owning the earliest due schedule is
/// drained up to [`MAX_SCHEDULED_PUBLISH_BATCH`] posts. Published posts leave the due
/// set, so each iteration advances every tenant's earliest schedule in turn.
#[derive(Clone)]
pub struct BlogScheduledPublishWorkAdapter {
    database: DatabaseConnection,
    events: TransactionalEventBus,
}

impl BlogScheduledPublishWorkAdapter {
    pub fn new(database: DatabaseConnection, events: TransactionalEventBus) -> Self {
        Self { database, events }
    }

    /// Publishes this worker through the generic scheduler. The host only composes
    /// this method; it never receives Blog tables or Blog task types.
    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    /// Tenant with the earliest due schedule. Read-only: the CAS in the owner command
    /// re-checks the due predicate before publishing anything.
    async fn next_due_tenant(&self) -> Result<Option<Uuid>, ModuleWorkError> {
        let now = chrono::Utc::now().fixed_offset();
        blog_post::Entity::find()
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Draft)))
            .filter(blog_post::Column::ScheduledAt.is_not_null())
            .filter(blog_post::Column::ScheduledAt.lte(now))
            .order_by_asc(blog_post::Column::ScheduledAt)
            .order_by_asc(blog_post::Column::Id)
            .one(&self.database)
            .await
            .map(|post| post.map(|post| post.tenant_id))
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }
}

pub struct BlogScheduledPublishWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for BlogScheduledPublishWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let events = host.shared_get::<TransactionalEventBus>().ok_or_else(|| {
            ModuleWorkError::Handler(
                "blog scheduled publishing requires the host transactional event bus".to_string(),
            )
        })?;
        BlogScheduledPublishWorkAdapter::new(host.db_clone(), events)
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for BlogScheduledPublishWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != BLOG_SCHEDULED_PUBLISH_WORKER {
            return Ok(None);
        }
        Ok(self
            .next_due_tenant()
            .await?
            .map(|tenant_id| tenant_scoped_work_item(BLOG_SCHEDULED_PUBLISH_WORKER, tenant_id)))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The published post row is the durable completion state: a published post is
        // no longer a due draft, so there is no second state to write here.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for BlogScheduledPublishWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        BLOG_SCHEDULED_PUBLISH_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let tenant_id = work_item_tenant(&item, BLOG_SCHEDULED_PUBLISH_WORKER)?;
        let service = PostService::new(self.database.clone(), self.events.clone());
        let published = service
            .publish_scheduled_posts(tenant_id, MAX_SCHEDULED_PUBLISH_BATCH)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;

        if !published.is_empty() {
            tracing::info!(
                tenant_id = %tenant_id,
                published = published.len(),
                "Blog scheduled publishing published due drafts"
            );
        }
        Ok(ModuleWorkOutcome::Completed)
    }
}

/// Blog-owned retention worker for `blog_preview_tokens`.
///
/// Expired tokens have no consumer, so they are pure storage growth. Deleting a post
/// already removes its tokens through the foreign key cascade; this worker bounds the
/// purely time-based residue.
#[derive(Clone)]
pub struct BlogPreviewTokenCleanupWorkAdapter {
    database: DatabaseConnection,
    events: TransactionalEventBus,
}

impl BlogPreviewTokenCleanupWorkAdapter {
    pub fn new(database: DatabaseConnection, events: TransactionalEventBus) -> Self {
        Self { database, events }
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    /// Tenant owning the earliest expired token. Read-only discovery; the delete
    /// itself re-checks the expiry predicate.
    async fn next_due_tenant(&self) -> Result<Option<Uuid>, ModuleWorkError> {
        let now = chrono::Utc::now().fixed_offset();
        blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::ExpiresAt.lt(now))
            .order_by_asc(blog_preview_token::Column::ExpiresAt)
            .one(&self.database)
            .await
            .map(|token| token.map(|token| token.tenant_id))
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }
}

pub struct BlogPreviewTokenCleanupWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for BlogPreviewTokenCleanupWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let events = host.shared_get::<TransactionalEventBus>().ok_or_else(|| {
            ModuleWorkError::Handler(
                "blog preview token cleanup requires the host transactional event bus".to_string(),
            )
        })?;
        BlogPreviewTokenCleanupWorkAdapter::new(host.db_clone(), events)
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for BlogPreviewTokenCleanupWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != BLOG_PREVIEW_TOKEN_CLEANUP_WORKER {
            return Ok(None);
        }
        Ok(self
            .next_due_tenant()
            .await?
            .map(|tenant_id| tenant_scoped_work_item(BLOG_PREVIEW_TOKEN_CLEANUP_WORKER, tenant_id)))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The token rows are the durable state: a deleted token is no longer
        // expired-but-present, so there is no second state to record here.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for BlogPreviewTokenCleanupWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        BLOG_PREVIEW_TOKEN_CLEANUP_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let tenant_id = work_item_tenant(&item, BLOG_PREVIEW_TOKEN_CLEANUP_WORKER)?;
        let service = PreviewTokenService::new(self.database.clone(), self.events.clone());
        let deleted = service
            .cleanup_expired(tenant_id, MAX_PREVIEW_TOKEN_CLEANUP_BATCH)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;

        if deleted > 0 {
            tracing::info!(
                tenant_id = %tenant_id,
                deleted,
                "Blog preview token cleanup removed expired tokens"
            );
        }
        Ok(ModuleWorkOutcome::Completed)
    }
}

/// Blog-owned reconciliation of routes whose target post no longer exists.
///
/// Such a route is normally removed by the delete transaction, and a deletion that
/// happened elsewhere is reported through `TargetDeleted`. Reconciliation is the
/// safety net for a missed delivery: it finds routes whose target post is absent,
/// removes them, and reports the purged routes through `UrlAliasPurged`, so a stale
/// redirect cannot survive indefinitely.
#[derive(Clone)]
pub struct BlogRouteReconciliationWorkAdapter {
    database: DatabaseConnection,
    events: TransactionalEventBus,
}

impl BlogRouteReconciliationWorkAdapter {
    pub fn new(database: DatabaseConnection, events: TransactionalEventBus) -> Self {
        Self { database, events }
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    /// Tenant owning the first stale route.
    async fn next_due_tenant(&self) -> Result<Option<Uuid>, ModuleWorkError> {
        blog_post_route::Entity::find()
            .filter(dangling_route_condition())
            .order_by_asc(blog_post_route::Column::TenantId)
            .order_by_asc(blog_post_route::Column::SourceRoute)
            .one(&self.database)
            .await
            .map(|route| route.map(|route| route.tenant_id))
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }

    async fn reconcile_tenant(&self, tenant_id: Uuid) -> Result<u64, ModuleWorkError> {
        let stale = blog_post_route::Entity::find()
            .filter(blog_post_route::Column::TenantId.eq(tenant_id))
            .filter(dangling_route_condition())
            .order_by_asc(blog_post_route::Column::SourceRoute)
            .limit(ROUTE_RECONCILIATION_SCAN)
            .all(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        if stale.is_empty() {
            return Ok(0);
        }

        let txn = self
            .database
            .begin()
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        let owner = BlogPostRouteOwner::new(self.events.clone());
        let mut removed = 0u64;
        for route in stale {
            // The iteration removes at most ROUTE_RECONCILIATION_BATCH routes, no
            // matter how many routes one target owns. The remaining routes of a
            // target are removed by the next iteration.
            if removed >= ROUTE_RECONCILIATION_BATCH {
                break;
            }
            let budget = ROUTE_RECONCILIATION_BATCH - removed;
            removed += owner
                .remove_target_routes_up_to_in_tx(&txn, tenant_id, None, route.target_id, budget)
                .await
                .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        }
        txn.commit()
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        Ok(removed)
    }
}

pub struct BlogRouteReconciliationWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for BlogRouteReconciliationWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let events = host.shared_get::<TransactionalEventBus>().ok_or_else(|| {
            ModuleWorkError::Handler(
                "blog route reconciliation requires the host transactional event bus".to_string(),
            )
        })?;
        BlogRouteReconciliationWorkAdapter::new(host.db_clone(), events)
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for BlogRouteReconciliationWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != BLOG_ROUTE_RECONCILIATION_WORKER {
            return Ok(None);
        }
        Ok(self
            .next_due_tenant()
            .await?
            .map(|tenant_id| tenant_scoped_work_item(BLOG_ROUTE_RECONCILIATION_WORKER, tenant_id)))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The route rows are the durable state: a purged route is no longer stale.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for BlogRouteReconciliationWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        BLOG_ROUTE_RECONCILIATION_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let tenant_id = work_item_tenant(&item, BLOG_ROUTE_RECONCILIATION_WORKER)?;
        let removed = self.reconcile_tenant(tenant_id).await?;
        if removed > 0 {
            tracing::info!(
                tenant_id = %tenant_id,
                removed,
                "Blog route reconciliation removed stale routes"
            );
        }
        Ok(ModuleWorkOutcome::Completed)
    }
}

/// A route of a `blog_post` target whose post is absent in the same tenant.
///
/// The correlated subquery mirrors what the owner checks on read, so discovery and
/// the authoritative predicate agree.
fn dangling_route_condition() -> Condition {
    // Correlated subquery: the posts of the route's own tenant.
    let tenant_post_ids = blog_post::Entity::find()
        .select_only()
        .column(blog_post::Column::Id)
        .filter(blog_post::Column::Id.eq(Expr::col((
            blog_post_route::Entity,
            blog_post_route::Column::TargetId,
        ))))
        .filter(blog_post::Column::TenantId.eq(Expr::col((
            blog_post_route::Entity,
            blog_post_route::Column::TenantId,
        ))))
        .into_query();

    Condition::all()
        .add(blog_post_route::Column::TargetKind.eq(BLOG_POST_TARGET_TYPE))
        .add(
            Expr::col((blog_post_route::Entity, blog_post_route::Column::TargetId))
                .not_in_subquery(tenant_post_ids),
        )
}

/// The reconciliation worker is `runtime`-gated with the rest of this module, so these
/// tests run under `cargo test -p rustok-blog --features runtime`.
#[cfg(test)]
mod reconciliation_tests {
    use super::*;
    use crate::services::canonical_post_route;
    use crate::tests::hardening::{
        create_draft_post, insert_route, outbox_event_count, setup_hardening_schema,
    };

    fn adapter(
        db: &DatabaseConnection,
        events: &TransactionalEventBus,
    ) -> BlogRouteReconciliationWorkAdapter {
        BlogRouteReconciliationWorkAdapter::new(db.clone(), events.clone())
    }

    #[tokio::test]
    async fn reconciliation_removes_stale_routes_and_then_stops_claiming() {
        let (db, events) = setup_hardening_schema().await;
        let tenant_id = Uuid::new_v4();
        let ghost_target = Uuid::new_v4();
        insert_route(
            &db,
            tenant_id,
            "ghost-reconcile",
            BLOG_POST_TARGET_TYPE,
            ghost_target,
        )
        .await;

        let adapter = adapter(&db, &events);
        let item = adapter
            .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
            .await
            .expect("claim should not fail")
            .expect("a stale route must be claimable");
        assert_eq!(item.tenant_id, tenant_id);

        let outcome = adapter
            .execute(item)
            .await
            .expect("reconciliation should run");
        assert!(matches!(outcome, ModuleWorkOutcome::Completed));
        assert!(
            BlogPostRouteOwner::find_redirect(
                &db,
                tenant_id,
                &canonical_post_route("ghost-reconcile")
            )
            .await
            .expect("route read")
            .is_none(),
            "reconciliation must remove the stale route"
        );
        assert_eq!(
            outbox_event_count(&db, "content.url_alias.purged").await,
            1,
            "reconciliation must report the purged route"
        );

        assert!(
            adapter
                .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
                .await
                .expect("claim should not fail")
                .is_none(),
            "an empty stale set is not a work item"
        );
        assert_eq!(
            outbox_event_count(&db, "content.url_alias.purged").await,
            1,
            "a second run must not publish again"
        );

        // A route whose target still exists belongs to no one: it must be kept.
        let post_id = create_draft_post(&db, &events, tenant_id, "live-reconcile").await;
        insert_route(&db, tenant_id, "live-route", BLOG_POST_TARGET_TYPE, post_id).await;
        assert!(
            adapter
                .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
                .await
                .expect("claim should not fail")
                .is_none()
        );
        assert!(
            BlogPostRouteOwner::find_redirect(&db, tenant_id, &canonical_post_route("live-route"))
                .await
                .expect("route read")
                .is_some(),
            "a live target keeps its route"
        );
    }

    #[tokio::test]
    async fn reconciliation_claims_only_its_own_worker_slug() {
        let (db, events) = setup_hardening_schema().await;
        let adapter = adapter(&db, &events);

        assert!(
            adapter
                .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
                .await
                .expect("claim should not fail")
                .is_none(),
            "another Blog worker's slug is not this worker's work"
        );
        assert!(
            adapter
                .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
                .await
                .expect("claim should not fail")
                .is_none(),
            "a schema without stale routes claims nothing"
        );
    }
}

/// The scheduled-publish worker is `runtime`-gated with the rest of this module, so these
/// tests run under `cargo test -p rustok-blog --features runtime`.
#[cfg(test)]
mod scheduled_publish_worker_tests {
    use super::*;
    use crate::services::status_to_storage;
    use crate::tests::hardening::{
        create_draft_post, make_post_due, set_post_schedule, setup_hardening_schema,
    };

    fn adapter(
        db: &DatabaseConnection,
        events: &TransactionalEventBus,
    ) -> BlogScheduledPublishWorkAdapter {
        BlogScheduledPublishWorkAdapter::new(db.clone(), events.clone())
    }

    /// The stored status of a post row, read from the owner table.
    async fn post_status(db: &DatabaseConnection, post_id: Uuid) -> String {
        blog_post::Entity::find_by_id(post_id)
            .one(db)
            .await
            .expect("post read should not fail")
            .expect("post should exist")
            .status
    }

    /// The claim is a read-only scan of due drafts, so the worker must hand the host the
    /// tenant whose schedule is oldest and never a tenant without due work.
    #[tokio::test]
    async fn claim_selects_the_tenant_of_the_earliest_due_draft_and_drains_it() {
        let (db, events) = setup_hardening_schema().await;
        let earlier_tenant = Uuid::new_v4();
        let later_tenant = Uuid::new_v4();

        let later = create_draft_post(&db, &events, later_tenant, "due-later").await;
        make_post_due(&db, later, 1).await;
        let earlier = create_draft_post(&db, &events, earlier_tenant, "due-earlier").await;
        make_post_due(&db, earlier, 3).await;

        let adapter = adapter(&db, &events);
        let first = adapter
            .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
            .await
            .expect("claim should not fail")
            .expect("a due draft must be claimable");
        assert_eq!(
            first.tenant_id, earlier_tenant,
            "the tenant with the earliest due schedule is claimed first"
        );
        assert!(
            matches!(
                adapter
                    .execute(first)
                    .await
                    .expect("the scheduled publish should run"),
                ModuleWorkOutcome::Completed
            ),
            "the worker reports a completed item"
        );
        assert_eq!(
            post_status(&db, earlier).await,
            status_to_storage(BlogPostStatus::Published),
            "the claimed tenant's due draft is published"
        );
        assert_eq!(
            post_status(&db, later).await,
            status_to_storage(BlogPostStatus::Draft),
            "another tenant's due draft is untouched"
        );

        let second = adapter
            .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
            .await
            .expect("claim should not fail")
            .expect("the remaining due tenant must be claimable");
        assert_eq!(
            second.tenant_id, later_tenant,
            "the drained tenant is not claimed again"
        );
        adapter
            .execute(second)
            .await
            .expect("the second scheduled publish should run");
        assert_eq!(
            post_status(&db, later).await,
            status_to_storage(BlogPostStatus::Published),
            "the second tenant's due draft is published by its own claim"
        );

        assert!(
            adapter
                .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
                .await
                .expect("claim should not fail")
                .is_none(),
            "a drained queue claims nothing"
        );
    }

    /// A schedule in the future is not due work, and another Blog worker's slug is not this
    /// adapter's source.
    #[tokio::test]
    async fn claim_ignores_future_schedules_and_foreign_worker_slugs() {
        let (db, events) = setup_hardening_schema().await;
        let tenant_id = Uuid::new_v4();
        let post_id = create_draft_post(&db, &events, tenant_id, "future-only").await;
        set_post_schedule(
            &db,
            post_id,
            Some(chrono::Utc::now() + chrono::Duration::hours(2)),
        )
        .await;

        let adapter = adapter(&db, &events);
        assert!(
            adapter
                .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
                .await
                .expect("claim should not fail")
                .is_none(),
            "a schedule in the future is not due work"
        );
        assert!(
            adapter
                .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
                .await
                .expect("claim should not fail")
                .is_none(),
            "another Blog worker's slug is not this worker's work"
        );

        make_post_due(&db, post_id, 1).await;
        let item = adapter
            .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
            .await
            .expect("claim should not fail")
            .expect("a due draft becomes claimable once its schedule passes");
        assert_eq!(item.tenant_id, tenant_id);
    }
}
