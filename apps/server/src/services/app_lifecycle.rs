use crate::error::{Error, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;

#[cfg(feature = "mod-seo")]
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
#[cfg(feature = "mod-seo")]
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::event_transport_factory::{
    EventRuntime, RelayRuntimeConfig, spawn_outbox_relay_worker,
};
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_modules::ModuleControlPlane;
#[cfg(feature = "mod-seo")]
use rustok_seo::SeoApplicationServices;

// в”Ђв”Ђ Graceful-shutdown handle в”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђв”Ђ

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
    /// notified when [`StopHandle::stop`] is called.  Clone it once per background worker
    /// so each worker gets its own independent view of the channel.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.stop_tx.subscribe()
    }

    pub async fn stop(&self) {
        let _ = self.stop_tx.send(true);
        // Yield so spawned tasks have a chance to notice the signal.
        tokio::task::yield_now().await;
    }

    pub fn is_stopping(&self) -> bool {
        *self.stop_tx.borrow()
    }
}

static OUTBOX_RELAY_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static REMOTE_EXECUTOR_REAPER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "mod-seo")]
static SEO_BULK_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

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

    if !settings.runtime.runs_background_workers() {
        tracing::info!(host_mode = ?settings.runtime.host_mode, "Skipping background workers for non-worker host mode");
        return Ok(());
    }

    // Register graceful-shutdown handle if not already present.
    if !runtime_ctx.shared_contains::<StopHandle>() {
        let (handle, _rx) = StopHandle::new();
        runtime_ctx.shared_insert(handle);
    }

    // Obtain a stop receiver from the stored handle so workers can observe
    // the shutdown signal.  `subscribe()` creates a new independent receiver
    // from the existing sender вЂ” safe to call multiple times.
    let stop_handle = runtime_ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before spawning workers");
    let stop_rx = stop_handle.subscribe();

    if !runtime_ctx.shared_contains::<OutboxRelayWorkerHandle>() {
        let event_runtime = runtime_ctx
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .ok_or_else(|| Error::Message("EventRuntime not initialized".to_string()))?;

        if let Some(relay_config) = event_runtime.relay_config.clone() {
            runtime_ctx.shared_insert(spawn_relay_worker_handle(relay_config, stop_rx.clone()));
        }
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
    if seo_bulk_worker_enabled && !runtime_ctx.shared_contains::<SeoBulkWorkerHandle>() {
        runtime_ctx.shared_insert(spawn_seo_bulk_worker_handle(
            runtime_ctx.clone(),
            stop_rx.clone(),
        ));
    } else if !seo_bulk_worker_enabled {
        tracing::info!("SEO bulk worker disabled by runtime.background_workers config");
    }

    Ok(())
}

/// Stops all runtime workers that were registered during server bootstrap.
pub async fn shutdown_runtime_workers(runtime_ctx: &ServerRuntimeContext) {
    if let Some(handle) = runtime_ctx.shared_get::<StopHandle>() {
        tracing::info!("Stopping background workers");
        handle.stop().await;
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
) -> SeoBulkWorkerHandle {
    let instance_id = SEO_BULK_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(worker = "seo_bulk", instance_id, "Starting runtime worker");
    SeoBulkWorkerHandle {
        instance_id,
        _handle: tokio::spawn(seo_bulk_worker_loop(runtime_ctx, stop_rx)),
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
            _ = stop_rx.changed() => {
                tracing::info!("Remote executor reaper received shutdown signal, exiting");
                return;
            }
        }
    }
}

#[cfg(feature = "mod-seo")]
async fn seo_bulk_worker_loop(
    runtime_ctx: ServerRuntimeContext,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
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

        match service.bulk().execute_next_bulk_job().await {
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

        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {}
            _ = stop_rx.changed() => {
                tracing::info!("SEO bulk worker received shutdown signal, exiting");
                return;
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
        OutboxRelayWorkerHandle, connect_runtime_workers_with_runtime, resolve_boot_database_uri,
        should_use_local_sqlite_fallback,
    };
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_core::events::{EventBus, MemoryTransport};
    use rustok_outbox::{OutboxRelay, OutboxTransport};
    use rustok_test_utils::setup_test_db;
    use std::{sync::Arc, time::Duration};

    use crate::services::event_transport_factory::{EventRuntime, RelayRuntimeConfig};

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
        let relay_config = RelayRuntimeConfig {
            interval: Duration::from_secs(60),
            relay: OutboxRelay::new(db.clone(), Arc::new(MemoryTransport::new())),
        };
        let runtime = Arc::new(EventRuntime {
            delivery_profile: crate::common::settings::EventDeliveryProfile::OutboxLocal,
            iggy_mode: None,
            transport: Arc::new(OutboxTransport::new(db.clone())),
            listener_bus: EventBus::new(),
            relay_config: Some(relay_config),
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

        // Gracefully shut down background workers to avoid hanging tests
        if let Some(stop_handle) = runtime_ctx.shared_get::<super::StopHandle>() {
            stop_handle.stop().await;
        };
    }
}
