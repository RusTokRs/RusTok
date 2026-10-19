//! Restart-safe delivery of Product-owned committed Media removals.
use crate::services::server_runtime_context::ServerRuntimeContext;
use std::time::Duration;
use tokio::{
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

pub struct ProductMediaReleaseWorkerHandle {
    _task: JoinHandle<()>,
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
                tokio::select! {
                    // Cancellation cannot lose an obligation: a release whose result
                    // was not acknowledged still has its committed queue row for replay.
                    changed = stop.changed() => {if changed.is_err() || *stop.borrow() {break;}},
                    result = runtime.process_media_releases(100) => {
                        match result {
                            Ok(report) => {if report.examined > 0 {
                                tracing::info!(examined = report.examined, released = report.released, failed = report.failed,
                                    "Product Media removal delivery completed");
                            }},
                            Err(error) => tracing::error!(%error, "Product Media removal delivery failed"),
                        }
                    }
                }
            }
        }),
    }
}
