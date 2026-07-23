use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications::{
    DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE, NotificationCandidateWorker, NotificationError,
    NotificationRecipientPolicyRuntime,
};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

const CANDIDATE_POLL_INTERVAL: Duration = Duration::from_millis(500);
static NOTIFICATION_CANDIDATE_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

pub struct NotificationCandidateWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl NotificationCandidateWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn start_notification_candidate_worker_if_ready(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<NotificationCandidateWorkerHandle>()
    {
        return Ok(());
    }

    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| Error::Message("module runtime extensions are unavailable".to_string()))?;
    let Some(policy_runtime) = extensions
        .get::<NotificationRecipientPolicyRuntime>()
        .cloned()
    else {
        tracing::info!("Notification candidate worker disabled: recipient policy runtime is absent");
        return Ok(());
    };

    if !policy_runtime.candidate_worker_enabled() {
        tracing::info!("Notification candidate worker disabled by explicit runtime flag");
        return Ok(());
    }
    if !policy_runtime.relation_ports_ready() {
        tracing::warn!(
            "Notification candidate worker not started: recipient relation ports are not ready"
        );
        return Ok(());
    }
    if !policy_runtime.candidate_worker_ready() {
        return Ok(());
    }

    let registry = rustok_notifications::api::notification_source_registry_from_extensions(
        extensions.as_ref(),
    )
    .ok_or_else(|| {
        Error::Message("notification source registry is unavailable for candidate worker".to_string())
    })?;

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before notification candidate worker startup")
        .subscribe();

    let instance_id = NOTIFICATION_CANDIDATE_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let worker_id = format!("notification-candidate-{instance_id}");
    let worker = NotificationCandidateWorker::new(
        ctx.db_clone(),
        registry,
        policy_runtime.policy_arc(),
        worker_id,
        DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE,
    )
    .map_err(|error| Error::Message(format!("notification candidate worker is invalid: {error}")))?;

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        "Starting notification candidate worker"
    );
    ctx.shared_insert(NotificationCandidateWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_candidate_worker_loop(worker, stop_rx)),
    });
    Ok(())
}

async fn notification_candidate_worker_loop(
    worker: NotificationCandidateWorker,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(worker_id = worker.worker_id(), "Notification candidate worker stopped");
            return;
        }

        let item_ids = match worker.claimable_candidate_ids().await {
            Ok(item_ids) => item_ids,
            Err(error) => {
                tracing::error!(
                    worker_id = worker.worker_id(),
                    error = %error,
                    "Notification candidate worker failed to select claimable items"
                );
                Vec::new()
            }
        };

        for item_id in item_ids {
            // A shutdown signal prevents future claims. A candidate already being
            // processed is allowed to finish its lease/CAS completion path.
            if *stop_rx.borrow() {
                tracing::info!(worker_id = worker.worker_id(), "Notification candidate worker stopped before next claim");
                return;
            }

            match worker.process_candidate(item_id).await {
                Ok(result) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    item_id = %result.item_id,
                    status = ?result.status,
                    replayed = result.replayed,
                    "Notification candidate processed"
                ),
                Err(NotificationError::LeaseUnavailable) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    item_id = %item_id,
                    "Notification candidate claim lost to another worker"
                ),
                Err(error) => tracing::warn!(
                    worker_id = worker.worker_id(),
                    item_id = %item_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification candidate processing completed with durable failure state"
                ),
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(CANDIDATE_POLL_INTERVAL) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(worker_id = worker.worker_id(), "Notification candidate worker received shutdown signal");
                    return;
                }
            }
        }
    }
}
