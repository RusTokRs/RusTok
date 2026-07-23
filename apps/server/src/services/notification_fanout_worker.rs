use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
use rustok_notifications::{
    DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE, DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE,
    NotificationError, NotificationFanoutWorker,
};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const NOTIFICATION_FANOUT_WORKER_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED";
const FANOUT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const NOTIFICATIONS_MODULE_SLUG: &str = "notifications";
static NOTIFICATION_FANOUT_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

pub struct NotificationFanoutWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl NotificationFanoutWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn start_notification_fanout_worker_if_ready(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<NotificationFanoutWorkerHandle>()
    {
        return Ok(());
    }
    if !fanout_worker_enabled_from_environment() {
        tracing::info!(
            variable = NOTIFICATION_FANOUT_WORKER_ENABLED_ENV,
            "Notification fanout worker disabled by explicit runtime flag"
        );
        return Ok(());
    }

    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| Error::Message("module runtime extensions are unavailable".to_string()))?;
    let source_registry = rustok_notifications::api::notification_source_registry_from_extensions(
        extensions.as_ref(),
    )
    .ok_or_else(|| {
        Error::Message("notification source registry is unavailable for fanout worker".to_string())
    })?;
    if source_registry.is_empty() {
        tracing::warn!(
            "Notification fanout worker not started: materialized source registry is empty"
        );
        return Ok(());
    }
    let module_registry = ctx
        .shared_get::<ModuleRegistry>()
        .ok_or_else(|| Error::Message("module registry is unavailable".to_string()))?;

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before notification fanout worker startup")
        .subscribe();

    let instance_id = NOTIFICATION_FANOUT_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let worker_id = format!("notification-fanout-{instance_id}");
    let worker = NotificationFanoutWorker::new(
        ctx.db_clone(),
        source_registry,
        worker_id,
        DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE,
        DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE,
    )
    .map_err(|error| Error::Message(format!("notification fanout worker is invalid: {error}")))?;

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        page_size = worker.page_size(),
        "Starting notification fanout worker"
    );
    ctx.shared_insert(NotificationFanoutWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_fanout_worker_loop(
            worker,
            ctx.db_clone(),
            module_registry,
            stop_rx,
        )),
    });
    Ok(())
}

async fn notification_fanout_worker_loop(
    worker: NotificationFanoutWorker,
    db: DatabaseConnection,
    module_registry: ModuleRegistry,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(worker_id = worker.worker_id(), "Notification fanout worker stopped");
            return;
        }

        let source_work = match worker.claimable_source_inbox_work().await {
            Ok(source_work) => source_work,
            Err(error) => {
                tracing::error!(
                    worker_id = worker.worker_id(),
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification fanout worker failed to select source inbox records"
                );
                Vec::new()
            }
        };
        for work in source_work {
            if *stop_rx.borrow() {
                tracing::info!(
                    worker_id = worker.worker_id(),
                    "Notification fanout worker stopped before next source claim"
                );
                return;
            }
            if !tenant_notifications_enabled(&db, &module_registry, work.tenant_id).await {
                continue;
            }
            match worker.materialize_source_inbox(work.inbox_id).await {
                Ok(receipt) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    inbox_id = %receipt.inbox_id,
                    status = ?receipt.status,
                    fanout_job_id = ?receipt.fanout_job_id,
                    replayed = receipt.replayed,
                    "Notification source inbox record materialized"
                ),
                Err(NotificationError::LeaseUnavailable) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    inbox_id = %work.inbox_id,
                    "Notification source inbox lease lost to another worker"
                ),
                Err(error) => tracing::warn!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    inbox_id = %work.inbox_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification source materialization returned a durable service error"
                ),
            }
        }

        let job_work = match worker.claimable_fanout_job_work().await {
            Ok(job_work) => job_work,
            Err(error) => {
                tracing::error!(
                    worker_id = worker.worker_id(),
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification fanout worker failed to select fanout jobs"
                );
                Vec::new()
            }
        };
        for work in job_work {
            if *stop_rx.borrow() {
                tracing::info!(
                    worker_id = worker.worker_id(),
                    "Notification fanout worker stopped before next job claim"
                );
                return;
            }
            if !tenant_notifications_enabled(&db, &module_registry, work.tenant_id).await {
                continue;
            }
            match worker.process_fanout_job(work.job_id).await {
                Ok(page) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    job_id = %page.job_id,
                    candidates = page.candidates,
                    inserted_items = page.inserted_items,
                    completed = page.completed,
                    "Notification fanout page processed"
                ),
                Err(NotificationError::LeaseUnavailable) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    job_id = %work.job_id,
                    "Notification fanout job lease lost to another worker"
                ),
                Err(error) => tracing::warn!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    job_id = %work.job_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification fanout page returned a durable service error"
                ),
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(FANOUT_POLL_INTERVAL) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(worker_id = worker.worker_id(), "Notification fanout worker received shutdown signal");
                    return;
                }
            }
        }
    }
}

async fn tenant_notifications_enabled(
    db: &DatabaseConnection,
    module_registry: &ModuleRegistry,
    tenant_id: uuid::Uuid,
) -> bool {
    match EffectiveModulePolicyService::is_enabled(
        db,
        module_registry,
        tenant_id,
        NOTIFICATIONS_MODULE_SLUG,
    )
    .await
    {
        Ok(true) => true,
        Ok(false) => {
            tracing::debug!(
                tenant_id = %tenant_id,
                module_slug = NOTIFICATIONS_MODULE_SLUG,
                "Notification fanout skipped because tenant capability is disabled"
            );
            false
        }
        Err(error) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                module_slug = NOTIFICATIONS_MODULE_SLUG,
                error = %error,
                "Notification fanout policy lookup failed closed"
            );
            false
        }
    }
}

fn fanout_worker_enabled_from_environment() -> bool {
    match std::env::var(NOTIFICATION_FANOUT_WORKER_ENABLED_ENV) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "" | "0" | "false" | "no" | "off" => false,
            _ => {
                tracing::warn!(
                    variable = NOTIFICATION_FANOUT_WORKER_ENABLED_ENV,
                    value,
                    "Invalid notification fanout worker flag; keeping worker disabled"
                );
                false
            }
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(error) => {
            tracing::warn!(
                variable = NOTIFICATION_FANOUT_WORKER_ENABLED_ENV,
                error = %error,
                "Notification fanout worker flag is unreadable; keeping worker disabled"
            );
            false
        }
    }
}
