use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_notifications::{
    DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, NotificationError,
    NotificationOutboxIntakeWorker,
};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED";
const OUTBOX_INTAKE_POLL_INTERVAL: Duration = Duration::from_millis(500);
static NOTIFICATION_OUTBOX_INTAKE_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

pub struct NotificationOutboxIntakeWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl NotificationOutboxIntakeWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn start_notification_outbox_intake_if_enabled(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<NotificationOutboxIntakeWorkerHandle>()
    {
        return Ok(());
    }
    if !outbox_intake_enabled_from_environment() {
        tracing::info!(
            variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
            "Notification outbox intake disabled by explicit runtime flag"
        );
        return Ok(());
    }

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before notification outbox intake startup")
        .subscribe();

    let instance_id = NOTIFICATION_OUTBOX_INTAKE_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let worker = NotificationOutboxIntakeWorker::new(
        ctx.db_clone(),
        DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE,
    )
    .map_err(|error| Error::Message(format!("notification outbox intake is invalid: {error}")))?;

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        "Starting notification outbox intake worker"
    );
    ctx.shared_insert(NotificationOutboxIntakeWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_outbox_intake_loop(worker, stop_rx)),
    });
    Ok(())
}

async fn notification_outbox_intake_loop(
    worker: NotificationOutboxIntakeWorker,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!("Notification outbox intake stopped");
            return;
        }

        let event_ids = match worker.pending_outbox_event_ids().await {
            Ok(event_ids) => event_ids,
            Err(error) => {
                tracing::error!(
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification outbox intake failed to select dispatched envelopes"
                );
                Vec::new()
            }
        };

        for outbox_event_id in event_ids {
            if *stop_rx.borrow() {
                tracing::info!("Notification outbox intake stopped before next envelope");
                return;
            }

            match worker.process_outbox_event(outbox_event_id).await {
                Ok(result) => tracing::debug!(
                    outbox_event_id = %result.outbox_event_id,
                    source_inbox_id = %result.source_inbox_id,
                    source_slug = result.source_slug,
                    event_type = result.event_type,
                    source_revision = result.source_revision,
                    replayed = result.replayed,
                    "Notification source envelope accepted from outbox"
                ),
                Err(NotificationError::SourceIdentityConflict) => tracing::error!(
                    outbox_event_id = %outbox_event_id,
                    "Notification outbox intake rejected a conflicting semantic replay"
                ),
                Err(error) => tracing::warn!(
                    outbox_event_id = %outbox_event_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification outbox intake did not create a receipt; envelope will be retried"
                ),
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(OUTBOX_INTAKE_POLL_INTERVAL) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Notification outbox intake received shutdown signal");
                    return;
                }
            }
        }
    }
}

fn outbox_intake_enabled_from_environment() -> bool {
    match std::env::var(NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "" | "0" | "false" | "no" | "off" => false,
            _ => {
                tracing::warn!(
                    variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
                    value,
                    "Invalid notification outbox intake flag; keeping intake disabled"
                );
                false
            }
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(error) => {
            tracing::warn!(
                variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
                error = %error,
                "Notification outbox intake flag is unreadable; keeping intake disabled"
            );
            false
        }
    }
}
