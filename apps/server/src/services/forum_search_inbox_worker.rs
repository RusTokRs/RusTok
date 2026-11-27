#[path = "forum_search_contract_consumer.rs"]
mod contract_consumer;

pub use contract_consumer::{
    ForumSearchContractConsumerWorkerHandle, forum_search_contract_consumer_enabled,
    start_forum_search_contract_consumer_if_enabled,
};

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_core::ModuleRuntimeExtensions;
use rustok_search::{
    DEFAULT_FORUM_SWEEP_EVENT_LIMIT, DEFAULT_FORUM_SWEEP_TENANT_LIMIT, ForumProjectionReconciler,
    SharedForumProjectionOwnerRevisionSourcePort,
    search_projection_source_registry_from_extensions,
};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::DirectWorkerRegistrationLock;
use crate::services::server_runtime_context::published_worker_running;
use crate::services::server_runtime_context::{
    ServerRuntimeContext, ServerRuntimeStartReservation,
};
use crate::services::stop_handle::{StopHandle, may_admit_next_worker_operation};

const FORUM_SEARCH_INBOX_POLL_INTERVAL: Duration = Duration::from_secs(5);
static FORUM_SEARCH_INBOX_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

// Prevent two callers from publishing competing inbox sweeper tasks.
struct ForumSearchInboxStartupReservation;

pub struct ForumSearchInboxWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl ForumSearchInboxWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    /// Await a durable worker's current operation after the shutdown signal.
    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }

    pub(crate) fn into_task(self) -> JoinHandle<()> {
        self._handle
    }
}

pub fn start_forum_search_inbox_worker_if_ready(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers() {
        return Ok(());
    }
    if published_worker_running::<ForumSearchInboxWorkerHandle>(
        ctx,
        "Forum Search inbox worker",
        ForumSearchInboxWorkerHandle::is_finished,
    )? {
        return Ok(());
    }

    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| Error::Message("module runtime extensions are unavailable".to_string()))?;
    let source_registry = search_projection_source_registry_from_extensions(extensions.as_ref())
        .ok_or_else(|| {
            Error::Message(
                "Search projection source registry is unavailable for Forum inbox worker"
                    .to_string(),
            )
        })?;
    let Some(forum_source) =
        source_registry.build_with_extensions("forum", ctx.db_clone(), extensions.as_ref())
    else {
        tracing::warn!(
            "Forum Search inbox worker not started: Forum projection source is unavailable"
        );
        return Ok(());
    };
    let owner_source = extensions
        .get::<SharedForumProjectionOwnerRevisionSourcePort>()
        .cloned()
        .ok_or_else(|| {
            Error::Message(
                "Forum Search inbox worker requires the Forum owner revision source".to_string(),
            )
        })?;

    // The Blog author projection is a tenant-wide inbox scope, so the sweeper
    // rebuilds it through the same neutral driver the event handler uses. A host
    // without the Blog module registers no source and keeps the optional driver
    // empty instead of failing the worker.
    let blog_source =
        source_registry.build_with_extensions("blog", ctx.db_clone(), extensions.as_ref());
    let reconciler = ForumProjectionReconciler::with_owner_revision_source(
        ctx.db_clone(),
        forum_source,
        owner_source,
    )
    .with_blog_source(blog_source);
    if !reconciler.supports_background_reconciliation() {
        tracing::info!("Forum Search inbox worker not started: PostgreSQL backend is required");
        return Ok(());
    }

    let stop = StopHandle::ensure(ctx);
    if stop.is_stopping() {
        return Err(Error::Message(
            "Forum Search inbox worker cannot start during server shutdown".to_string(),
        ));
    }
    let Some(reservation) =
        ServerRuntimeStartReservation::try_claim(ctx, ForumSearchInboxStartupReservation)
    else {
        // Another caller is still preparing this worker; a successful return
        // would falsely tell bootstrap it is running if that starter fails.
        if published_worker_running::<ForumSearchInboxWorkerHandle>(
            ctx,
            "Forum Search inbox worker",
            ForumSearchInboxWorkerHandle::is_finished,
        )? {
            return Ok(());
        }
        return Err(Error::Message(
            "Forum Search inbox worker startup is already in progress; retry".to_string(),
        ));
    };
    if published_worker_running::<ForumSearchInboxWorkerHandle>(
        ctx,
        "Forum Search inbox worker",
        ForumSearchInboxWorkerHandle::is_finished,
    )? {
        return Ok(());
    }
    let stop_rx = stop.subscribe();

    let instance_id = FORUM_SEARCH_INBOX_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        instance_id,
        tenant_limit = DEFAULT_FORUM_SWEEP_TENANT_LIMIT,
        event_limit = DEFAULT_FORUM_SWEEP_EVENT_LIMIT,
        poll_interval_seconds = FORUM_SEARCH_INBOX_POLL_INTERVAL.as_secs(),
        "Starting Forum Search inbox and owner checkpoint worker"
    );
    // The startup reservation excludes competing initializers; this short
    // registration gate excludes shutdown taking the handle between the final
    // stop check and publication. No async setup is performed under the gate.
    let registration = DirectWorkerRegistrationLock::ensure(ctx);
    let _registration_guard = registration
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop.is_stopping() {
        return Err(Error::Message(
            "Forum Search inbox startup interrupted by server shutdown".to_string(),
        ));
    }
    ctx.shared_insert(ForumSearchInboxWorkerHandle {
        instance_id,
        _handle: tokio::spawn(forum_search_inbox_worker_loop(reconciler, stop_rx)),
    });
    reservation.commit();
    Ok(())
}

async fn forum_search_inbox_worker_loop(
    reconciler: ForumProjectionReconciler,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if !may_admit_next_worker_operation(&stop_rx) {
            tracing::info!("Forum Search inbox worker stopped");
            return;
        }

        match reconciler
            .sweep_due(
                DEFAULT_FORUM_SWEEP_TENANT_LIMIT,
                DEFAULT_FORUM_SWEEP_EVENT_LIMIT,
            )
            .await
        {
            Ok(report)
                if report.due_tenants > 0
                    || report.owner_tenants_scanned > 0
                    || report.recovered_processing_events > 0 =>
            {
                tracing::debug!(
                    due_tenants = report.due_tenants,
                    claimed_events = report.claimed_events,
                    completed_events = report.completed_events,
                    failed_events = report.failed_events,
                    recovered_processing_events = report.recovered_processing_events,
                    owner_tenants_scanned = report.owner_tenants_scanned,
                    owner_tenants_reconciled = report.owner_tenants_reconciled,
                    owner_tenants_blocked = report.owner_tenants_blocked,
                    owner_tenants_failed = report.owner_tenants_failed,
                    owner_revisions_checkpointed = report.owner_revisions_checkpointed,
                    owner_rebuilds = report.owner_rebuilds,
                    "Forum Search inbox and owner checkpoint sweep completed"
                )
            }
            Ok(_) => {}
            Err(_error) => tracing::warn!(
                error_code = "forum_search_inbox_sweep_failed",
                "Forum Search inbox sweep failed during owner checkpoint cycle"
            ),
        }

        tokio::select! {
            _ = tokio::time::sleep(FORUM_SEARCH_INBOX_POLL_INTERVAL) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Forum Search inbox worker received shutdown signal");
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::RustokSettings;

    #[tokio::test]
    async fn previously_exited_worker_cannot_pass_bootstrap_as_running() {
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        assert!(ctx.settings().runtime.runs_background_workers());

        let handle = tokio::spawn(async {});
        tokio::time::timeout(Duration::from_secs(2), async {
            while !handle.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake worker exits");

        ctx.shared_insert(ForumSearchInboxWorkerHandle {
            instance_id: 1,
            _handle: handle,
        });
        let error = start_forum_search_inbox_worker_if_ready(&ctx)
            .expect_err("exited Forum worker must not be treated as running");
        assert!(format!("{error}").contains("Forum Search inbox worker has terminated"));
        assert!(ctx.shared_contains::<ForumSearchInboxWorkerHandle>());
    }
}
