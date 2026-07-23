use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications::{
    DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE, DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE,
    NotificationError, NotificationFanoutWorker,
};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const NOTIFICATION_FANOUT_WORKER_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED";
const FANOUT_POLL_INTERVAL: Duration = Duration::from_millis(500);
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
    let registry = rustok_notifications::api::notification_source_registry_from_extensions(
        extensions.as_ref(),
    )
    .ok_or_else(|| {
        Error::Message("notification source registry is unavailable for fanout worker".to_string())
    })?;
    if registry.is_empty() {
        tracing::warn!(
            "Notification fanout worker not started: materialized source registry is empty"
        );
        return Ok(());
    }

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
        registry,
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
        _handle: tokio::spawn(notification_fanout_worker_loop(worker, stop_rx)),
    });
    Ok(())
}

async fn notification_fanout_worker_loop(
    worker: NotificationFanoutWorker,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(worker_id = worker.worker_id(), "Notification fanout worker stopped");
            return;
        }

        let source_ids = match worker.claimable_source_inbox_ids().await {
            Ok(source_ids) => source_ids,
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
        for inbox_id in source_ids {
            if *stop_rx.borrow() {
                tracing::info!(
                    worker_id = worker.worker_id(),
                    "Notification fanout worker stopped before next source claim"
                );
                return;
            }
            match worker.materialize_source_inbox(inbox_id).await {
                Ok(receipt) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    inbox_id = %receipt.inbox_id,
                    status = ?receipt.status,
                    fanout_job_id = ?receipt.fanout_job_id,
                    replayed = receipt.replayed,
                    "Notification source inbox record materialized"
                ),
                Err(NotificationError::LeaseUnavailable) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    inbox_id = %inbox_id,
                    "Notification source inbox lease lost to another worker"
                ),
                Err(error) => tracing::warn!(
                    worker_id = worker.worker_id(),
                    inbox_id = %inbox_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification source materialization completed with durable failure state"
                ),
            }
        }

        let job_ids = match worker.claimable_fanout_job_ids().await {
            Ok(job_ids) => job_ids,
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
        for job_id in job_ids {
            if *stop_rx.borrow() {
                tracing::info!(
                    worker_id = worker.worker_id(),
                    "Notification fanout worker stopped before next job claim"
                );
                return;
            }
            match worker.process_fanout_job(job_id).await {
                Ok(page) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    job_id = %page.job_id,
                    candidates = page.candidates,
                    inserted_items = page.inserted_items,
                    completed = page.completed,
                    "Notification fanout page processed"
                ),
                Err(NotificationError::LeaseUnavailable) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    job_id = %job_id,
                    "Notification fanout job lease lost to another worker"
                ),
                Err(error) => tracing::warn!(
                    worker_id = worker.worker_id(),
                    job_id = %job_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification fanout page completed with durable failure state"
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
