use loco_rs::{app::AppContext, config::Config};

use crate::error::{Error, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::common::settings::RustokSettings;
use crate::services::build_executor::BuildExecutionService;
use crate::services::event_transport_factory::{
    spawn_outbox_relay_worker, EventRuntime, RelayRuntimeConfig,
};
use crate::services::registry_governance::RegistryGovernanceService;
use crate::services::registry_validation_runner::RegistryValidationRunnerService;
use crate::services::release_backend::ReleaseDeploymentService;

// ── Graceful-shutdown handle ──────────────────────────────────────────────────

/// Stored in `ctx.shared_store`; `on_shutdown` calls `stop()` to abort workers.
#[derive(Clone)]
pub struct StopHandle {
    stop_tx: tokio::sync::watch::Sender<bool>,
}

impl StopHandle {
    pub fn new() -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (Self { stop_tx: tx }, rx)
    }

    pub async fn stop(&self) {
        let _ = self.stop_tx.send(true);
        // Yield so spawned tasks have a chance to notice the signal.
        tokio::task::yield_now().await;
    }
}

static OUTBOX_RELAY_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static BUILD_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static REGISTRY_VALIDATION_STAGE_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
static REGISTRY_REMOTE_EXECUTOR_REAPER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

const LOCAL_SQLITE_DATABASE_URI: &str = "sqlite://rustok.sqlite?mode=rwc";
const REGISTRY_REMOTE_EXECUTOR_REAPER_ACTOR: &str = "system:registry-remote-executor-reaper";

pub struct OutboxRelayWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl OutboxRelayWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
}

pub struct BuildWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl BuildWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
}

pub struct RegistryValidationStageWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl RegistryValidationStageWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
}

pub struct RegistryRemoteExecutorReaperWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl RegistryRemoteExecutorReaperWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
}

pub fn apply_boot_database_fallback(config: &mut Config) -> bool {
    if should_use_local_sqlite_fallback(
        std::env::var("DATABASE_URL").is_ok(),
        config.database.uri.as_str(),
    ) {
        config.database.uri = LOCAL_SQLITE_DATABASE_URI.to_string();
        return true;
    }

    false
}

pub async fn connect_runtime_workers(ctx: &AppContext) -> Result<()> {
    let settings = RustokSettings::from_settings(&ctx.config.settings)
        .map_err(|error| Error::Message(format!("Invalid rustok settings: {error}")))?;

    if settings.runtime.is_registry_only() {
        tracing::info!("Skipping background workers for registry-only host mode");
        return Ok(());
    }

    // Register graceful-shutdown handle if not already present.
    if !ctx.shared_store.contains::<StopHandle>() {
        let (handle, _rx) = StopHandle::new();
        ctx.shared_store.insert(handle);
    }

    if ctx.shared_store.contains::<OutboxRelayWorkerHandle>() {
        // Keep going: build worker may still need to be attached.
    } else {
        let event_runtime = ctx
            .shared_store
            .get::<std::sync::Arc<EventRuntime>>()
            .ok_or_else(|| Error::Message("EventRuntime not initialized".to_string()))?;

        if let Some(relay_config) = event_runtime.relay_config.clone() {
            ctx.shared_store
                .insert(spawn_relay_worker_handle(relay_config));
        }
    }

    if settings.build.enabled && !ctx.shared_store.contains::<BuildWorkerHandle>() {
        ctx.shared_store.insert(spawn_build_worker_handle(
            ctx.clone(),
            settings.build.clone(),
        ));
    }

    if settings.registry.validation_runner.enabled
        && !ctx
            .shared_store
            .contains::<RegistryValidationStageWorkerHandle>()
    {
        ctx.shared_store
            .insert(spawn_registry_validation_stage_worker_handle(
                ctx.clone(),
                settings.registry.validation_runner.clone(),
            ));
    }

    if settings.registry.remote_executor.enabled
        && !ctx
            .shared_store
            .contains::<RegistryRemoteExecutorReaperWorkerHandle>()
    {
        ctx.shared_store
            .insert(spawn_registry_remote_executor_reaper_worker_handle(
                ctx.clone(),
                settings.registry.remote_executor.clone(),
            ));
    }

    Ok(())
}

fn spawn_relay_worker_handle(relay_config: RelayRuntimeConfig) -> OutboxRelayWorkerHandle {
    OutboxRelayWorkerHandle {
        instance_id: OUTBOX_RELAY_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed),
        _handle: spawn_outbox_relay_worker(relay_config),
    }
}

fn spawn_build_worker_handle(
    ctx: AppContext,
    config: crate::common::settings::BuildRuntimeSettings,
) -> BuildWorkerHandle {
    BuildWorkerHandle {
        instance_id: BUILD_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed),
        _handle: tokio::spawn(build_worker_loop(ctx, config)),
    }
}

fn spawn_registry_validation_stage_worker_handle(
    ctx: AppContext,
    config: crate::common::settings::RegistryValidationRunnerSettings,
) -> RegistryValidationStageWorkerHandle {
    RegistryValidationStageWorkerHandle {
        instance_id: REGISTRY_VALIDATION_STAGE_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed),
        _handle: tokio::spawn(registry_validation_stage_worker_loop(ctx, config)),
    }
}

fn spawn_registry_remote_executor_reaper_worker_handle(
    ctx: AppContext,
    config: crate::common::settings::RegistryRemoteExecutorSettings,
) -> RegistryRemoteExecutorReaperWorkerHandle {
    RegistryRemoteExecutorReaperWorkerHandle {
        instance_id: REGISTRY_REMOTE_EXECUTOR_REAPER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed),
        _handle: tokio::spawn(registry_remote_executor_reaper_worker_loop(ctx, config)),
    }
}

async fn build_worker_loop(ctx: AppContext, config: crate::common::settings::BuildRuntimeSettings) {
    let executor = BuildExecutionService::new(&ctx);
    let release_backend = ReleaseDeploymentService::new(&ctx, config.clone());
    let poll_interval = Duration::from_millis(config.poll_interval_ms);

    loop {
        match executor.execute_next_queued_build(false).await {
            Ok(Some(report)) => {
                tracing::info!(
                    build_id = %report.build_id,
                    cargo_command = %report.cargo_command,
                    "Executed queued build plan"
                );

                if report.status == "success" {
                    if let Some(environment) = config.auto_release_environment.as_deref() {
                        match executor
                            .ensure_release_for_build(report.build_id, environment, false)
                            .await
                        {
                            Ok(release) => match release_backend
                                .publish_release(&release.id, config.auto_activate_release)
                                .await
                            {
                                Ok(published_release) => tracing::info!(
                                    build_id = %report.build_id,
                                    release_id = %published_release.id,
                                    release_status = ?published_release.status,
                                    "Published release from successful build"
                                ),
                                Err(error) => tracing::error!(
                                    build_id = %report.build_id,
                                    release_id = %release.id,
                                    error = %error,
                                    "Failed to publish release from successful build"
                                ),
                            },
                            Err(error) => tracing::error!(
                                build_id = %report.build_id,
                                error = %error,
                                "Failed to create release record from successful build"
                            ),
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(error = %error, "Background build worker failed to execute queued build");
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

async fn registry_validation_stage_worker_loop(
    ctx: AppContext,
    config: crate::common::settings::RegistryValidationRunnerSettings,
) {
    let runner = RegistryValidationRunnerService::new(ctx.db.clone(), config.clone());
    let poll_interval = Duration::from_millis(config.poll_interval_ms);

    loop {
        match runner.execute_next_queued_stage().await {
            Ok(Some(report)) => tracing::info!(
                request_id = %report.request_id,
                slug = %report.slug,
                stage_key = %report.stage_key,
                status = %report.status,
                "Executed queued registry validation stage"
            ),
            Ok(None) => {}
            Err(error) => tracing::error!(
                error = %error,
                actor = %config.actor,
                "Background registry validation stage runner failed"
            ),
        }

        tokio::time::sleep(poll_interval).await;
    }
}

async fn registry_remote_executor_reaper_worker_loop(
    ctx: AppContext,
    config: crate::common::settings::RegistryRemoteExecutorSettings,
) {
    let governance = RegistryGovernanceService::new(ctx.db.clone());
    let poll_interval = Duration::from_millis(config.requeue_scan_interval_ms);

    loop {
        match governance
            .requeue_expired_validation_stage_claims(REGISTRY_REMOTE_EXECUTOR_REAPER_ACTOR)
            .await
        {
            Ok(requeued) if requeued > 0 => tracing::info!(
                requeued,
                lease_ttl_ms = config.lease_ttl_ms,
                "Requeued expired remote validation stage claims"
            ),
            Ok(_) => {}
            Err(error) => tracing::error!(
                error = %error,
                lease_ttl_ms = config.lease_ttl_ms,
                "Background remote executor lease reaper failed"
            ),
        }

        tokio::time::sleep(poll_interval).await;
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
        connect_runtime_workers, should_use_local_sqlite_fallback, BuildWorkerHandle,
        OutboxRelayWorkerHandle, RegistryRemoteExecutorReaperWorkerHandle,
    };
    use loco_rs::tests_cfg::app::get_app_context;
    use rustok_core::events::MemoryTransport;
    use rustok_outbox::{OutboxRelay, OutboxTransport};
    use std::{sync::Arc, time::Duration};

    use crate::services::event_transport_factory::{EventRuntime, RelayRuntimeConfig};

    #[test]
    fn uses_sqlite_fallback_when_database_url_is_missing_and_uri_is_empty() {
        assert!(should_use_local_sqlite_fallback(false, ""));
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
    async fn connect_runtime_workers_is_idempotent_for_outbox_relay_handle() {
        let ctx = get_app_context().await;
        let relay_config = RelayRuntimeConfig {
            interval: Duration::from_secs(60),
            relay: OutboxRelay::new(ctx.db.clone(), Arc::new(MemoryTransport::new())),
        };
        let runtime = Arc::new(EventRuntime {
            transport: Arc::new(OutboxTransport::new(ctx.db.clone())),
            relay_config: Some(relay_config),
            channel_capacity: 128,
            relay_fallback_active: false,
        });
        ctx.shared_store.insert(runtime);

        connect_runtime_workers(&ctx)
            .await
            .expect("first worker connect should succeed");
        let first_instance_id = ctx
            .shared_store
            .get_ref::<OutboxRelayWorkerHandle>()
            .expect("relay handle should be stored")
            .instance_id();

        connect_runtime_workers(&ctx)
            .await
            .expect("second worker connect should be idempotent");
        let second_instance_id = ctx
            .shared_store
            .get_ref::<OutboxRelayWorkerHandle>()
            .expect("relay handle should still be stored")
            .instance_id();

        assert_eq!(first_instance_id, second_instance_id);
    }

    #[tokio::test]
    async fn connect_runtime_workers_is_idempotent_for_build_worker_handle() {
        let mut ctx = get_app_context().await;
        ctx.config.settings = Some(serde_json::json!({
            "rustok": {
                "build": {
                    "enabled": true,
                    "poll_interval_ms": 1000
                }
            }
        }));
        let runtime = Arc::new(EventRuntime {
            transport: Arc::new(OutboxTransport::new(ctx.db.clone())),
            relay_config: None,
            channel_capacity: 128,
            relay_fallback_active: false,
        });
        ctx.shared_store.insert(runtime);

        connect_runtime_workers(&ctx)
            .await
            .expect("first worker connect should succeed");
        let first_instance_id = ctx
            .shared_store
            .get_ref::<BuildWorkerHandle>()
            .expect("build worker handle should be stored")
            .instance_id();

        connect_runtime_workers(&ctx)
            .await
            .expect("second worker connect should be idempotent");
        let second_instance_id = ctx
            .shared_store
            .get_ref::<BuildWorkerHandle>()
            .expect("build worker handle should still be stored")
            .instance_id();

        assert_eq!(first_instance_id, second_instance_id);
    }

    #[tokio::test]
    async fn connect_runtime_workers_is_idempotent_for_remote_executor_reaper_handle() {
        let mut ctx = get_app_context().await;
        ctx.config.settings = Some(serde_json::json!({
            "rustok": {
                "registry": {
                    "remote_executor": {
                        "enabled": true,
                        "shared_token": "test-runner-token",
                        "lease_ttl_ms": 30000,
                        "requeue_scan_interval_ms": 1000
                    }
                }
            }
        }));
        let runtime = Arc::new(EventRuntime {
            transport: Arc::new(OutboxTransport::new(ctx.db.clone())),
            relay_config: None,
            channel_capacity: 128,
            relay_fallback_active: false,
        });
        ctx.shared_store.insert(runtime);

        connect_runtime_workers(&ctx)
            .await
            .expect("first worker connect should succeed");
        let first_instance_id = ctx
            .shared_store
            .get_ref::<RegistryRemoteExecutorReaperWorkerHandle>()
            .expect("remote executor reaper handle should be stored")
            .instance_id();

        connect_runtime_workers(&ctx)
            .await
            .expect("second worker connect should be idempotent");
        let second_instance_id = ctx
            .shared_store
            .get_ref::<RegistryRemoteExecutorReaperWorkerHandle>()
            .expect("remote executor reaper handle should still be stored")
            .instance_id();

        assert_eq!(first_instance_id, second_instance_id);
    }
}
