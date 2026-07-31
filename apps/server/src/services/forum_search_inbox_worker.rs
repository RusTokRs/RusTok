use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_core::ModuleRuntimeExtensions;
use rustok_search::{
    DEFAULT_FORUM_SWEEP_EVENT_LIMIT, DEFAULT_FORUM_SWEEP_TENANT_LIMIT,
    ForumProjectionReconciler, SharedForumProjectionOwnerRevisionSourcePort,
    search_projection_source_registry_from_extensions,
};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

const FORUM_SEARCH_INBOX_POLL_INTERVAL: Duration = Duration::from_secs(5);
static FORUM_SEARCH_INBOX_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

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
}

pub fn start_forum_search_inbox_worker_if_ready(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<ForumSearchInboxWorkerHandle>()
    {
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
    let Some(forum_source) = source_registry.build("forum", ctx.db_clone()) else {
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

    let reconciler = ForumProjectionReconciler::with_owner_revision_source(
        ctx.db_clone(),
        forum_source,
        owner_source,
    );
    if !reconciler.supports_background_reconciliation() {
        tracing::info!(
            "Forum Search inbox worker not started: PostgreSQL backend is required"
        );
        return Ok(());
    }

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before Forum Search inbox worker startup")
        .subscribe();

    let instance_id = FORUM_SEARCH_INBOX_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        instance_id,
        tenant_limit = DEFAULT_FORUM_SWEEP_TENANT_LIMIT,
        event_limit = DEFAULT_FORUM_SWEEP_EVENT_LIMIT,
        poll_interval_seconds = FORUM_SEARCH_INBOX_POLL_INTERVAL.as_secs(),
        "Starting Forum Search inbox and owner checkpoint worker"
    );
    ctx.shared_insert(ForumSearchInboxWorkerHandle {
        instance_id,
        _handle: tokio::spawn(forum_search_inbox_worker_loop(reconciler, stop_rx)),
    });
    Ok(())
}

async fn forum_search_inbox_worker_loop(
    reconciler: ForumProjectionReconciler,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
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
            Err(error) => tracing::warn!(
                error = %error,
                "Forum Search inbox and owner checkpoint sweep failed"
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
