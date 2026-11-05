use crate::error::{Error, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;

#[cfg(feature = "mod-seo")]
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
#[cfg(feature = "mod-seo")]
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::event_transport_factory::{
    EventRuntime, OutboxRetentionRuntimeConfig, RelayRuntimeConfig, spawn_outbox_relay_worker,
};
use crate::services::outbox_retention_worker::spawn_outbox_retention_worker;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_modules::ModuleControlPlane;
#[cfg(feature = "mod-seo")]
use rustok_seo::{SeoApplicationServices, SeoWorkerAuthorization};

// ── Graceful-shutdown handle ─────────────────────────────────────────────────

/// Stored in `ServerRuntimeContext`; `on_shutdown` calls `stop()` to abort workers.
#[derive(Clone)]
pub struct StopHandle {
    stop_tx: tokio::sync::watch::Sender<bool>,
}

impl StopHandle {
    pub fn new() -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (Self { stop_tx: tx }, rx)
    }

    /// Create a new `Receiver` subscribed to the shutdown signal.
    ///
    /// The returned receiver immediately sees the current value and will be
    /// notified when [`StopHandle::stop`] is called. Clone it once per background worker
    /// so each worker gets its own independent view of the channel.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.stop_tx.subscribe()
    }

    pub async fn stop(&self) {
        let was_stopping = self.stop_tx.send_replace(true);
        if !was_stopping {
            // Yield so spawned tasks have a chance to notice the signal.
            tokio::task::yield_now().await;
        }
    }

    pub fn is_stopping(&self) -> bool {
        *self.stop_tx.borrow()
    }

    /// Safely retrieves or initializes the shared `StopHandle` from the context without panic.
    pub fn ensure(ctx: &ServerRuntimeContext) -> Self {
        if let Some(existing) = ctx.shared_get::<Self>() {
            existing
        } else {
            let (handle, _rx) = Self::new();
            let _ = ctx.shared_insert_if_absent(handle.clone());
            ctx.shared_get::<Self>().unwrap_or(handle)
        }
    }
}

static OUTBOX_RELAY_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static OUTBOX_RETENTION_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static AUTH_RETENTION_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static REMOTE_EXECUTOR_REAPER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "mod-seo")]
static SEO_BULK_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static MODULE_WORK_SCHEDULER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "mod-workflow")]
static WORKFLOW_CRON_SCHEDULER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

const LOCAL_SQLITE_DATABASE_URI: &str = "sqlite://rustok.sqlite?mode=rwc";
#[cfg(feature = "mod-seo")]
const SEO_BULK_WORKER_POLL_INTERVAL_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorkerLifecycleState {
    Starting,
    Ready,
    Degraded,
    Stopping,
    Failed,
}

impl RuntimeWorkerLifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
        }
    }

    pub fn metric_value(self) -> i8 {
        match self {
            Self::Starting => 1,
            Self::Ready => 2,
            Self::Degraded => 3,
            Self::Stopping => 4,
            Self::Failed => 5,
        }
    }

    pub fn from_worker_snapshot(
        required: bool,
        handle_finished: Option<bool>,
        stop_requested: bool,
    ) -> Self {
        if stop_requested {
            return Self::Stopping;
        }

        match (required, handle_finished) {
            (true, None) => Self::Starting,
            (true, Some(false)) => Self::Ready,
            (true, Some(true)) => Self::Failed,
            (false, Some(true)) => Self::Degraded,
            (false, _) => Self::Ready,
        }
    }
}

pub struct OutboxRelayWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl OutboxRelayWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

pub struct OutboxRetentionWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl OutboxRetentionWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

pub struct AuthRetentionWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl AuthRetentionWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

pub struct RemoteExecutorReaperHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl RemoteExecutorReaperHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

#[cfg(feature = "mod-seo")]
pub struct SeoBulkWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

#[cfg(feature = "mod-seo")]
impl SeoBulkWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

/// Whether module-work composition registered any scheduled work. A missing
/// snapshot means composition has not yet established whether a task is required.
#[derive(Clone, Copy)]
pub(crate) struct ModuleWorkSchedulerRequirement(pub bool);

// Serializes module-work initialization with shutdown's handle drain. This is
// stored per runtime context rather than globally, so independent hosts remain
// independent and a cancelled initializer releases the reservation.
#[derive(Clone, Default)]
pub(crate) struct ModuleWorkSchedulerInitializationLock(
    pub(crate) std::sync::Arc<tokio::sync::Mutex<()>>,
);

impl ModuleWorkSchedulerInitializationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }
}

pub struct ModuleWorkSchedulerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl ModuleWorkSchedulerHandle {
    pub fn new(handle: JoinHandle<()>) -> Self {
        Self {
            instance_id: MODULE_WORK_SCHEDULER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed),
            _handle: handle,
        }
    }

    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

#[cfg(feature = "mod-workflow")]
pub struct WorkflowCronSchedulerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

#[cfg(feature = "mod-workflow")]
impl WorkflowCronSchedulerHandle {
    pub fn new(handle: JoinHandle<()>) -> Self {
        Self {
            instance_id: WORKFLOW_CRON_SCHEDULER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed),
            _handle: handle,
        }
    }

    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

/// One host-owned worker observation, shared by readiness and Prometheus.
/// It reports task liveness only, not backlog drainage or recent progress.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeWorkerSnapshot {
    pub name: &'static str,
    pub required: bool,
    pub finished: Option<bool>,
}

pub(crate) fn additional_runtime_worker_snapshots(
    ctx: &ServerRuntimeContext,
) -> Vec<RuntimeWorkerSnapshot> {
    #[cfg(feature = "commerce-marketplace-financial")]
    use crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle;
    use crate::services::module_artifact_maintenance::ModuleArtifactMaintenanceHandle;
    use crate::services::module_event_dispatcher::{
        ModuleEventDispatcherHandle, ModuleEventDispatcherRequirement,
    };
    use crate::services::module_transition_watchdog::ModuleTransitionWatchdogHandle;
    #[cfg(feature = "mod-commerce")]
    use crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle;
    #[cfg(feature = "mod-payment")]
    use crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle;
    #[cfg(feature = "mod-product")]
    use crate::services::product_media_release_worker::ProductMediaReleaseWorkerHandle;

    let local = ctx.settings().runtime.runs_background_workers();
    let listeners_required = ctx
        .shared_get::<ModuleEventDispatcherRequirement>()
        .map(|requirement| requirement.0)
        // Until composition has evaluated its listener set, fail closed.
        .unwrap_or(true);
    let module_work_required = ctx
        .shared_get::<ModuleWorkSchedulerRequirement>()
        .map(|requirement| requirement.0)
        // Before work registrations have been inspected, fail closed.
        .unwrap_or(true);
    vec![
        RuntimeWorkerSnapshot {
            name: "module_work_scheduler",
            required: local && module_work_required,
            finished: ctx
                .shared_map::<ModuleWorkSchedulerHandle, _>(ModuleWorkSchedulerHandle::is_finished),
        },
        RuntimeWorkerSnapshot {
            name: "auth_retention",
            required: local
                && ctx
                    .settings()
                    .runtime
                    .background_workers
                    .auth_retention_enabled,
            finished: ctx
                .shared_map::<AuthRetentionWorkerHandle, _>(AuthRetentionWorkerHandle::is_finished),
        },
        RuntimeWorkerSnapshot {
            name: "module_event_dispatcher",
            required: local && listeners_required,
            finished: ctx.shared_map::<ModuleEventDispatcherHandle, _>(
                ModuleEventDispatcherHandle::is_finished,
            ),
        },
        RuntimeWorkerSnapshot {
            name: "module_transition_watchdog",
            required: local,
            finished: ctx.shared_map::<ModuleTransitionWatchdogHandle, _>(
                ModuleTransitionWatchdogHandle::is_finished,
            ),
        },
        RuntimeWorkerSnapshot {
            name: "module_artifact_maintenance",
            required: local,
            finished: ctx.shared_map::<ModuleArtifactMaintenanceHandle, _>(
                ModuleArtifactMaintenanceHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-payment")]
        RuntimeWorkerSnapshot {
            name: "payment_provider_event_recovery",
            required: local,
            finished: ctx.shared_map::<PaymentProviderEventWorkerHandle, _>(
                PaymentProviderEventWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-commerce")]
        RuntimeWorkerSnapshot {
            name: "paid_order_create_label",
            required: local,
            finished: ctx.shared_map::<PaidOrderCreateLabelWorkerHandle, _>(
                PaidOrderCreateLabelWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "commerce-marketplace-financial")]
        RuntimeWorkerSnapshot {
            name: "marketplace_financial_recovery",
            required: local,
            finished: ctx.shared_map::<MarketplaceFinancialWorkerHandle, _>(
                MarketplaceFinancialWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-product")]
        RuntimeWorkerSnapshot {
            name: "product_media_release",
            required: local,
            finished: ctx.shared_map::<ProductMediaReleaseWorkerHandle, _>(
                ProductMediaReleaseWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-workflow")]
        RuntimeWorkerSnapshot {
            name: "workflow_cron",
            required: local
                && ctx
                    .settings()
                    .runtime
                    .background_workers
                    .workflow_cron_enabled,
            finished: ctx.shared_map::<WorkflowCronSchedulerHandle, _>(
                WorkflowCronSchedulerHandle::is_finished,
            ),
        },
    ]
}

/// Resolves the local development database fallback without depending on a host config type.
pub fn resolve_boot_database_uri(
    database_url_present: bool,
    configured_uri: &str,
) -> Option<&'static str> {
    should_use_local_sqlite_fallback(database_url_present, configured_uri)
        .then_some(LOCAL_SQLITE_DATABASE_URI)
}

/// Start runtime workers from framework-neutral server runtime state.
pub async fn connect_runtime_workers_with_runtime(runtime_ctx: ServerRuntimeContext) -> Result<()> {
    let settings = runtime_ctx.settings().clone();
    #[cfg(feature = "mod-seo")]
    let seo_bulk_worker_enabled = settings.runtime.background_workers.seo_bulk_enabled;
    #[cfg(feature = "mod-seo")]
    let seo_sitemap_worker_enabled = settings.runtime.background_workers.seo_sitemap_enabled;
    #[cfg(feature = "mod-seo")]
    let seo_index_repair_worker_enabled =
        settings.runtime.background_workers.seo_index_repair_enabled;
    #[cfg(feature = "mod-seo")]
    let seo_worker_enabled =
        seo_bulk_worker_enabled || seo_sitemap_worker_enabled || seo_index_repair_worker_enabled;

    if !settings.runtime.runs_background_workers() {
        tracing::info!(host_mode = ?settings.runtime.host_mode, "Skipping background workers for non-worker host mode");
        return Ok(());
    }

    // Register the graceful-shutdown handle atomically so concurrent bootstrap calls cannot
    // create divergent stop channels.
    let stop_handle = StopHandle::ensure(&runtime_ctx);
    if stop_handle.is_stopping() {
        tracing::warn!("Shutdown already requested; skipping background worker startup");
        return Ok(());
    }
    let stop_rx = stop_handle.subscribe();

    if !runtime_ctx.shared_contains::<OutboxRelayWorkerHandle>() {
        let event_runtime = runtime_ctx
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .ok_or_else(|| Error::Message("EventRuntime not initialized".to_string()))?;

        if let Some(relay_config) = event_runtime.relay_config.clone() {
            runtime_ctx.shared_insert(spawn_relay_worker_handle(relay_config, stop_rx.clone()));
        }
    }

    if !runtime_ctx.shared_contains::<OutboxRetentionWorkerHandle>() {
        let retention_config = runtime_ctx
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .and_then(|runtime| runtime.outbox_retention.clone());
        if let Some(retention_config) = retention_config {
            runtime_ctx.shared_insert(spawn_outbox_retention_handle(
                retention_config,
                stop_rx.clone(),
            ));
        }
    }

    if settings.runtime.background_workers.auth_retention_enabled
        && !runtime_ctx.shared_contains::<AuthRetentionWorkerHandle>()
    {
        runtime_ctx.shared_insert(spawn_auth_retention_handle(
            runtime_ctx.db_clone(),
            stop_rx.clone(),
        ));
    }

    if settings.registry.remote_executor.enabled
        && !runtime_ctx.shared_contains::<RemoteExecutorReaperHandle>()
    {
        runtime_ctx.shared_insert(spawn_remote_executor_reaper_handle(
            runtime_ctx.clone(),
            settings.registry.remote_executor.requeue_scan_interval_ms,
            stop_rx.clone(),
        ));
    }

    #[cfg(feature = "mod-seo")]
    if seo_worker_enabled && !runtime_ctx.shared_contains::<SeoBulkWorkerHandle>() {
        let authorization = SeoWorkerAuthorization::from_runtime_config(
            settings.runtime.runs_background_workers(),
            seo_worker_enabled,
        )
        .map_err(|_| {
            Error::Message(
                "SEO worker execution is not authorized by runtime background-worker settings"
                    .to_string(),
            )
        })?;
        runtime_ctx.shared_insert(spawn_seo_bulk_worker_handle(
            runtime_ctx.clone(),
            stop_rx.clone(),
            authorization,
            seo_bulk_worker_enabled,
            seo_sitemap_worker_enabled,
            seo_index_repair_worker_enabled,
        ));
    } else if !seo_worker_enabled {
        tracing::info!("SEO workers disabled by runtime.background_workers config");
    }

    if settings.runtime.runs_background_workers()
        && !runtime_ctx.shared_contains::<crate::services::module_transition_watchdog::ModuleTransitionWatchdogHandle>()
    {
        runtime_ctx.shared_insert(
            crate::services::module_transition_watchdog::spawn_module_transition_watchdog_handle(
                runtime_ctx.db_clone(),
                5_000,
                stop_rx.clone(),
            ),
        );
    }

    if settings.runtime.runs_background_workers()
        && !runtime_ctx.shared_contains::<crate::services::module_artifact_maintenance::ModuleArtifactMaintenanceHandle>()
    {
        match runtime_ctx.shared_get::<rustok_storage::StorageRuntime>() {
            Some(storage) => {
                runtime_ctx.shared_insert(
                    crate::services::module_artifact_maintenance::spawn_module_artifact_maintenance_handle(
                        runtime_ctx.db_clone(),
                        storage,
                        60_000,
                        stop_rx.clone(),
                    ),
                );
            }
            None => {
                tracing::error!(
                    "Module artifact maintenance requires initialized durable storage; blob and data-object garbage collection are disabled"
                );
            }
        }
    }

    Ok(())
}

/// Stops all runtime workers that were registered during server bootstrap.
pub async fn shutdown_runtime_workers(runtime_ctx: &ServerRuntimeContext) {
    // Publish shutdown even when startup has not created a stop handle yet.
    // A concurrent initializer must not start workers after the drain.
    let stop = StopHandle::ensure(runtime_ctx);
    tracing::info!("Stopping background workers");
    stop.stop().await;

    let timeout_duration = Duration::from_secs(5);

    let relay_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<OutboxRelayWorkerHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Outbox relay worker stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "Outbox relay worker joined with error"),
                Err(_) => tracing::warn!("Outbox relay worker shutdown timed out after 5s"),
            }
        }
    };

    let retention_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<OutboxRetentionWorkerHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Outbox retention worker stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "Outbox retention worker joined with error"),
                Err(_) => tracing::warn!("Outbox retention worker shutdown timed out after 5s"),
            }
        }
    };

    let auth_retention_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<AuthRetentionWorkerHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Auth retention worker stopped gracefully"),
                Ok(Err(err)) => {
                    tracing::warn!(?err, "Auth retention worker joined with error")
                }
                Err(_) => {
                    tracing::warn!("Auth retention worker shutdown timed out after 5s")
                }
            }
        }
    };

    let reaper_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<RemoteExecutorReaperHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Remote executor reaper worker stopped gracefully"),
                Ok(Err(err)) => {
                    tracing::warn!(?err, "Remote executor reaper worker joined with error")
                }
                Err(_) => {
                    tracing::warn!("Remote executor reaper worker shutdown timed out after 5s")
                }
            }
        }
    };

    #[cfg(feature = "mod-seo")]
    let seo_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<SeoBulkWorkerHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("SEO bulk worker stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "SEO bulk worker joined with error"),
                Err(_) => tracing::warn!("SEO bulk worker shutdown timed out after 5s"),
            }
        }
    };
    #[cfg(not(feature = "mod-seo"))]
    let seo_drain = std::future::ready(());

    let module_work_drain = async {
        let initialization = ModuleWorkSchedulerInitializationLock::ensure(runtime_ctx);
        let deadline = tokio::time::Instant::now() + timeout_duration;
        let worker = match tokio::time::timeout_at(deadline, initialization.0.lock()).await {
            Ok(_guard) => runtime_ctx.shared_take::<ModuleWorkSchedulerHandle>(),
            Err(_) => {
                tracing::warn!("Module work scheduler initialization did not stop within 5s");
                return;
            }
        };
        if let Some(worker) = worker {
            match tokio::time::timeout_at(deadline, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Module work scheduler stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "Module work scheduler joined with error"),
                Err(_) => tracing::warn!("Module work scheduler shutdown timed out after 5s"),
            }
        }
    };

    #[cfg(feature = "mod-workflow")]
    let workflow_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<WorkflowCronSchedulerHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Workflow cron scheduler stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "Workflow cron scheduler joined with error"),
                Err(_) => tracing::warn!("Workflow cron scheduler shutdown timed out after 5s"),
            }
        }
    };
    #[cfg(not(feature = "mod-workflow"))]
    let workflow_drain = std::future::ready(());

    let watchdog_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::module_transition_watchdog::ModuleTransitionWatchdogHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Module transition watchdog worker stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "Module transition watchdog worker joined with error"),
                Err(_) => tracing::warn!("Module transition watchdog worker shutdown timed out after 5s"),
            }
        }
    };

    let module_event_dispatcher_drain = async {
        if let Some(worker) = runtime_ctx
            .shared_take::<crate::services::module_event_dispatcher::ModuleEventDispatcherHandle>(
        ) {
            // Request a cooperative drain first; if a handler blocks past the
            // host deadline, abort the dispatcher task as the emergency fallback.
            worker.request_stop();
            let abort_handle = worker.abort_handle();
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Module event dispatcher stopped gracefully"),
                Ok(Err(error)) => {
                    tracing::warn!(?error, "Module event dispatcher exited with error")
                }
                Err(_) => {
                    abort_handle.abort();
                    tracing::warn!("Module event dispatcher shutdown timed out after 5s")
                }
            }
        }
    };

    let module_artifact_maintenance_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::module_artifact_maintenance::ModuleArtifactMaintenanceHandle>() {
            match tokio::time::timeout(timeout_duration, worker.join()).await {
                Ok(Ok(())) => tracing::info!("Module artifact maintenance worker stopped gracefully"),
                Ok(Err(err)) => tracing::warn!(?err, "Module artifact maintenance worker joined with error"),
                Err(_) => tracing::warn!("Module artifact maintenance worker shutdown timed out after 5s"),
            }
        }
    };

    #[cfg(feature = "mod-commerce")]
    let paid_order_label_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle>() {
            await_module_worker_shutdown("Paid-order label", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-commerce"))]
    let paid_order_label_drain = std::future::ready(());

    #[cfg(feature = "commerce-marketplace-financial")]
    let marketplace_financial_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle>() {
            await_module_worker_shutdown("Marketplace financial", worker.join()).await;
        }
    };
    #[cfg(not(feature = "commerce-marketplace-financial"))]
    let marketplace_financial_drain = std::future::ready(());

    #[cfg(feature = "mod-payment")]
    let payment_provider_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle>() {
            await_module_worker_shutdown("Payment provider event", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-payment"))]
    let payment_provider_drain = std::future::ready(());

    #[cfg(feature = "mod-product")]
    let product_media_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::product_media_release_worker::ProductMediaReleaseWorkerHandle>() {
            await_module_worker_shutdown("Product media release", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-product"))]
    let product_media_drain = std::future::ready(());

    #[cfg(feature = "mod-notifications")]
    let notification_outbox_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::notification_outbox_intake_worker::NotificationOutboxIntakeWorkerHandle>() {
            await_module_worker_shutdown("Notification outbox intake", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-notifications"))]
    let notification_outbox_drain = std::future::ready(());

    #[cfg(feature = "mod-notifications")]
    let notification_candidate_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::notification_candidate_worker::NotificationCandidateWorkerHandle>() {
            await_module_worker_shutdown("Notification candidate", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-notifications"))]
    let notification_candidate_drain = std::future::ready(());

    #[cfg(feature = "mod-notifications")]
    let notification_fanout_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::notification_fanout_worker::NotificationFanoutWorkerHandle>() {
            await_module_worker_shutdown("Notification fanout", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-notifications"))]
    let notification_fanout_drain = std::future::ready(());

    #[cfg(feature = "mod-forum")]
    let forum_search_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::forum_search_inbox_worker::ForumSearchInboxWorkerHandle>() {
            await_module_worker_shutdown("Forum search inbox", worker.join()).await;
        }
    };
    #[cfg(not(feature = "mod-forum"))]
    let forum_search_drain = std::future::ready(());

    tokio::join!(
        relay_drain,
        retention_drain,
        auth_retention_drain,
        reaper_drain,
        seo_drain,
        module_work_drain,
        workflow_drain,
        watchdog_drain,
        module_event_dispatcher_drain,
        module_artifact_maintenance_drain,
        paid_order_label_drain,
        marketplace_financial_drain,
        payment_provider_drain,
        product_media_drain,
        notification_outbox_drain,
        notification_candidate_drain,
        notification_fanout_drain,
        forum_search_drain,
    );

    // These supervisors do not consume StopHandle. Explicitly abort them even
    // if other clones of their handles or the shared context remain alive.
    if let Some(listener) = runtime_ctx.shared_take::<
        crate::services::channel_cache_invalidation::ChannelCacheInvalidationListenerHandle,
    >() {
        listener.abort();
    }
    if let Some(listener) = runtime_ctx.shared_take::<
        crate::services::rbac_cache_invalidation::RbacCacheInvalidationListenerHandle,
    >() {
        listener.abort();
    }
    if let Some(watchdog) = runtime_ctx.shared_take::<
        crate::services::rbac_invalidation_generation::RbacInvalidationGenerationWatchdogHandle,
    >() {
        watchdog.abort();
    }
    if let Some(forwarder) =
        runtime_ctx.shared_take::<crate::services::event_bus::EventForwarderHandle>()
    {
        forwarder.abort();
    }
}

/// Drain module-owned workers after broadcasting the shutdown signal. The
/// timeout mirrors host worker drains; durable recovery state survives a
/// process exit even if a provider or database call is still blocked.
#[cfg(any(
    test,
    feature = "mod-order",
    feature = "commerce-marketplace-financial",
    feature = "mod-payment",
    feature = "mod-product",
    feature = "mod-notifications",
    feature = "mod-forum"
))]
async fn await_module_worker_shutdown(
    worker: &'static str,
    join: impl std::future::Future<Output = std::result::Result<(), tokio::task::JoinError>>,
) {
    match tokio::time::timeout(Duration::from_secs(5), join).await {
        Ok(Ok(())) => tracing::info!(worker, "Module worker stopped gracefully"),
        Ok(Err(error)) => tracing::warn!(worker, ?error, "Module worker exited with error"),
        Err(_) => tracing::warn!(worker, "Module worker shutdown timed out after 5s"),
    }
}

fn spawn_relay_worker_handle(
    relay_config: RelayRuntimeConfig,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> OutboxRelayWorkerHandle {
    let instance_id = OUTBOX_RELAY_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        worker = "outbox_relay",
        instance_id,
        "Starting runtime worker"
    );
    OutboxRelayWorkerHandle {
        instance_id,
        _handle: spawn_outbox_relay_worker(relay_config, stop_rx),
    }
}

fn spawn_outbox_retention_handle(
    retention_config: OutboxRetentionRuntimeConfig,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> OutboxRetentionWorkerHandle {
    let instance_id = OUTBOX_RETENTION_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        worker = "outbox_retention",
        instance_id,
        "Starting runtime worker"
    );
    OutboxRetentionWorkerHandle {
        instance_id,
        _handle: spawn_outbox_retention_worker(retention_config, stop_rx),
    }
}

fn spawn_auth_retention_handle(
    db: sea_orm::DatabaseConnection,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> AuthRetentionWorkerHandle {
    let instance_id = AUTH_RETENTION_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        worker = "auth_retention",
        instance_id,
        "Starting runtime worker"
    );
    AuthRetentionWorkerHandle {
        instance_id,
        _handle: tokio::spawn(auth_retention_loop(db, stop_rx)),
    }
}

fn auth_retention_removed_artifacts(
    purge: &crate::services::auth_retention::AuthRetentionPurge,
) -> bool {
    purge.sessions > 0
        || purge.invite_consumptions > 0
        || purge.invites > 0
        || purge.oauth_tokens > 0
        || purge.oauth_authorization_codes > 0
}

async fn auth_retention_loop(
    db: sea_orm::DatabaseConnection,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    // Daily cadence is enough for a 30-day retention window.
    let poll_interval = Duration::from_secs(24 * 60 * 60);

    loop {
        if *stop_rx.borrow() {
            tracing::info!("Auth retention worker received shutdown signal, exiting");
            return;
        }

        match crate::services::auth_retention::purge_expired_auth_artifacts(&db).await {
            Ok(purge) if auth_retention_removed_artifacts(&purge) => {
                tracing::info!(
                    sessions = purge.sessions,
                    invite_consumptions = purge.invite_consumptions,
                    invites = purge.invites,
                    oauth_tokens = purge.oauth_tokens,
                    oauth_authorization_codes = purge.oauth_authorization_codes,
                    "Purged expired auth artifacts past retention"
                );
            }
            Ok(_) => {}
            Err(error) => tracing::error!(
                error = %error,
                "Auth retention worker failed to purge expired artifacts"
            ),
        }

        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Auth retention worker received shutdown signal, exiting");
                    return;
                }
            }
        }
    }
}

fn spawn_remote_executor_reaper_handle(
    runtime_ctx: ServerRuntimeContext,
    scan_interval_ms: u64,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> RemoteExecutorReaperHandle {
    let instance_id = REMOTE_EXECUTOR_REAPER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        worker = "remote_executor_reaper",
        instance_id,
        "Starting runtime worker"
    );
    RemoteExecutorReaperHandle {
        instance_id,
        _handle: tokio::spawn(remote_executor_reaper_loop(
            runtime_ctx,
            scan_interval_ms,
            stop_rx,
        )),
    }
}

#[cfg(feature = "mod-seo")]
fn spawn_seo_bulk_worker_handle(
    runtime_ctx: ServerRuntimeContext,
    stop_rx: tokio::sync::watch::Receiver<bool>,
    authorization: SeoWorkerAuthorization,
    bulk_enabled: bool,
    sitemap_enabled: bool,
    index_repair_enabled: bool,
) -> SeoBulkWorkerHandle {
    let instance_id = SEO_BULK_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        worker = "seo",
        instance_id,
        bulk_enabled,
        sitemap_enabled,
        index_repair_enabled,
        "Starting runtime worker"
    );
    SeoBulkWorkerHandle {
        instance_id,
        _handle: tokio::spawn(seo_worker_loop(
            runtime_ctx,
            stop_rx,
            authorization,
            bulk_enabled,
            sitemap_enabled,
            index_repair_enabled,
        )),
    }
}

async fn remote_executor_reaper_loop(
    runtime_ctx: ServerRuntimeContext,
    scan_interval_ms: u64,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let governance = ModuleControlPlane::new(runtime_ctx.db_clone()).publication();
    let poll_interval = Duration::from_millis(scan_interval_ms.max(1));

    loop {
        if *stop_rx.borrow() {
            tracing::info!("Remote executor reaper received shutdown signal, exiting");
            return;
        }

        match governance.requeue_expired_remote_validation_claims().await {
            Ok(requeued) if requeued > 0 => tracing::info!(
                requeued,
                "Remote executor reaper requeued expired validation stage claims"
            ),
            Ok(_) => {}
            Err(error) => tracing::error!(
                error = %error,
                "Remote executor reaper failed to process expired validation stage claims"
            ),
        }

        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Remote executor reaper received shutdown signal, exiting");
                    return;
                }
            }
        }
    }
}

#[cfg(feature = "mod-seo")]
async fn seo_worker_loop(
    runtime_ctx: ServerRuntimeContext,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
    authorization: SeoWorkerAuthorization,
    bulk_enabled: bool,
    sitemap_enabled: bool,
    index_repair_enabled: bool,
) {
    let event_bus = transactional_event_bus_from_context(&runtime_ctx);
    let runtime_extensions = module_runtime_extensions_from_ctx(&runtime_ctx);
    let service = match SeoApplicationServices::from_runtime_extensions(
        runtime_ctx.db_clone(),
        event_bus,
        &runtime_extensions,
    ) {
        Ok(service) => service,
        Err(error) => {
            tracing::error!(error = %error, "Failed to initialize SEO bulk worker registry");
            return;
        }
    };
    let poll_interval = Duration::from_millis(SEO_BULK_WORKER_POLL_INTERVAL_MS);

    loop {
        if *stop_rx.borrow() {
            tracing::info!("SEO bulk worker received shutdown signal, exiting");
            return;
        }

        if bulk_enabled {
            match service.bulk().execute_next_bulk_job(&authorization).await {
                Ok(Some(job)) => tracing::info!(
                    job_id = %job.id,
                    operation = %job.operation_kind.as_str(),
                    status = %job.status.as_str(),
                    "Executed queued SEO bulk job"
                ),
                Ok(None) => {}
                Err(error) => tracing::error!(
                    error = %error,
                    "SEO bulk worker failed to execute queued job"
                ),
            }
        }

        if *stop_rx.borrow() {
            tracing::info!("SEO bulk worker received shutdown signal, exiting");
            return;
        }

        if sitemap_enabled {
            match service
                .bulk()
                .execute_next_sitemap_job(&authorization)
                .await
            {
                Ok(Some(job)) => tracing::info!(
                    job_id = %job.id,
                    status = %job.status,
                    "Executed queued SEO sitemap job phase"
                ),
                Ok(None) => {}
                Err(error) => tracing::error!(
                    error = %error,
                    "SEO sitemap worker failed to execute queued job"
                ),
            }
        }

        if *stop_rx.borrow() {
            tracing::info!("SEO bulk worker received shutdown signal, exiting");
            return;
        }

        if index_repair_enabled {
            match service
                .bulk()
                .execute_next_index_repair_job(&authorization)
                .await
            {
                Ok(Some(result)) => tracing::info!(
                    target_type = ?result.target_type,
                    replay_mode = %result.replay_mode.as_str(),
                    repaired_count = result.repaired_count,
                    replayed_count = result.replayed_count,
                    "Executed queued SEO index repair/replay job"
                ),
                Ok(None) => {}
                Err(error) => tracing::error!(
                    error = %error,
                    "SEO index repair worker failed to execute queued job"
                ),
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("SEO bulk worker received shutdown signal, exiting");
                    return;
                }
            }
        }
    }
}

fn should_use_local_sqlite_fallback(database_url_present: bool, current_uri: &str) -> bool {
    !database_url_present
        && (current_uri.is_empty()
            || current_uri.contains("localhost:5432")
            || current_uri.contains("db:5432"))
}

#[cfg(test)]
mod tests {
    use super::{
        OutboxRelayWorkerHandle, OutboxRetentionWorkerHandle, connect_runtime_workers_with_runtime,
        resolve_boot_database_uri, should_use_local_sqlite_fallback,
    };
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_core::events::{EventBus, MemoryTransport};
    use rustok_outbox::{OutboxRelay, OutboxRetention, OutboxTransport};
    use rustok_test_utils::setup_test_db;
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use std::{sync::Arc, time::Duration};

    use crate::services::event_transport_factory::{
        EventRuntime, OutboxRetentionRuntimeConfig, RelayRuntimeConfig,
    };

    #[tokio::test]
    async fn additional_workers_respect_host_modes_and_auth_retention_config() {
        use super::additional_runtime_worker_snapshots;
        use crate::common::settings::RuntimeHostMode;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        for (mode, expected) in [
            (RuntimeHostMode::Full, true),
            (RuntimeHostMode::Worker, true),
            (RuntimeHostMode::Api, false),
            (RuntimeHostMode::AdminSsr, false),
            (RuntimeHostMode::StorefrontSsr, false),
            (RuntimeHostMode::RegistryOnly, false),
        ] {
            let mut settings = RustokSettings::default();
            settings.runtime.host_mode = mode;
            let ctx = ServerRuntimeContext::new(db.clone(), settings);
            let snapshots = additional_runtime_worker_snapshots(&ctx);
            for worker in &snapshots {
                assert_eq!(worker.required, expected, "{}", worker.name);
                assert_eq!(worker.finished, None, "{}", worker.name);
            }
        }
        let mut settings = RustokSettings::default();
        settings.runtime.background_workers.auth_retention_enabled = false;
        let ctx = ServerRuntimeContext::new(db, settings);
        let auth = additional_runtime_worker_snapshots(&ctx)
            .into_iter()
            .find(|worker| worker.name == "auth_retention")
            .unwrap();
        assert!(!auth.required);
    }

    #[test]
    fn uses_sqlite_fallback_when_database_url_is_missing_and_uri_is_empty() {
        assert!(should_use_local_sqlite_fallback(false, ""));
        assert_eq!(
            resolve_boot_database_uri(false, ""),
            Some("sqlite://rustok.sqlite?mode=rwc")
        );
    }

    #[test]
    fn uses_sqlite_fallback_when_database_url_is_missing_and_uri_points_to_local_postgres() {
        assert!(should_use_local_sqlite_fallback(
            false,
            "postgres://postgres:postgres@localhost:5432/rustok"
        ));
        assert!(should_use_local_sqlite_fallback(
            false,
            "postgres://postgres:postgres@db:5432/rustok"
        ));
    }

    #[test]
    fn skips_sqlite_fallback_when_database_url_exists_or_uri_is_remote() {
        assert!(!should_use_local_sqlite_fallback(
            true,
            "postgres://postgres:postgres@localhost:5432/rustok"
        ));
        assert!(!should_use_local_sqlite_fallback(
            false,
            "postgres://postgres:postgres@prod-db.internal:5432/rustok"
        ));
    }

    #[tokio::test]
    async fn stop_handle_broadcasts_graceful_shutdown_signal() {
        let (stop_handle, mut initial_rx) = super::StopHandle::new();
        let mut subscribed_rx = stop_handle.subscribe();

        assert!(!*initial_rx.borrow());
        assert!(!*subscribed_rx.borrow());

        stop_handle.stop().await;

        initial_rx
            .changed()
            .await
            .expect("initial receiver should observe stop signal");
        subscribed_rx
            .changed()
            .await
            .expect("subscribed receiver should observe stop signal");
        assert!(*initial_rx.borrow());
        assert!(*subscribed_rx.borrow());
    }

    #[tokio::test]
    async fn connect_runtime_workers_is_idempotent_for_outbox_relay_handle() {
        let db = setup_test_db().await;
        // The retention worker prunes against `sys_events`, so the schema it
        // reads exists before the worker starts (a missing table would make every
        // run log a failure instead of proving the worker is wired).
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("outbox schema for the retention worker");
        let relay_config = RelayRuntimeConfig {
            interval: Duration::from_secs(60),
            relay: OutboxRelay::new(db.clone(), Arc::new(MemoryTransport::new())),
        };
        let runtime = Arc::new(EventRuntime {
            delivery_profile: crate::common::settings::EventDeliveryProfile::Outbox,
            iggy_mode: None,
            transport: Arc::new(OutboxTransport::new(db.clone())),
            listener_bus: EventBus::new(),
            relay_config: Some(relay_config),
            // Both outbox workers are resolved from the runtime, so enabling
            // retention here also proves the second handle is started once.
            outbox_retention: Some(OutboxRetentionRuntimeConfig {
                interval: Duration::from_secs(60),
                retention: OutboxRetention::new(db.clone()),
            }),
            channel_capacity: 128,
            relay_fallback_active: false,
        });
        let runtime_ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        runtime_ctx.shared_insert(runtime);

        connect_runtime_workers_with_runtime(runtime_ctx.clone())
            .await
            .expect("first worker connect should succeed");
        let first_instance_id = runtime_ctx
            .shared_map::<OutboxRelayWorkerHandle, _>(OutboxRelayWorkerHandle::instance_id)
            .expect("relay handle should be stored");

        connect_runtime_workers_with_runtime(runtime_ctx.clone())
            .await
            .expect("second worker connect should be idempotent");
        let second_instance_id = runtime_ctx
            .shared_map::<OutboxRelayWorkerHandle, _>(OutboxRelayWorkerHandle::instance_id)
            .expect("relay handle should still be stored");

        assert_eq!(first_instance_id, second_instance_id);

        let first_retention_instance_id = runtime_ctx
            .shared_map::<OutboxRetentionWorkerHandle, _>(OutboxRetentionWorkerHandle::instance_id)
            .expect("retention handle should be stored");

        connect_runtime_workers_with_runtime(runtime_ctx.clone())
            .await
            .expect("third worker connect should be idempotent");
        let second_retention_instance_id = runtime_ctx
            .shared_map::<OutboxRetentionWorkerHandle, _>(OutboxRetentionWorkerHandle::instance_id)
            .expect("retention handle should still be stored");

        assert_eq!(
            first_retention_instance_id, second_retention_instance_id,
            "the retention worker must not be started twice"
        );

        // Gracefully shut down and drain background workers to avoid hanging tests
        super::shutdown_runtime_workers(&runtime_ctx).await;
    }

    #[test]
    fn runtime_worker_lifecycle_state_computes_correct_transitions_and_metrics() {
        use super::RuntimeWorkerLifecycleState;

        // When stop_requested is true, state is always Stopping
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(true, None, true),
            RuntimeWorkerLifecycleState::Stopping
        );
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(true, Some(false), true),
            RuntimeWorkerLifecycleState::Stopping
        );
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(false, Some(true), true),
            RuntimeWorkerLifecycleState::Stopping
        );

        // Required worker transitions
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(true, None, false),
            RuntimeWorkerLifecycleState::Starting
        );
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(true, Some(false), false),
            RuntimeWorkerLifecycleState::Ready
        );
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(true, Some(true), false),
            RuntimeWorkerLifecycleState::Failed
        );

        // Non-required worker transitions
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(false, None, false),
            RuntimeWorkerLifecycleState::Ready
        );
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(false, Some(false), false),
            RuntimeWorkerLifecycleState::Ready
        );
        assert_eq!(
            RuntimeWorkerLifecycleState::from_worker_snapshot(false, Some(true), false),
            RuntimeWorkerLifecycleState::Degraded
        );

        // String and metric representation
        assert_eq!(RuntimeWorkerLifecycleState::Starting.as_str(), "starting");
        assert_eq!(RuntimeWorkerLifecycleState::Starting.metric_value(), 1);

        assert_eq!(RuntimeWorkerLifecycleState::Ready.as_str(), "ready");
        assert_eq!(RuntimeWorkerLifecycleState::Ready.metric_value(), 2);

        assert_eq!(RuntimeWorkerLifecycleState::Degraded.as_str(), "degraded");
        assert_eq!(RuntimeWorkerLifecycleState::Degraded.metric_value(), 3);

        assert_eq!(RuntimeWorkerLifecycleState::Stopping.as_str(), "stopping");
        assert_eq!(RuntimeWorkerLifecycleState::Stopping.metric_value(), 4);

        assert_eq!(RuntimeWorkerLifecycleState::Failed.as_str(), "failed");
        assert_eq!(RuntimeWorkerLifecycleState::Failed.metric_value(), 5);
    }

    #[tokio::test]
    async fn stop_handle_send_replace_works_without_active_receivers_and_is_idempotent() {
        let (stop_handle, initial_rx) = super::StopHandle::new();
        // Drop the initial receiver to simulate 0 active listeners
        drop(initial_rx);

        assert!(!stop_handle.is_stopping());

        // Stop should store true even without active receivers
        stop_handle.stop().await;
        assert!(stop_handle.is_stopping());

        // Subscribing after stop immediately sees true
        let late_rx = stop_handle.subscribe();
        assert!(*late_rx.borrow());

        // Repeated stop is idempotent
        stop_handle.stop().await;
        assert!(stop_handle.is_stopping());
    }

    #[tokio::test]
    async fn connect_runtime_workers_skips_startup_if_already_stopping() {
        let db = setup_test_db().await;
        let runtime_ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let stop_handle = super::StopHandle::ensure(&runtime_ctx);
        stop_handle.stop().await;

        // connect_runtime_workers should notice is_stopping() and return Ok without starting workers
        let res = connect_runtime_workers_with_runtime(runtime_ctx.clone()).await;
        assert!(res.is_ok());
        assert!(!runtime_ctx.shared_contains::<OutboxRelayWorkerHandle>());
    }

    #[test]
    fn auth_retention_reports_oauth_only_cleanup() {
        let oauth_only = crate::services::auth_retention::AuthRetentionPurge {
            oauth_tokens: 3,
            ..Default::default()
        };
        assert!(super::auth_retention_removed_artifacts(&oauth_only));
        let code_only = crate::services::auth_retention::AuthRetentionPurge {
            oauth_authorization_codes: 2,
            ..Default::default()
        };
        assert!(super::auth_retention_removed_artifacts(&code_only));
        assert!(!super::auth_retention_removed_artifacts(
            &crate::services::auth_retention::AuthRetentionPurge::default()
        ));
    }

    #[tokio::test]
    async fn module_worker_join_finishes_after_watched_shutdown() {
        let (stop, mut rx) = super::StopHandle::new();
        let complete = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = complete.clone();
        let handle = tokio::spawn(async move {
            rx.changed().await.expect("shutdown broadcast");
            assert!(*rx.borrow());
            observed.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        stop.stop().await;
        super::await_module_worker_shutdown("shutdown-regression", handle).await;
        assert!(complete.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[cfg(feature = "mod-payment")]
    #[tokio::test]
    async fn shutdown_drains_payment_provider_event_recovery_worker() {
        let db = setup_test_db().await;
        let runtime_ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let stop = super::StopHandle::ensure(&runtime_ctx);
        let worker =
            crate::services::payment_provider_event_worker::spawn_payment_provider_event_worker(
                runtime_ctx.clone(),
                stop.subscribe(),
            );
        assert!(worker.instance_id() > 0);
        runtime_ctx.shared_insert(worker);
        assert!(runtime_ctx.shared_contains::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >());

        tokio::time::timeout(
            Duration::from_secs(5),
            super::shutdown_runtime_workers(&runtime_ctx),
        )
        .await
        .expect("payment recovery task must drain on shutdown");
        assert!(stop.is_stopping());
        assert!(!runtime_ctx.shared_contains::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >());
    }

    #[tokio::test]
    async fn shutdown_aborts_invalidation_and_event_supervisors_even_with_live_clones() {
        use crate::services::channel_cache_invalidation::ChannelCacheInvalidationListenerHandle;
        use crate::services::event_bus::EventForwarderHandle;
        use crate::services::rbac_cache_invalidation::RbacCacheInvalidationListenerHandle;
        use crate::services::rbac_invalidation_generation::RbacInvalidationGenerationWatchdogHandle;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let (channel, mut tasks) = ChannelCacheInvalidationListenerHandle::for_shutdown_test();
        let (rbac, other) = RbacCacheInvalidationListenerHandle::for_shutdown_test();
        tasks.extend(other);
        let (watchdog, other) = RbacInvalidationGenerationWatchdogHandle::for_shutdown_test();
        tasks.extend(other);
        let (forwarder, other) = EventForwarderHandle::for_shutdown_test();
        tasks.extend(other);
        // Retain clones to prove shutdown does not rely on the last Arc being dropped.
        ctx.shared_insert(channel.clone());
        ctx.shared_insert(rbac.clone());
        ctx.shared_insert(watchdog.clone());
        ctx.shared_insert(forwarder.clone());
        assert!(tasks.iter().all(|task| !task.is_finished()));

        super::shutdown_runtime_workers(&ctx).await;
        tokio::time::timeout(Duration::from_secs(2), async {
            while tasks.iter().any(|task| !task.is_finished()) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("all supervisor tasks must be aborted");
        assert!(!channel.is_running());
        assert!(!rbac.is_running());
        assert!(!watchdog.is_running());
        assert!(!forwarder.is_running());
        assert!(!ctx.shared_contains::<ChannelCacheInvalidationListenerHandle>());
        assert!(!ctx.shared_contains::<RbacCacheInvalidationListenerHandle>());
        assert!(!ctx.shared_contains::<RbacInvalidationGenerationWatchdogHandle>());
        assert!(!ctx.shared_contains::<EventForwarderHandle>());
    }

    #[tokio::test]
    async fn shutdown_runtime_workers_concurrently_drains_module_work_handle() {
        let db = setup_test_db().await;
        let runtime_ctx = ServerRuntimeContext::new(db, RustokSettings::default());

        let stop = super::StopHandle::ensure(&runtime_ctx).subscribe();
        let join_handle = tokio::spawn(async move {
            let mut stop = stop;
            let _ = stop.changed().await;
        });

        let worker_handle = super::ModuleWorkSchedulerHandle::new(join_handle);
        assert!(!worker_handle.is_finished());
        assert!(worker_handle.instance_id() > 0);

        runtime_ctx.shared_insert(worker_handle);
        assert!(runtime_ctx.shared_contains::<super::ModuleWorkSchedulerHandle>());

        super::shutdown_runtime_workers(&runtime_ctx).await;

        assert!(!runtime_ctx.shared_contains::<super::ModuleWorkSchedulerHandle>());
        assert!(super::StopHandle::ensure(&runtime_ctx).is_stopping());
    }
}
