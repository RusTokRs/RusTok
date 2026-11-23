//! Restart-safe delivery of Product-owned committed Media removals.
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::stop_handle::run_admitted_worker_operation;
use std::time::Duration;
use tokio::{
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

pub struct ProductMediaReleaseWorkerHandle {
    _task: JoinHandle<()>,
}

impl ProductMediaReleaseWorkerHandle {
    pub fn is_finished(&self) -> bool {
        self._task.is_finished()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._task.await
    }

    pub(crate) fn into_task(self) -> JoinHandle<()> {
        self._task
    }
}
pub fn spawn_product_media_release_worker(
    ctx: ServerRuntimeContext,
    mut stop: tokio::sync::watch::Receiver<bool>,
) -> ProductMediaReleaseWorkerHandle {
    ProductMediaReleaseWorkerHandle {
        _task: tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                if *stop.borrow() {
                    break;
                }
                tokio::select! {
                    changed = stop.changed() => {if changed.is_err() || *stop.borrow() {break;} continue;},
                    _ = ticker.tick() => {},
                }
                // Read one owner snapshot for the entire batch. Its CatalogService
                // binds Product persistence and Media authority together; shared host
                // values may be replaced independently during composition.
                let Some(runtime) =
                    ctx.shared_get::<rustok_product::ProductCatalogCommandRuntime>()
                else {
                    continue;
                };
                if runtime.profile() != rustok_product::ProductCatalogCommandProfile::EmbeddedNative
                    || runtime.media_reference_port().is_none()
                {
                    continue;
                }
                // Finish a batch that already passed the stop-admission check.
                // Cancelling here could drop the owner future after an external
                // Media release but before confirmation/queue acknowledgement.
                // Process loss or a bounded forced abort still requires durable replay.
                match run_admitted_worker_operation(&stop, || runtime.process_media_releases(100))
                    .await
                {
                    None => break,
                    Some(Ok(report)) if report.examined > 0 => {
                        tracing::info!(
                            examined = report.examined,
                            released = report.released,
                            failed = report.failed,
                            "Product Media removal delivery completed"
                        );
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_error)) => {
                        tracing::error!(
                            error_code = "product_media_release_delivery_failed",
                            "Product Media removal delivery failed"
                        );
                    }
                }
            }
        }),
    }
}
