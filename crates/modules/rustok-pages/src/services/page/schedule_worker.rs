//! Pages-owned module work adapter for the host scheduler.
//!
//! The scheduled-publish queue in [`super::schedule`] is a durable table that only moves when
//! a worker sweeps it. This adapter publishes that queue to the generic platform scheduler, so
//! the deployed server drives it: the scheduler owns the poll loop and the deployment stop
//! signal, and Pages owns discovery and execution.
//!
//! Two properties matter and neither is an accident:
//!
//! * **Discovery covers every runnable state.** A due `scheduled` job is not the only thing a
//!   sweep has to pick up: a worker that died mid-publish leaves an `executing` job whose lease
//!   has expired, and no due job exists while it is stranded. Claiming only for due jobs would
//!   leave that publication unreachable until unrelated work appeared, so a claim is served for
//!   either condition.
//! * **Claims are paced.** [`PAGE_PUBLISH_SWEEP_INTERVAL_SECS`] spaces executions the way the
//!   standalone [`super::PagePublishScheduler`] loop did. The module-work scheduler polls every
//!   second; without this pace a transiently failing job would spend all
//!   [`super::MAX_PAGE_PUBLISH_JOB_ATTEMPTS`] attempts in a handful of seconds and become
//!   terminally `failed` instead of outlasting a short outage.
//!
//! Discovery is a read-only hint and carries no exclusive lease, so two hosts may claim the same
//! tenant concurrently. Execution is safe under duplicate runs: the claim in
//! [`PageService::sweep_due_publish_jobs_as_of`] is an optimistic `scheduled -> executing`
//! transition that also takes the reaper lease, and the replayed command carries the publish
//! idempotency key.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{
    ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
use rustok_outbox::TransactionalEventBus;
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistration, ModuleWorkScheduler};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::dto::PagePublishJobState;
use crate::entities::page_publish_job;

use super::PageService;
use super::helpers::body_revision_timestamp;
use super::schedule::{
    MAX_PAGE_PUBLISH_JOBS_PER_SWEEP, PAGE_PUBLISH_SWEEP_INTERVAL_SECS,
    PAGE_SCHEDULED_PUBLISH_WORKER,
};

/// Pages-owned bridge between the scheduled-publish queue and the generic scheduler.
#[derive(Clone)]
pub struct PageScheduledPublishWorkAdapter {
    service: PageService,
    // Fairness hint only: the durable job rows remain the source of work. Share the cursor
    // across adapter clones and serialize local claims so concurrent iterations cannot claim the
    // same tenant from the same state twice.
    last_claimed_tenant: Arc<tokio::sync::Mutex<Option<Uuid>>>,
    // Pace guard for local executions; another host keeps its own.
    last_execution: Arc<tokio::sync::Mutex<Option<Instant>>>,
    interval: Duration,
    batch: u64,
}

impl PageScheduledPublishWorkAdapter {
    pub fn new(service: PageService) -> Self {
        Self {
            service,
            last_claimed_tenant: Arc::new(tokio::sync::Mutex::new(None)),
            last_execution: Arc::new(tokio::sync::Mutex::new(None)),
            interval: Duration::from_secs(PAGE_PUBLISH_SWEEP_INTERVAL_SECS),
            batch: MAX_PAGE_PUBLISH_JOBS_PER_SWEEP,
        }
    }

    /// Overrides the sweep cadence.
    ///
    /// Deployments keep [`PAGE_PUBLISH_SWEEP_INTERVAL_SECS`]; the knob exists so a host that
    /// already paces its scheduler externally can adjust it, and so tests can run consecutive
    /// sweeps without waiting. [`Duration::ZERO`] disables pacing.
    pub fn with_sweep_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Publishes this worker through the generic scheduler. The host only composes this method;
    /// it never receives Pages tables or Pages task types.
    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    /// Rows that a sweep would change: due `scheduled` jobs and leases the reaper has to resolve.
    ///
    /// The reaper side is not optional. A worker that dies mid-publish leaves exactly one row —
    /// `executing`, lease expired — and no due job. Without it, that publication would wait for
    /// unrelated Pages work, which on a quiet tenant may be never.
    fn runnable_jobs() -> Condition {
        let now = body_revision_timestamp(Utc::now());
        Condition::any()
            .add(
                Condition::all()
                    .add(page_publish_job::Column::State.eq(PagePublishJobState::Scheduled.as_str()))
                    .add(page_publish_job::Column::PublishAt.lte(now)),
            )
            .add(
                Condition::all()
                    .add(page_publish_job::Column::State.eq(PagePublishJobState::Executing.as_str()))
                    .add(
                        Condition::any()
                            .add(page_publish_job::Column::LeaseUntil.lte(now))
                            .add(page_publish_job::Column::LeaseUntil.is_null()),
                    ),
            )
    }

    /// Rotates runnable tenants instead of re-claiming the earliest one forever. The cursor is
    /// only a fairness hint and an instance restart begins a new cycle.
    async fn next_runnable_tenant(&self) -> Result<Option<Uuid>, ModuleWorkError> {
        let mut last = self.last_claimed_tenant.lock().await;
        let runnable = || {
            page_publish_job::Entity::find()
                .filter(Self::runnable_jobs())
                .select_only()
                .column(page_publish_job::Column::TenantId)
        };

        let next = if let Some(last_tenant) = *last {
            let higher = runnable()
                .filter(page_publish_job::Column::TenantId.gt(last_tenant))
                .order_by_asc(page_publish_job::Column::TenantId)
                .into_tuple::<Uuid>()
                .one(self.service.database())
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
            match higher {
                Some(tenant_id) => Some(tenant_id),
                None => runnable()
                    .order_by_asc(page_publish_job::Column::TenantId)
                    .into_tuple::<Uuid>()
                    .one(self.service.database())
                    .await
                    .map_err(|error| ModuleWorkError::Source(error.to_string()))?,
            }
        } else {
            runnable()
                .order_by_asc(page_publish_job::Column::PublishAt)
                .order_by_asc(page_publish_job::Column::Id)
                .into_tuple::<Uuid>()
                .one(self.service.database())
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?
        };

        if let Some(tenant_id) = next {
            *last = Some(tenant_id);
        }
        Ok(next)
    }

    /// `true` while the previous execution is still inside the pace window.
    ///
    /// An idle poll does not consume the window: the pace is armed per execution, not per poll,
    /// so work that becomes due is picked up on the next poll instead of waiting out a window
    /// that an empty queue opened.
    async fn pace_pending(&self) -> bool {
        let last = self.last_execution.lock().await;
        matches!(*last, Some(at) if at.elapsed() < self.interval)
    }

    /// Arms the pace window for the execution that is about to run.
    async fn arm_pace(&self) {
        *self.last_execution.lock().await = Some(Instant::now());
    }
}

pub struct PageScheduledPublishWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for PageScheduledPublishWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let events = host.shared_get::<TransactionalEventBus>().ok_or_else(|| {
            ModuleWorkError::Handler(
                "pages scheduled publishing requires the host transactional event bus".to_string(),
            )
        })?;
        PageScheduledPublishWorkAdapter::new(PageService::new(host.db_clone(), events))
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for PageScheduledPublishWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != PAGE_SCHEDULED_PUBLISH_WORKER {
            return Ok(None);
        }
        // Pacing is per tenant-discovery, so an empty queue costs one indexed query per poll.
        if self.pace_pending().await {
            return Ok(None);
        }
        let Some(tenant_id) = self.next_runnable_tenant().await? else {
            return Ok(None);
        };
        // The cursor advanced, so this tenant is owed an execution: arm the window now rather
        // than at the end, where a panic in the host loop could leave it unarmed.
        self.arm_pace().await;
        Ok(Some(ModuleWorkItem {
            id: tenant_id,
            tenant_id,
            worker_slug: PAGE_SCHEDULED_PUBLISH_WORKER.to_string(),
            lease_token: Uuid::new_v4().to_string(),
            payload: serde_json::json!({ "tenant_id": tenant_id }),
        }))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The job rows are the durable completion state: a published or terminally failed job is
        // no longer runnable, so there is no second state to write here.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for PageScheduledPublishWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        PAGE_SCHEDULED_PUBLISH_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        let worker_slug = self.worker_slug();
        if item.worker_slug != worker_slug {
            return Err(ModuleWorkError::Handler(format!(
                "pages scheduled work item belongs to worker `{}` but `{worker_slug}` was executed",
                item.worker_slug
            )));
        }
        let value = item
            .payload
            .get("tenant_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ModuleWorkError::Handler("pages work item has no tenant_id".to_string())
            })?;
        let tenant_id = Uuid::parse_str(value).map_err(|_| {
            ModuleWorkError::Handler("pages work item tenant_id is invalid".to_string())
        })?;
        if tenant_id != item.id || tenant_id != item.tenant_id {
            return Err(ModuleWorkError::Handler(
                "pages work item identity mismatch".to_string(),
            ));
        }

        let outcome = self
            .service
            .sweep_due_publish_jobs_as_of(body_revision_timestamp(Utc::now()), self.batch)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;

        if outcome.published > 0
            || outcome.reaped.requeued > 0
            || outcome.reaped.failed > 0
            || outcome.reaped.superseded > 0
        {
            tracing::info!(
                tenant_id = %tenant_id,
                published = outcome.published,
                requeued = outcome.reaped.requeued,
                failed = outcome.reaped.failed,
                superseded = outcome.reaped.superseded,
                "Pages scheduled publishing swept the publish queue"
            );
        }
        // Whether the page lock of one job rejected the command is recorded on that job row; the
        // sweep itself completed, so a retry of the same envelope has nothing new to do.
        Ok(ModuleWorkOutcome::Completed)
    }
}
