use crate::error::{Error, Result};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::task::{JoinError, JoinHandle, JoinSet};

#[cfg(feature = "mod-seo")]
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::event_transport_factory::{
    EventRuntime, OutboxRetentionRuntimeConfig, RelayRuntimeConfig, spawn_outbox_relay_worker,
};
use crate::services::module_transition_watchdog::ModuleTransitionWatchdogHandle;
use crate::services::outbox_retention_worker::spawn_outbox_retention_worker;
use crate::services::server_runtime_context::{ServerRuntimeContext, published_worker_running};
#[cfg(feature = "mod-seo")]
use crate::services::server_runtime_context::module_runtime_extensions_from_ctx;
use rustok_modules::ModuleControlPlane;
#[cfg(feature = "mod-seo")]
use rustok_seo::{SeoApplicationServices, SeoWorkerAuthorization};

// ── Graceful-shutdown handle ─────────────────────────────────────────────────

pub use crate::services::stop_handle::StopHandle;

static OUTBOX_RELAY_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static OUTBOX_RETENTION_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static AUTH_RETENTION_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static REMOTE_EXECUTOR_REAPER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "mod-seo")]
static SEO_BULK_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static MODULE_WORK_SCHEDULER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "mod-workflow")]
static WORKFLOW_CRON_SCHEDULER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
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

#[cfg(feature = "mod-workflow")]
#[derive(Clone, Default)]
pub(crate) struct WorkflowCronSchedulerInitializationLock(
    pub(crate) std::sync::Arc<tokio::sync::Mutex<()>>,
);

#[cfg(feature = "mod-workflow")]
impl WorkflowCronSchedulerInitializationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }
}

#[derive(Clone, Default)]
struct ModuleArtifactMaintenanceInitializationLock(std::sync::Arc<tokio::sync::Mutex<()>>);

impl ModuleArtifactMaintenanceInitializationLock {
    fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }
}

/// Serializes the physical DLQ duplicate scanner and its companion's
/// admission with shutdown's JoinHandle snapshot. Only synchronous operations
/// run while this lock is held, including the async scanner starter (which has
/// no await points); it is distinct from the router bootstrap mutex.
#[derive(Clone, Default)]
pub(crate) struct EventDlqObserverInitializationLock(
    pub(crate) std::sync::Arc<std::sync::Mutex<()>>,
);

impl EventDlqObserverInitializationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }
}

/// Serializes both Social Graph operational observers' handle publication
/// against the runtime shutdown snapshot. This lock is independent of the
/// bootstrap lock: direct observer starters also use it without needing to
/// acquire the bootstrap lock recursively.
#[cfg(feature = "mod-social_graph")]
#[derive(Clone, Default)]
pub(crate) struct SocialGraphObserverInitializationLock(
    pub(crate) std::sync::Arc<tokio::sync::Mutex<()>>,
);

#[cfg(feature = "mod-social_graph")]
impl SocialGraphObserverInitializationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }
}

/// Direct, synchronous worker starters can bypass router bootstrap. Guard
/// their final shutdown check -> task spawn -> JoinHandle publication against
/// the matching shutdown take. Do not hold this blocking gate across awaits or
/// while joining tasks. One runtime-local gate covers the Product Search channel
/// projection repair, three Notifications, Forum Search inbox/contract,
/// Product Index, Social Graph Index, Comments TCP listener, and Comments
/// durable audit handoff workers.
#[derive(Clone, Default)]
pub(crate) struct DirectWorkerRegistrationLock(
    pub(crate) std::sync::Arc<std::sync::Mutex<()>>,
);

impl DirectWorkerRegistrationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }
}

/// Take a directly registered worker under the same gate as its final spawn.
/// Shutdown publishes stop before entering the drain, so a starter that loses
/// this gate will observe stop and refuse to publish a new task.
fn take_direct_worker_for_shutdown<T>(ctx: &ServerRuntimeContext) -> Option<T>
where
    T: 'static + Send + Sync,
{
    let registration = DirectWorkerRegistrationLock::ensure(ctx);
    let _guard = registration
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ctx.shared_take::<T>()
}

/// Serializes the complete server bootstrap against competing bootstraps and
/// the shutdown drain. Several worker starters intentionally use a
/// check-then-insert sequence; this host-level fence makes that sequence
/// single-owner for one ServerRuntimeContext.
#[derive(Clone, Default)]
pub(crate) struct ServerBootstrapInitializationLock(
    std::sync::Arc<tokio::sync::Mutex<()>>,
);

impl ServerBootstrapInitializationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }

    pub(crate) async fn lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.0.lock().await
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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
    }
}

#[cfg(feature = "mod-workflow")]
#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkflowCronSchedulerRequirement(pub(crate) bool);

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

    fn into_task(self) -> JoinHandle<()> {
        self._handle
    }
}

/// One host-owned worker observation, shared by readiness and Prometheus.
/// It reports local task availability and supervisor health, not backlog drainage or recent progress.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeWorkerSnapshot {
    pub name: &'static str,
    pub required: bool,
    pub finished: Option<bool>,
}

pub(crate) fn additional_cache_reconciliation_worker_snapshots(
    ctx: &ServerRuntimeContext,
) -> Vec<RuntimeWorkerSnapshot> {
    let snapshots = vec![RuntimeWorkerSnapshot {
        name: "field_definition_cache_generation",
        required: true,
        finished: ctx.shared_map::<
            crate::services::field_definition_cache::FieldDefinitionCacheGenerationReconciliationHandle,
            _,
        >(|worker| !worker.is_ready()),
    }];

    #[cfg(feature = "mod-seo")]
    snapshots.push(RuntimeWorkerSnapshot {
        name: "seo_redirect_cache_reconciliation",
        required: crate::services::seo_redirect_cache_reconciliation::seo_redirect_cache_reconciliation_required(ctx),
        finished: ctx.shared_map::<
            crate::services::seo_redirect_cache_reconciliation::SeoRedirectCacheReconciliationHandle,
            _,
        >(|worker| !worker.is_ready()),
    });

    snapshots
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
    use crate::services::event_bus::EventForwarderHandle;
    #[cfg(feature = "mod-commerce")]
    use crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle;
    #[cfg(feature = "mod-payment")]
    use crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle;
    #[cfg(feature = "mod-product")]
    use crate::services::product_media_release_worker::ProductMediaReleaseWorkerHandle;
    #[cfg(feature = "mod-forum")]
    use crate::services::forum_search_inbox_worker::{
        ForumSearchContractConsumerWorkerHandle, ForumSearchInboxWorkerHandle,
    };
    #[cfg(feature = "mod-product")]
    use crate::services::product_index_refresh_worker::ProductIndexRefreshWorkerHandle;
    #[cfg(feature = "mod-social_graph")]
    use crate::services::social_graph_index_worker::SocialGraphIndexWorkerHandle;
    #[cfg(feature = "mod-notifications")]
    use crate::services::notification_candidate_worker::NotificationCandidateWorkerHandle;
    #[cfg(feature = "mod-notifications")]
    use crate::services::notification_fanout_worker::NotificationFanoutWorkerHandle;
    #[cfg(feature = "mod-notifications")]
    use crate::services::notification_outbox_intake_worker::NotificationOutboxIntakeWorkerHandle;
    #[cfg(feature = "mod-comments")]
    use crate::services::comments_provider_runtime::{
        CommentsTcpDelegationScheduleAuditHandoffWorkerHandle, CommentsTcpListenerTask,
    };

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
    let mut snapshots = vec![
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
            name: "event_forwarder",
            required: local
                && ctx.shared_contains::<crate::services::event_bus::SharedEventBus>(),
            finished: ctx.shared_map::<EventForwarderHandle, _>(|worker| !worker.is_running()),
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
        // These durable broker consumers deliberately stop with their source
        // offset uncommitted on poison/receive failures. Once started, their
        // liveness is part of readiness so a dead consumer cannot leave the
        // HTTP host green while its projection queue is stalled.
        #[cfg(feature = "mod-product")]
        RuntimeWorkerSnapshot {
            name: "product_index_refresh",
            required: local
                && ctx.shared_contains::<ProductIndexRefreshWorkerHandle>(),
            finished: ctx.shared_map::<ProductIndexRefreshWorkerHandle, _>(|worker| {
                !worker.is_ready()
            }),
        },
        #[cfg(feature = "mod-social_graph")]
        RuntimeWorkerSnapshot {
            name: "social_graph_index",
            required: local && ctx.shared_contains::<SocialGraphIndexWorkerHandle>(),
            finished: ctx.shared_map::<SocialGraphIndexWorkerHandle, _>(
                SocialGraphIndexWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-forum")]
        RuntimeWorkerSnapshot {
            name: "forum_search_inbox",
            required: local && ctx.shared_contains::<ForumSearchInboxWorkerHandle>(),
            finished: ctx.shared_map::<ForumSearchInboxWorkerHandle, _>(
                ForumSearchInboxWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-forum")]
        RuntimeWorkerSnapshot {
            name: "forum_search_contract_consumer",
            required: local
                && ctx.shared_contains::<ForumSearchContractConsumerWorkerHandle>(),
            finished: ctx.shared_map::<ForumSearchContractConsumerWorkerHandle, _>(
                |worker| worker.is_finished(),
            ),
        },
        #[cfg(feature = "mod-comments")]
        RuntimeWorkerSnapshot {
            name: "comments_tcp_listener",
            required: local && ctx.shared_contains::<CommentsTcpListenerTask>(),
            finished: ctx.shared_map::<CommentsTcpListenerTask, _>(|task| task.task.is_finished()),
        },
        #[cfg(feature = "mod-comments")]
        RuntimeWorkerSnapshot {
            name: "comments_schedule_audit_handoff",
            required: local
                && ctx.shared_contains::<CommentsTcpDelegationScheduleAuditHandoffWorkerHandle>(),
            finished: ctx.shared_map::<CommentsTcpDelegationScheduleAuditHandoffWorkerHandle, _>(
                CommentsTcpDelegationScheduleAuditHandoffWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-notifications")]
        RuntimeWorkerSnapshot {
            name: "notification_outbox_intake",
            required: local
                && ctx.shared_contains::<NotificationOutboxIntakeWorkerHandle>(),
            finished: ctx.shared_map::<NotificationOutboxIntakeWorkerHandle, _>(
                NotificationOutboxIntakeWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-notifications")]
        RuntimeWorkerSnapshot {
            name: "notification_fanout",
            required: local && ctx.shared_contains::<NotificationFanoutWorkerHandle>(),
            finished: ctx.shared_map::<NotificationFanoutWorkerHandle, _>(
                NotificationFanoutWorkerHandle::is_finished,
            ),
        },
        #[cfg(feature = "mod-notifications")]
        RuntimeWorkerSnapshot {
            name: "notification_candidate",
            required: local && ctx.shared_contains::<NotificationCandidateWorkerHandle>(),
            finished: ctx.shared_map::<NotificationCandidateWorkerHandle, _>(
                NotificationCandidateWorkerHandle::is_finished,
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
                    .shared_get::<WorkflowCronSchedulerRequirement>()
                    .map(|requirement| requirement.0)
                    .unwrap_or_else(|| {
                        ctx.settings()
                            .runtime
                            .background_workers
                            .workflow_cron_enabled
                    }),
            finished: ctx.shared_map::<WorkflowCronSchedulerHandle, _>(
                WorkflowCronSchedulerHandle::is_finished,
            ),
        },
    ];
    snapshots.extend(additional_cache_reconciliation_worker_snapshots(ctx));
    snapshots
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
    // Serialize the maintenance handle's take/join/reinsert lifecycle with both
    // concurrent startup calls and the shutdown drain.
    let artifact_initialization = ModuleArtifactMaintenanceInitializationLock::ensure(&runtime_ctx);
    let _artifact_initialization_guard = artifact_initialization.0.lock().await;
    if stop_handle.is_stopping() {
        tracing::warn!("Shutdown requested while waiting for artifact maintenance initialization");
        return Ok(());
    }
    let artifact_maintenance_running = match runtime_ctx
        .shared_take::<crate::services::module_artifact_maintenance::ModuleArtifactMaintenanceHandle>()
    {
        Some(worker) if worker.is_finished() => {
            if let Err(error) = worker.join().await {
                tracing::warn!(?error, "Previous module artifact maintenance worker exited");
            }
            false
        }
        Some(worker) => {
            runtime_ctx.shared_insert(worker);
            true
        }
        None => false,
    };
    let durable_storage_available = runtime_ctx.shared_contains::<rustok_storage::StorageRuntime>();
    if !artifact_maintenance_running && !durable_storage_available {
        return Err(Error::Message(
            "Module artifact maintenance is required for background-worker hosts, but durable storage is not initialized"
                .to_string(),
        ));
    }
    let stop_rx = stop_handle.subscribe();

    if !published_worker_running::<OutboxRelayWorkerHandle>(
        &runtime_ctx,
        "Outbox relay worker",
        OutboxRelayWorkerHandle::is_finished,
    )? {
        let event_runtime = runtime_ctx
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .ok_or_else(|| Error::Message("EventRuntime not initialized".to_string()))?;

        if let Some(relay_config) = event_runtime.relay_config.clone() {
            runtime_ctx.shared_insert(spawn_relay_worker_handle(relay_config, stop_rx.clone()));
        }
    }

    if !published_worker_running::<OutboxRetentionWorkerHandle>(
        &runtime_ctx,
        "Outbox retention worker",
        OutboxRetentionWorkerHandle::is_finished,
    )? {
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
        && !published_worker_running::<AuthRetentionWorkerHandle>(
            &runtime_ctx,
            "Auth retention worker",
            AuthRetentionWorkerHandle::is_finished,
        )?
    {
        runtime_ctx.shared_insert(spawn_auth_retention_handle(
            runtime_ctx.db_clone(),
            stop_rx.clone(),
        ));
    }

    if settings.registry.remote_executor.enabled
        && !published_worker_running::<RemoteExecutorReaperHandle>(
            &runtime_ctx,
            "Remote executor reaper",
            RemoteExecutorReaperHandle::is_finished,
        )?
    {
        runtime_ctx.shared_insert(spawn_remote_executor_reaper_handle(
            runtime_ctx.clone(),
            settings.registry.remote_executor.requeue_scan_interval_ms,
            stop_rx.clone(),
        ));
    }

    #[cfg(feature = "mod-seo")]
    if seo_worker_enabled && !published_worker_running::<SeoBulkWorkerHandle>(
        &runtime_ctx,
        "SEO bulk worker",
        SeoBulkWorkerHandle::is_finished,
    )? {
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
        && !published_worker_running::<ModuleTransitionWatchdogHandle>(
            &runtime_ctx,
            "Module transition watchdog",
            ModuleTransitionWatchdogHandle::is_finished,
        )?
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
                let worker = crate::services::module_artifact_maintenance::spawn_module_artifact_maintenance_handle(
                    runtime_ctx.db_clone(),
                    storage,
                    60_000,
                    stop_rx.clone(),
                )
                .map_err(Error::Message)?;
                runtime_ctx.shared_insert(worker);
            }
            None => {
                return Err(Error::Message(
                    "Module artifact maintenance is required for background-worker hosts, but durable storage is not initialized"
                        .to_string(),
                ));
            }
        }
    }

    Ok(())
}

const INSTALLER_JOB_SHUTDOWN_GRACE: Duration = Duration::from_secs(60);
const INSTALLER_JOB_ABORT_JOIN_GRACE: Duration = Duration::from_secs(5);

struct InstallerJobTaskRegistry {
    state: Mutex<InstallerJobTaskRegistryState>,
}

struct InstallerJobTaskRegistryState {
    closed: bool,
    tasks: JoinSet<()>,
}

impl Default for InstallerJobTaskRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(InstallerJobTaskRegistryState {
                closed: false,
                tasks: JoinSet::new(),
            }),
        }
    }
}

impl InstallerJobTaskRegistry {
    fn spawn<F>(&self, task: F) -> bool
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return false;
        }

        // Reap completed jobs before admitting another one. The JoinSet keeps
        // ownership of every live task and makes panics observable.
        while let Some(result) = state.tasks.try_join_next() {
            observe_installer_job_task_result(result);
        }
        let _abort_handle = state.tasks.spawn(task);
        true
    }

    async fn drain(&self) {
        self.drain_with_timeout(INSTALLER_JOB_SHUTDOWN_GRACE, INSTALLER_JOB_ABORT_JOIN_GRACE)
            .await;
    }

    async fn drain_with_timeout(&self, grace: Duration, abort_join_grace: Duration) {
        let mut tasks = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.closed = true;
            std::mem::replace(&mut state.tasks, JoinSet::new())
        };

        if tasks.is_empty() {
            return;
        }

        let pending = tasks.len();
        tracing::info!(pending, "Draining accepted installer HTTP jobs");
        let drained = tokio::time::timeout(grace, async {
            while let Some(result) = tasks.join_next().await {
                observe_installer_job_task_result(result);
            }
        })
        .await;

        if drained.is_err() {
            let pending = tasks.len();
            tracing::error!(
                pending,
                "Installer HTTP job shutdown grace expired; aborting in-flight work and leaving any durable running claim in place for operator reconciliation"
            );
            tasks.abort_all();
            let joined = tokio::time::timeout(abort_join_grace, async {
                while let Some(result) = tasks.join_next().await {
                    observe_installer_job_task_result(result);
                }
            })
            .await;
            if joined.is_err() {
                tracing::error!(
                    pending = tasks.len(),
                    "Installer HTTP job tasks did not finish after abort; dropping the JoinSet will abort remaining tasks"
                );
            }
        }
    }
}

fn observe_installer_job_task_result(result: std::result::Result<(), JoinError>) {
    if let Err(error) = result {
        if error.is_cancelled() {
            tracing::warn!("Installer HTTP job task was cancelled during shutdown");
        } else {
            tracing::error!(
                panicked = error.is_panic(),
                "Installer HTTP job task exited without completing normally"
            );
        }
    }
}

/// Register an HTTP installer job in the runtime-owned JoinSet.
///
/// The registry's close operation is serialized with task admission, so a
/// request racing shutdown is either included in the drain or rejected.
pub(crate) fn spawn_installer_job_task<F>(ctx: &ServerRuntimeContext, task: F) -> bool
where
    F: Future<Output = ()> + Send + 'static,
{
    let registry =
        ctx.shared_get_or_insert_with(|| std::sync::Arc::new(InstallerJobTaskRegistry::default()));
    registry.spawn(task)
}

/// Close installer-job admission and join all accepted tasks before the event
/// forwarder is drained.
pub(crate) async fn drain_installer_job_tasks(ctx: &ServerRuntimeContext) {
    let registry =
        ctx.shared_get_or_insert_with(|| std::sync::Arc::new(InstallerJobTaskRegistry::default()));
    registry.drain().await
}

/// Stops all runtime workers that were registered during server bootstrap.
pub async fn shutdown_runtime_workers(runtime_ctx: &ServerRuntimeContext) {
    // Publish shutdown even when startup has not created a stop handle yet.
    // A concurrent initializer must not start workers after the drain.
    let stop = StopHandle::ensure(runtime_ctx);
    tracing::info!("Stopping background workers");
    stop.stop().await;

    // Wait for any in-flight bootstrap to finish registering its workers
    // before taking the shutdown snapshot. The stop signal above ensures a
    // bootstrap that was already running will not start long-lived work.
    let bootstrap_initialization = ServerBootstrapInitializationLock::ensure(runtime_ctx);
    let _bootstrap_guard = bootstrap_initialization.lock().await;

    // Direct event-runtime construction can bypass router bootstrap. Wait
    // for its publication owner before taking worker snapshots.
    // Lock order matches router startup: bootstrap -> event runtime.
    let event_initialization =
        crate::services::event_transport_factory::EventRuntimeInitializationLock::ensure(runtime_ctx);
    let event_guard = event_initialization.lock().await;
    drop(event_guard);

    // Router bootstrap holds the lock above, but this public startup function
    // can also be invoked directly. It uses the artifact maintenance lock
    // across *all* additional worker registration. Wait for that caller to
    // finish before the shutdown snapshot, or a late worker could publish its
    // handle after the drain already took the old value.
    //
    // Lock order matches router bootstrap: bootstrap -> runtime workers.
    // Release the workers lock before join! (the artifact worker's own drain
    // acquires it again). New callers see the stop flag after acquiring it
    // and do not start more workers.
    let worker_initialization = ModuleArtifactMaintenanceInitializationLock::ensure(runtime_ctx);
    let _worker_initialization_guard = worker_initialization.0.lock().await;
    drop(_worker_initialization_guard);

    // Direct observer startup can bypass router bootstrap. Wait for each
    // observer's check-stop -> spawn -> publish sequence to complete before
    // taking the shutdown JoinHandle snapshot. Lock order is bootstrap ->
    // runtime workers -> observer; observer starters never acquire either
    // outer lock while holding this one.
    #[cfg(feature = "mod-social_graph")]
    let observer_initialization = SocialGraphObserverInitializationLock::ensure(runtime_ctx);
    #[cfg(feature = "mod-social_graph")]
    let _observer_initialization_guard = observer_initialization.0.lock().await;
    #[cfg(feature = "mod-social_graph")]
    drop(_observer_initialization_guard);

    // Direct DLQ observer/projection startup may bypass router bootstrap.
    // Synchronize both check-stop -> publish paths with the shutdown snapshot.
    let dlq_observer_initialization = EventDlqObserverInitializationLock::ensure(runtime_ctx);
    let _dlq_observer_guard = dlq_observer_initialization
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    drop(_dlq_observer_guard);

    let timeout_duration = Duration::from_secs(5);

    let rate_limit_cleanup_drain = async {
        // Retain the terminal registry even if shutdown wins before the first
        // initializer. Its mutex serializes the snapshot with publication.
        let registry = runtime_ctx.shared_get_or_insert_with(
            crate::middleware::rate_limit::RateLimitCleanupTaskRegistry::default,
        );
        let tasks = registry.close_and_take_tasks();
        for task in &tasks {
            task.abort();
        }
        await_runtime_worker_shutdown_tasks_with_timeout(
            "Rate-limit cleanup",
            tasks,
            timeout_duration,
        )
        .await;
    };

    let relay_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<OutboxRelayWorkerHandle>() {
            await_runtime_worker_shutdown("Outbox relay worker", Some(worker.into_task())).await;
        }
    };

    let retention_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<OutboxRetentionWorkerHandle>() {
            await_runtime_worker_shutdown("Outbox retention worker", Some(worker.into_task()))
                .await;
        }
    };

    let auth_retention_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<AuthRetentionWorkerHandle>() {
            await_runtime_worker_shutdown("Auth retention worker", Some(worker.into_task())).await;
        }
    };

    // Auth sends are request-spawned, not startup-owned. Their registry retains
    // task ownership after HTTP callers discard JoinHandles, and closes new
    // admission while allowing a bounded grace period for accepted sends.
    let auth_email_drain = crate::services::email::drain_auth_email_tasks(runtime_ctx);
    let installer_job_drain = drain_installer_job_tasks(runtime_ctx);

    let reaper_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<RemoteExecutorReaperHandle>() {
            await_runtime_worker_shutdown(
                "Remote executor reaper worker",
                Some(worker.into_task()),
            )
            .await;
        }
    };

    #[cfg(feature = "mod-seo")]
    let seo_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<SeoBulkWorkerHandle>() {
            await_runtime_worker_shutdown("SEO bulk worker", Some(worker.into_task())).await;
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
            await_runtime_worker_shutdown_until(
                "Module work scheduler",
                worker.into_task(),
                deadline,
            )
            .await;
        }
    };

    #[cfg(feature = "mod-workflow")]
    let workflow_drain = async {
        let initialization = WorkflowCronSchedulerInitializationLock::ensure(runtime_ctx);
        let deadline = tokio::time::Instant::now() + timeout_duration;
        let worker = match tokio::time::timeout_at(deadline, initialization.0.lock()).await {
            Ok(_guard) => runtime_ctx.shared_take::<WorkflowCronSchedulerHandle>(),
            Err(_) => {
                tracing::warn!("Workflow cron scheduler initialization did not stop within 5s");
                return;
            }
        };
        if let Some(worker) = worker {
            await_runtime_worker_shutdown_until(
                "Workflow cron scheduler",
                worker.into_task(),
                deadline,
            )
            .await;
        }
    };
    #[cfg(not(feature = "mod-workflow"))]
    let workflow_drain = std::future::ready(());

    let watchdog_drain = async {
        if let Some(worker) = runtime_ctx.shared_take::<crate::services::module_transition_watchdog::ModuleTransitionWatchdogHandle>() {
            await_runtime_worker_shutdown("Module transition watchdog", Some(worker.into_task())).await;
        }
    };

    let module_event_dispatcher_drain = async {
        if let Some(worker) =
            crate::services::module_event_dispatcher::take_module_dispatcher_for_shutdown(runtime_ctx)
        {
            // Request a cooperative drain first; the bounded helper aborts and
            // joins the dispatcher if an in-flight handler exceeds the deadline.
            worker.request_stop();
            await_runtime_worker_shutdown("Module event dispatcher", Some(worker.into_task()))
                .await;
        }
    };

    let module_artifact_maintenance_drain = async {
        let initialization = ModuleArtifactMaintenanceInitializationLock::ensure(runtime_ctx);
        let deadline = tokio::time::Instant::now() + timeout_duration;
        let worker = match tokio::time::timeout_at(deadline, initialization.0.lock()).await {
            Ok(_guard) => runtime_ctx
                .shared_take::<crate::services::module_artifact_maintenance::ModuleArtifactMaintenanceHandle>(),
            Err(_) => {
                tracing::warn!("Module artifact maintenance initialization did not stop within 5s");
                return;
            }
        };
        if let Some(worker) = worker {
            await_runtime_worker_shutdown_until(
                "Module artifact maintenance",
                worker.into_task(),
                deadline,
            )
            .await;
        }
    };

    #[cfg(feature = "mod-comments")]
    let comments_tcp_drain = async {
        if let Some(listener_task) = take_direct_worker_for_shutdown::<
            crate::services::comments_provider_runtime::CommentsTcpListenerTask,
        >(runtime_ctx) {
            await_runtime_worker_shutdown_with_timeout(
                "Comments TCP listener",
                Some(listener_task.task),
                listener_task.shutdown_timeout,
            )
            .await;
        }
        runtime_ctx
            .shared_remove::<crate::services::comments_provider_runtime::CommentsTcpListenerHandle>(
            );
    };
    #[cfg(not(feature = "mod-comments"))]
    let comments_tcp_drain = std::future::ready(());

    #[cfg(feature = "mod-comments")]
    let comments_schedule_audit_handoff_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<
            crate::services::comments_provider_runtime::CommentsTcpDelegationScheduleAuditHandoffWorkerHandle,
        >(runtime_ctx) {
            await_runtime_worker_shutdown(
                "Comments schedule audit handoff",
                worker.take_task(),
            )
            .await;
        }
    };
    #[cfg(not(feature = "mod-comments"))]
    let comments_schedule_audit_handoff_drain = std::future::ready(());

    #[cfg(feature = "mod-commerce")]
    let paid_order_label_drain = async {
        if let Some(worker) = crate::services::module_event_dispatcher::take_dispatcher_started_worker_for_shutdown::<crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Paid-order label", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-commerce"))]
    let paid_order_label_drain = std::future::ready(());

    #[cfg(feature = "commerce-marketplace-financial")]
    let marketplace_financial_drain = async {
        if let Some(worker) = crate::services::module_event_dispatcher::take_dispatcher_started_worker_for_shutdown::<crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Marketplace financial", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "commerce-marketplace-financial"))]
    let marketplace_financial_drain = std::future::ready(());

    #[cfg(feature = "mod-payment")]
    let payment_provider_drain = async {
        if let Some(worker) = crate::services::module_event_dispatcher::take_dispatcher_started_worker_for_shutdown::<crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Payment provider event", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-payment"))]
    let payment_provider_drain = std::future::ready(());

    #[cfg(feature = "mod-product")]
    let product_media_drain = async {
        if let Some(worker) = crate::services::module_event_dispatcher::take_dispatcher_started_worker_for_shutdown::<crate::services::product_media_release_worker::ProductMediaReleaseWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Product media release", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-product"))]
    let product_media_drain = std::future::ready(());

    let channel_cache_invalidation_drain = async {
        let tasks = crate::services::channel_cache_invalidation::take_listener_tasks_for_shutdown(
            runtime_ctx,
        )
        .await;
        for task in &tasks {
            task.abort();
        }
        await_runtime_worker_shutdown_tasks("Channel cache invalidation", tasks).await;
    };

    let rbac_cache_invalidation_drain = async {
        let tasks =
            crate::services::rbac_cache_invalidation::take_listener_tasks_for_shutdown(runtime_ctx)
                .await;
        for task in &tasks {
            task.abort();
        }
        await_runtime_worker_shutdown_tasks("RBAC cache invalidation", tasks).await;
    };

    let rbac_generation_watchdog_drain = async {
        if let Some(task) =
            crate::services::rbac_invalidation_generation::take_watchdog_task_for_shutdown(
                runtime_ctx,
            )
            .await
        {
            task.abort();
            await_runtime_worker_shutdown("RBAC invalidation generation watchdog", Some(task))
                .await;
        }
    };

    let event_forwarder_drain = async {
        if let Some(task) =
            crate::services::event_bus::take_forwarder_task_for_shutdown(runtime_ctx)
        {
            await_runtime_worker_shutdown_with_timeout(
                "Event forwarder",
                Some(task),
                Duration::from_secs(30),
            )
            .await;
        }
    };

    let cache_redis_status_drain = async {
        if let Some(task) =
            crate::services::cache_redis_status_monitor::take_task_for_shutdown(runtime_ctx).await
        {
            task.abort();
            await_runtime_worker_shutdown("Cache Redis status monitor", Some(task)).await;
        }
    };

    let field_definition_cache_invalidation_drain = async {
        if let Some(task) =
            crate::services::field_definition_cache::take_invalidation_task_for_shutdown(
                runtime_ctx,
            )
        {
            task.abort();
            await_runtime_worker_shutdown("Field definition cache invalidation", Some(task)).await;
        }
    };

    let field_definition_cache_reconciliation_drain = async {
        if let Some(task) =
            crate::services::field_definition_cache::take_task_for_shutdown(runtime_ctx)
        {
            task.abort();
            await_runtime_worker_shutdown("Field definition cache reconciliation", Some(task))
                .await;
        }
    };

    #[cfg(feature = "mod-seo")]
    let seo_redirect_cache_reconciliation_drain = async {
        if let Some(task) =
            crate::services::seo_redirect_cache_reconciliation::take_task_for_shutdown(runtime_ctx)
        {
            task.abort();
            await_runtime_worker_shutdown("SEO redirect cache reconciliation", Some(task)).await;
        }
    };
    #[cfg(not(feature = "mod-seo"))]
    let seo_redirect_cache_reconciliation_drain = std::future::ready(());

    let tenant_locale_generation_drain = async {
        let tasks = crate::services::tenant_locale_generation::take_listener_tasks_for_shutdown(
            runtime_ctx,
        )
        .await;
        for task in &tasks {
            task.abort();
        }
        await_runtime_worker_shutdown_tasks("Tenant locale generation listener", tasks).await;
    };

    let tenant_cache_generation_drain = async {
        let tasks =
            crate::services::tenant_cache_generation::take_listener_tasks_for_shutdown(runtime_ctx)
                .await;
        for task in &tasks {
            task.abort();
        }
        await_runtime_worker_shutdown_tasks("Tenant cache generation listener", tasks).await;
    };

    let event_dlq_duplicate_alert_drain = async {
        if let Some(mut worker) = runtime_ctx
            .shared_take::<crate::services::event_dlq_duplicate_alert_observer::EventDlqDuplicateAlertObserverHandle>()
        {
            await_runtime_worker_shutdown(
                "Event DLQ duplicate alert observer",
                worker.take_task(),
            )
            .await;
        }
    };

    let event_dlq_duplicate_alert_observability_drain = async {
        if let Some(mut worker) = runtime_ctx
            .shared_take::<crate::services::event_dlq_duplicate_alert_observability::EventDlqDuplicateAlertObservabilityHandle>()
        {
            await_runtime_worker_shutdown(
                "Event DLQ duplicate alert observability",
                worker.take_task(),
            )
            .await;
        }
    };

    let product_channel_reconciliation_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<
            crate::services::search_product_channel_reconciliation::ProductChannelProjectionWorkerHandle,
        >(runtime_ctx) {
            await_runtime_worker_shutdown(
                "Product channel projection reconciliation",
                Some(worker.into_task()),
            )
            .await;
        }
    };

    #[cfg(feature = "mod-product")]
    let product_index_refresh_drain = async {
        #[cfg(feature = "mod-product")]
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::product_index_refresh_worker::ProductIndexRefreshWorkerHandle>(runtime_ctx)
        {
            await_runtime_worker_shutdown(
                "Product index refresh",
                Some(worker.into_task()),
            )
            .await;
        }
    };
    #[cfg(not(feature = "mod-product"))]
    let product_index_refresh_drain = std::future::ready(());

    #[cfg(feature = "mod-social_graph")]
    let social_graph_index_drain = async {
        #[cfg(feature = "mod-social_graph")]
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::social_graph_index_worker::SocialGraphIndexWorkerHandle>(runtime_ctx)
        {
            await_runtime_worker_shutdown(
                "Social Graph Index consumer",
                Some(worker.into_task()),
            )
            .await;
        }
    };
    #[cfg(not(feature = "mod-social_graph"))]
    let social_graph_index_drain = std::future::ready(());

    #[cfg(feature = "mod-social_graph")]
    let social_graph_position_observer_drain = async {
        #[cfg(feature = "mod-social_graph")]
        if let Some(worker) = runtime_ctx
            .shared_take::<crate::services::social_graph_index_position_observer::SocialGraphIndexPositionObserverHandle>()
        {
            await_runtime_worker_shutdown(
                "Social Graph Index position observer",
                Some(worker.into_task()),
            )
            .await;
        }
    };
    #[cfg(not(feature = "mod-social_graph"))]
    let social_graph_position_observer_drain = std::future::ready(());

    #[cfg(feature = "mod-social_graph")]
    let social_graph_poison_observer_drain = async {
        #[cfg(feature = "mod-social_graph")]
        if let Some(worker) = runtime_ctx
            .shared_take::<crate::services::social_graph_index_poison_observer::SocialGraphIndexPoisonObserverHandle>()
        {
            await_runtime_worker_shutdown(
                "Social Graph Index poison observer",
                Some(worker.into_task()),
            )
            .await;
        }
    };
    #[cfg(not(feature = "mod-social_graph"))]
    let social_graph_poison_observer_drain = std::future::ready(());

    #[cfg(feature = "mod-notifications")]
    let notification_outbox_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::notification_outbox_intake_worker::NotificationOutboxIntakeWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Notification outbox intake", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-notifications"))]
    let notification_outbox_drain = std::future::ready(());

    #[cfg(feature = "mod-notifications")]
    let notification_candidate_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::notification_candidate_worker::NotificationCandidateWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Notification candidate", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-notifications"))]
    let notification_candidate_drain = std::future::ready(());

    #[cfg(feature = "mod-notifications")]
    let notification_fanout_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::notification_fanout_worker::NotificationFanoutWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Notification fanout", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-notifications"))]
    let notification_fanout_drain = std::future::ready(());

    #[cfg(feature = "mod-forum")]
    let forum_search_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::forum_search_inbox_worker::ForumSearchInboxWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Forum search inbox", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-forum"))]
    let forum_search_drain = std::future::ready(());

    #[cfg(feature = "mod-forum")]
    let forum_search_contract_consumer_drain = async {
        if let Some(worker) = take_direct_worker_for_shutdown::<crate::services::forum_search_inbox_worker::ForumSearchContractConsumerWorkerHandle>(runtime_ctx) {
            await_module_worker_shutdown("Forum Search contract consumer", worker.into_task()).await;
        }
    };
    #[cfg(not(feature = "mod-forum"))]
    let forum_search_contract_consumer_drain = std::future::ready(());

    tokio::join!(
        rate_limit_cleanup_drain,
        relay_drain,
        retention_drain,
        auth_retention_drain,
        auth_email_drain,
        installer_job_drain,
        reaper_drain,
        seo_drain,
        module_work_drain,
        workflow_drain,
        watchdog_drain,
        module_artifact_maintenance_drain,
        comments_tcp_drain,
        comments_schedule_audit_handoff_drain,
        channel_cache_invalidation_drain,
        rbac_cache_invalidation_drain,
        rbac_generation_watchdog_drain,
        cache_redis_status_drain,
        field_definition_cache_invalidation_drain,
        field_definition_cache_reconciliation_drain,
        seo_redirect_cache_reconciliation_drain,
        tenant_locale_generation_drain,
        tenant_cache_generation_drain,
        event_dlq_duplicate_alert_drain,
        event_dlq_duplicate_alert_observability_drain,
        product_channel_reconciliation_drain,
        product_index_refresh_drain,
        social_graph_index_drain,
        social_graph_position_observer_drain,
        social_graph_poison_observer_drain,
        paid_order_label_drain,
        marketplace_financial_drain,
        payment_provider_drain,
        product_media_drain,
        notification_outbox_drain,
        notification_candidate_drain,
        notification_fanout_drain,
        forum_search_drain,
        forum_search_contract_consumer_drain,
    );

    // Keep the dispatcher subscribed until all upstream producers, including
    // the Outbox relay's current durable pass, have finished. Stopping the
    // dispatcher in the parallel join could make an otherwise acknowledged
    // relay delivery fail after shutdown requested a graceful drain.
    module_event_dispatcher_drain.await;

    // The event forwarder is the last producer-side drain: dispatcher
    // handlers may enqueue final events while they finish, so only after
    // their join may the forwarder observe an empty queue and exit.
    event_forwarder_drain.await;
}

/// Guard a task handle so cancellation of the shutdown future cannot turn a
/// locally owned JoinHandle into a detached task.
struct ShutdownJoinHandle(Option<JoinHandle<()>>);

impl ShutdownJoinHandle {
    fn new(task: JoinHandle<()>) -> Self {
        Self(Some(task))
    }

    fn handle_mut(&mut self) -> &mut JoinHandle<()> {
        // INVARIANT: handle_mut is invoked only while the task is armed before disarm/completion.
        self.0.as_mut().expect("shutdown task handle is present")
    }

    fn abort(&self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }

    fn disarm(&mut self) {
        self.0.take();
    }
}

impl Drop for ShutdownJoinHandle {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}

/// Drain runtime-owned tasks after broadcasting stop. On timeout, abort and
/// join them so dropping the runtime context cannot detach them.
fn await_runtime_worker_shutdown(
    worker: &'static str,
    task: Option<JoinHandle<()>>,
) -> impl std::future::Future<Output = ()> {
    await_runtime_worker_shutdown_tasks(worker, task.into_iter().collect())
}

fn await_runtime_worker_shutdown_tasks(
    worker: &'static str,
    tasks: Vec<JoinHandle<()>>,
) -> impl std::future::Future<Output = ()> {
    await_runtime_worker_shutdown_tasks_with_timeout(worker, tasks, Duration::from_secs(5))
}

fn await_runtime_worker_shutdown_with_timeout(
    worker: &'static str,
    task: Option<JoinHandle<()>>,
    timeout_duration: Duration,
) -> impl std::future::Future<Output = ()> {
    await_runtime_worker_shutdown_tasks_with_timeout(
        worker,
        task.into_iter().collect(),
        timeout_duration,
    )
}

fn await_runtime_worker_shutdown_until(
    worker: &'static str,
    task: JoinHandle<()>,
    deadline: tokio::time::Instant,
) -> impl std::future::Future<Output = ()> {
    let mut task = ShutdownJoinHandle::new(task);
    async move {
        match tokio::time::timeout_at(deadline, task.handle_mut()).await {
            Ok(Ok(())) => task.disarm(),
            Ok(Err(error)) if error.is_cancelled() => task.disarm(),
            Ok(Err(error)) => {
                task.disarm();
                tracing::warn!(worker, ?error, "Runtime worker exited with error");
            }
            Err(_) => {
                tracing::warn!(worker, "Runtime worker shutdown timed out; aborting");
                task.abort();
                if let Err(error) = task.handle_mut().await {
                    if !error.is_cancelled() {
                        tracing::warn!(worker, ?error, "Runtime worker failed after abort");
                    }
                }
                task.disarm();
            }
        }
    }
}

fn await_runtime_worker_shutdown_tasks_with_timeout(
    worker: &'static str,
    tasks: Vec<JoinHandle<()>>,
    timeout_duration: Duration,
) -> impl std::future::Future<Output = ()> {
    let tasks: Vec<ShutdownJoinHandle> = tasks.into_iter().map(ShutdownJoinHandle::new).collect();
    async move {
        let deadline = tokio::time::Instant::now() + timeout_duration;
        let mut remaining = tasks.into_iter();
        while let Some(mut task) = remaining.next() {
            match tokio::time::timeout_at(deadline, task.handle_mut()).await {
                Ok(Ok(())) => task.disarm(),
                Ok(Err(error)) if error.is_cancelled() => task.disarm(),
                Ok(Err(error)) => {
                    task.disarm();
                    tracing::warn!(worker, ?error, "Runtime worker exited with error");
                }
                Err(_) => {
                    tracing::warn!(
                        worker,
                        "Runtime worker shutdown timed out; aborting remaining tasks"
                    );
                    task.abort();
                    if let Err(error) = task.handle_mut().await {
                        if !error.is_cancelled() {
                            tracing::warn!(worker, ?error, "Runtime worker failed after abort");
                        }
                    }
                    task.disarm();

                    let remaining: Vec<_> = remaining.collect();
                    for task in &remaining {
                        task.abort();
                    }
                    for mut task in remaining {
                        if let Err(error) = task.handle_mut().await {
                            if !error.is_cancelled() {
                                tracing::warn!(worker, ?error, "Runtime worker failed after abort");
                            }
                        }
                        task.disarm();
                    }
                    return;
                }
            }
        }
        tracing::info!(worker, "Runtime worker task drain completed");
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
fn await_module_worker_shutdown(
    worker: &'static str,
    task: JoinHandle<()>,
) -> impl std::future::Future<Output = ()> {
    await_module_worker_shutdown_with_timeout(worker, task, Duration::from_secs(5))
}

#[cfg(any(
    test,
    feature = "mod-order",
    feature = "commerce-marketplace-financial",
    feature = "mod-payment",
    feature = "mod-product",
    feature = "mod-notifications",
    feature = "mod-forum"
))]
fn await_module_worker_shutdown_with_timeout(
    worker: &'static str,
    task: JoinHandle<()>,
    timeout_duration: Duration,
) -> impl std::future::Future<Output = ()> {
    let mut task = ShutdownJoinHandle::new(task);
    async move {
        match tokio::time::timeout(timeout_duration, task.handle_mut()).await {
            Ok(Ok(())) => {
                task.disarm();
                tracing::info!(worker, "Module worker stopped gracefully");
            }
            Ok(Err(error)) => {
                task.disarm();
                tracing::warn!(worker, ?error, "Module worker exited with error");
            }
            Err(_) => {
                tracing::warn!(worker, "Module worker shutdown timed out; aborting");
                task.abort();
                if let Err(error) = task.handle_mut().await {
                    if !error.is_cancelled() {
                        tracing::warn!(worker, ?error, "Module worker failed after abort");
                    }
                }
                task.disarm();
            }
        }
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

const AUTH_RETENTION_SUCCESS_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const AUTH_RETENTION_FAILURE_RETRY_INTERVAL: Duration = Duration::from_secs(15 * 60);

fn auth_retention_next_delay(failed: bool) -> Duration {
    if failed {
        AUTH_RETENTION_FAILURE_RETRY_INTERVAL
    } else {
        AUTH_RETENTION_SUCCESS_INTERVAL
    }
}

async fn auth_retention_loop(
    db: sea_orm::DatabaseConnection,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    // Successful cleanup runs daily, but an unavailable database is retried
    // sooner so a short outage cannot suspend auth artifact retention for a day.
    loop {
        if *stop_rx.borrow() {
            tracing::info!("Auth retention worker received shutdown signal, exiting");
            return;
        }

        let failed = match crate::services::auth_retention::purge_expired_auth_artifacts(&db).await {
            Ok(purge) if auth_retention_removed_artifacts(&purge) => {
                tracing::info!(
                    sessions = purge.sessions,
                    invite_consumptions = purge.invite_consumptions,
                    invites = purge.invites,
                    oauth_tokens = purge.oauth_tokens,
                    oauth_authorization_codes = purge.oauth_authorization_codes,
                    "Purged expired auth artifacts past retention"
                );
                false
            }
            Ok(_) => false,
            Err(_error) => {
                tracing::error!(
                    error_code = "auth_retention_purge_failed",
                    retry_delay_seconds = AUTH_RETENTION_FAILURE_RETRY_INTERVAL.as_secs(),
                    "Auth retention worker failed to purge expired artifacts; retrying before the next daily cycle"
                );
                true
            }
        };

        tokio::select! {
            _ = tokio::time::sleep(auth_retention_next_delay(failed)) => {}
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
            Err(_error) => tracing::error!(
                error_code = "remote_executor_reaper_failed",
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
        Err(_error) => {
            tracing::error!(
                error_code = "seo_worker_registry_initialization_failed",
                "Failed to initialize SEO bulk worker registry"
            );
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
                Err(_error) => tracing::error!(
                    error_code = "seo_bulk_job_execution_failed",
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
                Err(_error) => tracing::error!(
                    error_code = "seo_sitemap_job_execution_failed",
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
                Err(_error) => tracing::error!(
                    error_code = "seo_index_repair_job_execution_failed",
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

#[cfg(test)]
mod tests {
    use super::{
        OutboxRelayWorkerHandle, OutboxRetentionWorkerHandle, ServerBootstrapInitializationLock,
        connect_runtime_workers_with_runtime, shutdown_runtime_workers,
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

    #[test]
    fn auth_retention_failure_retries_without_waiting_a_full_day() {
        assert_eq!(super::auth_retention_next_delay(false), Duration::from_secs(86_400));
        assert_eq!(super::auth_retention_next_delay(true), Duration::from_secs(900));
        assert!(
            super::auth_retention_next_delay(true) < super::auth_retention_next_delay(false),
            "a transient storage failure must not use the daily success interval"
        );
    }

    #[tokio::test]
    async fn host_shutdown_waits_for_in_flight_auth_email_delivery() {
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel::<()>();
        drop(crate::services::email::spawn_auth_email_task(
            &ctx,
            "host_shutdown_email",
            move || async move {
                started_tx.send(()).expect("signal started");
                finish_rx.await.expect("finish email");
                Ok(())
            },
        ));
        started_rx.await.expect("task started");

        let shutdown_ctx = ctx.clone();
        let shutdown = tokio::spawn(async move {
            shutdown_runtime_workers(&shutdown_ctx).await;
        });
        let stop = super::StopHandle::ensure(&ctx);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !stop.is_stopping() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("shutdown signalled");
        assert!(!shutdown.is_finished(), "host should await the pending email");

        finish_tx.send(()).expect("release send");
        shutdown.await.expect("shutdown finishes after email");
    }

    #[test]
    fn direct_worker_shutdown_snapshot_waits_for_in_flight_registration() {
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let registration = super::DirectWorkerRegistrationLock::ensure(&ctx);
        let guard = registration
            .0
            .lock()
            .expect("hold direct worker registration gate");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (taken_tx, taken_rx) = std::sync::mpsc::channel();
        let shutdown_ctx = ctx.clone();
        let take = std::thread::spawn(move || {
            ready_tx.send(()).expect("taking thread ready");
            let result = super::take_direct_worker_for_shutdown::<u64>(&shutdown_ctx);
            taken_tx.send(result).expect("report shutdown snapshot");
        });

        ready_rx.recv().expect("shutdown taker entered thread");
        assert!(
            taken_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "shutdown must wait while a direct starter owns publication"
        );
        // A direct starter that entered before stop may publish; the shutdown
        // taker must then capture and remove that handle, not detach it.
        ctx.shared_insert(42_u64);
        drop(guard);

        assert_eq!(
            taken_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("shutdown acquires the gate"),
            Some(42)
        );
        take.join().expect("shutdown thread joins");
        assert!(!ctx.shared_contains::<u64>());
    }

    #[cfg(feature = "mod-comments")]
    #[tokio::test]
    async fn comments_direct_worker_handles_wait_for_publication_before_shutdown_take() {
        use crate::services::comments_provider_runtime::{
            CommentsTcpDelegationScheduleAuditHandoffWorkerHandle, CommentsTcpListenerTask,
        };

        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let registration = super::DirectWorkerRegistrationLock::ensure(&ctx);
        let guard = registration
            .0
            .lock()
            .expect("hold Comments final registration window");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (taken_tx, taken_rx) = std::sync::mpsc::channel();
        let shutdown_ctx = ctx.clone();
        let taker = std::thread::spawn(move || {
            ready_tx.send(()).expect("shutdown taker begins");
            let listener = super::take_direct_worker_for_shutdown::<CommentsTcpListenerTask>(
                &shutdown_ctx,
            );
            let handoff = super::take_direct_worker_for_shutdown::<
                CommentsTcpDelegationScheduleAuditHandoffWorkerHandle,
            >(&shutdown_ctx);
            taken_tx
                .send((listener, handoff))
                .expect("report both captured tasks");
        });

        ready_rx.recv().expect("shutdown taker thread started");
        assert!(
            taken_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "shutdown cannot take a Comments handle before publication finishes"
        );

        ctx.shared_insert(CommentsTcpListenerTask {
            task: tokio::spawn(std::future::pending()),
            shutdown_timeout: Duration::from_secs(1),
        });
        ctx.shared_insert(
            CommentsTcpDelegationScheduleAuditHandoffWorkerHandle::for_test(tokio::spawn(
                std::future::pending(),
            )),
        );
        drop(guard);

        let (listener, handoff) = taken_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown acquires both Comments task handles");
        taker.join().expect("shutdown taker thread completes");
        let listener = listener.expect("listener handle must be visible to shutdown");
        listener.task.abort();
        let _ = listener.task.await;
        let handoff = handoff.expect("handoff handle must be visible to shutdown");
        let handoff_task = handoff.take_task().expect("handoff task is retained");
        handoff_task.abort();
        let _ = handoff_task.await;
        assert!(!ctx.shared_contains::<CommentsTcpListenerTask>());
        assert!(!ctx.shared_contains::<CommentsTcpDelegationScheduleAuditHandoffWorkerHandle>());
    }

    #[tokio::test]
    async fn shutdown_waits_for_server_bootstrap_before_draining_workers() {
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let initialization = ServerBootstrapInitializationLock::ensure(&ctx);
        let guard = initialization.lock().await;
        let (finished_tx, mut finished_rx) = tokio::sync::oneshot::channel();
        let shutdown_ctx = ctx.clone();
        let shutdown = tokio::spawn(async move {
            shutdown_runtime_workers(&shutdown_ctx).await;
            let _ = finished_tx.send(());
        });

        tokio::task::yield_now().await;
        assert!(
            finished_rx.try_recv().is_err(),
            "shutdown must not take its worker snapshot during bootstrap"
        );

        drop(guard);
        tokio::time::timeout(Duration::from_secs(5), finished_rx)
            .await
            .expect("shutdown should continue after bootstrap releases its lock")
            .expect("shutdown completion signal");
        shutdown.await.expect("shutdown task joins");
    }

    #[tokio::test]
    async fn shutdown_drains_handles_published_by_an_in_flight_direct_worker_initializer() {
        // Direct calls to connect_runtime_workers_with_runtime use this lock
        // without taking the router bootstrap lock. Simulate one such caller
        // that already passed its stop check but has not yet published a handle.
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        let initialization = super::ModuleArtifactMaintenanceInitializationLock::ensure(&ctx);
        let guard = initialization.0.lock().await;

        let shutdown_ctx = ctx.clone();
        let shutdown = tokio::spawn(async move {
            shutdown_runtime_workers(&shutdown_ctx).await;
        });
        let stop = super::StopHandle::ensure(&ctx);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !stop.is_stopping() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("shutdown must broadcast the stop signal");

        assert!(
            !shutdown.is_finished(),
            "shutdown cannot snapshot handles until direct startup releases its lock"
        );

        // The old implementation could take its snapshot here, then lose this
        // newly registered handle forever. Use a trivially stoppable task so
        // the test does not spend the full five-second forced-abort deadline.
        let (task_finished_tx, task_finished_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = task_finished_tx.send(());
        });
        ctx.shared_insert(super::AuthRetentionWorkerHandle {
            instance_id: 1,
            _handle: task,
        });
        drop(guard);

        tokio::time::timeout(Duration::from_secs(5), shutdown)
            .await
            .expect("shutdown should drain after initializer exits")
            .expect("shutdown task should complete");
        task_finished_rx.await.expect("late task should have run");
        assert!(
            !ctx.shared_contains::<super::AuthRetentionWorkerHandle>(),
            "late-registered handle must be removed and joined by the drain"
        );
    }

    #[tokio::test]
    async fn background_worker_startup_fails_when_required_durable_storage_is_missing() {
        use crate::common::settings::RuntimeHostMode;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(db, settings);

        let error = connect_runtime_workers_with_runtime(ctx)
            .await
            .expect_err("worker host must not start without required durable storage");
        assert!(
            error
                .to_string()
                .contains("durable storage is not initialized")
        );
    }

    #[cfg(feature = "mod-workflow")]
    #[tokio::test]
    async fn workflow_cron_readiness_uses_the_bootstrap_requirement_marker() {
        use crate::common::settings::RuntimeHostMode;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        settings.runtime.background_workers.workflow_cron_enabled = true;
        let ctx = ServerRuntimeContext::new(db, settings);
        ctx.shared_insert(super::WorkflowCronSchedulerRequirement(false));

        let workflow = super::runtime_worker_snapshots(&ctx)
            .into_iter()
            .find(|worker| worker.name == "workflow_cron")
            .expect("workflow cron readiness snapshot must be registered");
        assert!(
            !workflow.required,
            "an intentionally disabled scheduler must not keep the worker host unready"
        );
    }

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
                let opt_in_consumer = matches!(
                    worker.name,
                    "product_index_refresh"
                        | "social_graph_index"
                        | "forum_search_inbox"
                        | "forum_search_contract_consumer"
                        | "notification_outbox_intake"
                        | "notification_fanout"
                        | "notification_candidate"
                        | "comments_tcp_listener"
                        | "comments_schedule_audit_handoff"
                        | "event_forwarder"
                );
                assert_eq!(
                    worker.required,
                    expected && !opt_in_consumer,
                    "{}",
                    worker.name
                );
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

    #[tokio::test]
    async fn shutdown_aborts_and_joins_rate_limit_cleanup_tasks() {
        use crate::middleware::rate_limit::RateLimitCleanupTaskRegistry;

        struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyDrop {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let notify_drop = NotifyDrop(Some(stopped_tx));
        let registry = RateLimitCleanupTaskRegistry::default();
        registry
            .with_open(|tasks| {
                tasks.push(tokio::spawn(async move {
                    let _notify_drop = notify_drop;
                    std::future::pending::<()>().await;
                }));
                Ok(())
            })
            .expect("register cleanup task");
        ctx.shared_insert(registry.clone());

        super::shutdown_runtime_workers(&ctx).await;

        stopped_rx
            .await
            .expect("cleanup worker must be dropped by shutdown");
        assert!(registry.with_open::<()>(|_| panic!("closed registry admitted work")).is_err());
        let retained = ctx.shared_get::<RateLimitCleanupTaskRegistry>().unwrap();
        assert!(retained.with_open::<()>(|_| panic!("shutdown replaced registry")).is_err());
        assert!(retained.close_and_take_tasks().is_empty());
    }

    #[tokio::test]
    async fn shutdown_before_rate_limit_initialization_leaves_a_closed_registry() {
        use crate::middleware::rate_limit::RateLimitCleanupTaskRegistry;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        assert!(!ctx.shared_contains::<RateLimitCleanupTaskRegistry>());

        super::shutdown_runtime_workers(&ctx).await;

        let registry = ctx.shared_get_or_insert_with(RateLimitCleanupTaskRegistry::default);
        assert!(registry.with_open::<()>(|_| panic!("late startup admitted work")).is_err());
    }

    #[tokio::test]
    async fn runtime_worker_drain_aborts_and_joins_after_timeout() {
        struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyDrop {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _notify_drop = NotifyDrop(Some(stopped_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx.await.expect("worker must start before timeout");

        super::await_runtime_worker_shutdown_with_timeout(
            "test runtime worker",
            Some(task),
            Duration::from_millis(1),
        )
        .await;

        stopped_rx
            .await
            .expect("timed-out worker must be aborted and joined");
    }

    #[cfg(feature = "mod-comments")]
    #[tokio::test]
    async fn shutdown_joins_comments_tcp_listener_after_broadcasting_stop() {
        use crate::services::comments_provider_runtime::{
            CommentsTcpListenerHandle, CommentsTcpListenerTask,
        };

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let mut stop = super::StopHandle::ensure(&ctx).subscribe();
        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            if stop.changed().await.is_ok() && *stop.borrow() {
                let _ = stopped_tx.send(());
            }
        });
        ctx.shared_insert(CommentsTcpListenerHandle {
            instance_id: 1,
            local_addr: std::net::SocketAddr::from(([127, 0, 0, 1], 9000)),
        });
        ctx.shared_insert(CommentsTcpListenerTask {
            task,
            shutdown_timeout: Duration::from_secs(6),
        });

        super::shutdown_runtime_workers(&ctx).await;

        stopped_rx.await.expect("listener must receive shutdown");
        assert!(!ctx.shared_contains::<CommentsTcpListenerHandle>());
        assert!(!ctx.shared_contains::<CommentsTcpListenerTask>());
    }

    #[tokio::test]
    async fn shutdown_aborts_supervisors_even_when_clone_handles_exist() {
        use crate::services::event_bus::EventForwarderHandle;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());

        let (forwarder, _abort_handles) = EventForwarderHandle::for_shutdown_test();
        let clone = forwarder.clone();
        ctx.shared_insert(forwarder);

        assert!(clone.is_running());
        super::shutdown_runtime_workers(&ctx).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!clone.is_running());
        assert!(!ctx.shared_contains::<EventForwarderHandle>());
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
        runtime_ctx.shared_insert(rustok_storage::StorageRuntime::in_memory());

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

        let snapshots = super::additional_runtime_worker_snapshots(&runtime_ctx);
        for name in ["outbox_relay", "outbox_retention"] {
            let snapshot = snapshots
                .iter()
                .find(|snapshot| snapshot.name == name)
                .expect("started Outbox worker must be visible to readiness");
            assert!(snapshot.required, "{name} must be required after startup");
            assert_eq!(snapshot.finished, Some(false), "{name} should be live");
        }

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
    async fn cancelled_shutdown_drain_aborts_its_owned_task() {
        struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyDrop {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _notify_drop = NotifyDrop(Some(stopped_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx.await.expect("worker must start");

        let mut drain = Box::pin(super::await_runtime_worker_shutdown(
            "cancelled test worker",
            Some(task),
        ));
        tokio::select! {
            _ = &mut drain => panic!("pending worker unexpectedly completed"),
            _ = tokio::time::sleep(Duration::from_millis(1)) => {}
        }
        drop(drain);

        stopped_rx
            .await
            .expect("dropping the drain must abort the owned worker");
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

    #[tokio::test]
    async fn module_worker_drain_aborts_and_joins_after_timeout() {
        struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyDrop {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _notify_drop = NotifyDrop(Some(stopped_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx.await.expect("module worker must start");

        super::await_module_worker_shutdown_with_timeout(
            "test module worker",
            task,
            Duration::from_millis(1),
        )
        .await;

        stopped_rx
            .await
            .expect("timed-out module worker must be aborted and joined");
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
        use crate::common::settings::RuntimeHostMode;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(db, settings);
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
        let forwarder_snapshot = super::additional_runtime_worker_snapshots(&ctx)
            .into_iter()
            .find(|snapshot| snapshot.name == "event_forwarder")
            .expect("started event forwarder must be visible to readiness");
        assert!(forwarder_snapshot.required);
        assert_eq!(forwarder_snapshot.finished, Some(false));
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

    #[cfg(feature = "mod-comments")]
    #[tokio::test]
    async fn comments_listener_and_audit_handoff_workers_are_required_when_started() {
        use crate::common::settings::RuntimeHostMode;
        use crate::services::comments_provider_runtime::{
            CommentsTcpDelegationScheduleAuditHandoffWorkerHandle, CommentsTcpListenerTask,
        };
        use super::additional_runtime_worker_snapshots;

        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            settings,
        );
        let stop = super::StopHandle::ensure(&ctx);

        let mut listener_stop = stop.subscribe();
        let listener_task = tokio::spawn(async move {
            let _ = listener_stop.changed().await;
        });
        ctx.shared_insert(CommentsTcpListenerTask {
            task: listener_task,
            shutdown_timeout: Duration::from_secs(6),
        });

        let mut handoff_stop = stop.subscribe();
        let handoff_task = tokio::spawn(async move {
            let _ = handoff_stop.changed().await;
        });
        ctx.shared_insert(CommentsTcpDelegationScheduleAuditHandoffWorkerHandle::for_test(
            handoff_task,
        ));

        let snapshots = additional_runtime_worker_snapshots(&ctx);
        for name in ["comments_tcp_listener", "comments_schedule_audit_handoff"] {
            let snapshot = snapshots
                .iter()
                .find(|snapshot| snapshot.name == name)
                .expect("started Comments worker must be visible to readiness");
            assert!(snapshot.required, "{name} must be required after startup");
            assert_eq!(snapshot.finished, Some(false), "{name} should be live");
        }

        super::shutdown_runtime_workers(&ctx).await;
        assert!(!ctx.shared_contains::<CommentsTcpListenerTask>());
        assert!(!ctx.shared_contains::<CommentsTcpDelegationScheduleAuditHandoffWorkerHandle>());
    }

    #[tokio::test]
    async fn shutdown_drains_outbox_relay_before_stopping_module_dispatcher() {
        use crate::services::module_event_dispatcher::ModuleEventDispatcherHandle;

        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );
        ctx.shared_insert(ModuleEventDispatcherHandle::for_test());

        let mut stop_rx = super::StopHandle::ensure(&ctx).subscribe();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel::<()>();
        let in_flight_relay = tokio::spawn(async move {
            stop_rx.changed().await.expect("shutdown signalled");
            started_tx.send(()).expect("relay started draining");
            finish_rx.await.expect("complete acknowledged delivery");
        });
        ctx.shared_insert(OutboxRelayWorkerHandle {
            instance_id: 1,
            _handle: in_flight_relay,
        });

        let shutdown_ctx = ctx.clone();
        let shutdown = tokio::spawn(async move {
            shutdown_runtime_workers(&shutdown_ctx).await;
        });
        tokio::time::timeout(Duration::from_secs(2), started_rx)
            .await
            .expect("relay receives shutdown")
            .expect("relay begins its in-flight drain");
        assert!(
            ctx.shared_contains::<ModuleEventDispatcherHandle>(),
            "the dispatcher must remain registered until the relay completes"
        );
        finish_tx.send(()).expect("release relay");
        tokio::time::timeout(Duration::from_secs(3), shutdown)
            .await
            .expect("shutdown finishes after relay")
            .expect("shutdown task joins");

        assert!(!ctx.shared_contains::<OutboxRelayWorkerHandle>());
        assert!(!ctx.shared_contains::<ModuleEventDispatcherHandle>());
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


#[cfg(test)]
mod installer_job_task_registry_tests {
    use super::{
        drain_installer_job_tasks, spawn_installer_job_task, InstallerJobTaskRegistry,
    };
    use std::time::Duration;

    #[tokio::test]
    async fn installer_job_tasks_are_drained_and_admission_closes() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let ctx = super::ServerRuntimeContext::new(
            db,
            crate::common::settings::RustokSettings::default(),
        );
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

        assert!(spawn_installer_job_task(&ctx, async move {
            let _ = started_tx.send(());
            let _ = release_rx.await;
        }));
        started_rx.await.expect("installer task started");

        let drain_ctx = ctx.clone();
        let drain = tokio::spawn(async move {
            drain_installer_job_tasks(&drain_ctx).await;
        });
        tokio::task::yield_now().await;
        assert!(!drain.is_finished(), "shutdown must join accepted installer work");

        release_tx.send(()).expect("release installer task");
        tokio::time::timeout(Duration::from_secs(2), drain)
            .await
            .expect("installer drain completes")
            .expect("installer drain task joins");

        assert!(
            !spawn_installer_job_task(&ctx, async {}),
            "shutdown must permanently close installer task admission"
        );
    }

    #[tokio::test]
    async fn stuck_installer_job_tasks_are_aborted_after_the_grace_timeout() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        struct DropFlag(Arc<AtomicBool>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let registry = InstallerJobTaskRegistry::default();
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = Arc::clone(&dropped);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();

        assert!(registry.spawn(async move {
            let _drop_flag = DropFlag(task_dropped);
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        }));
        started_rx.await.expect("installer task started");

        registry
            .drain_with_timeout(Duration::from_millis(5), Duration::from_secs(1))
            .await;

        assert!(
            dropped.load(Ordering::SeqCst),
            "aborting the timed-out JoinSet must drop the in-flight task"
        );
        assert!(!registry.spawn(async {}), "drained registry stays closed");
    }
}
