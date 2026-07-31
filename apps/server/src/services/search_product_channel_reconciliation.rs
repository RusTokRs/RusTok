use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_search::{
    DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT, ProductChannelProjectionReconciler,
};
use tokio::task::JoinHandle;

use crate::error::Result;
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

const PRODUCT_CHANNEL_REPAIR_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const PRODUCT_CHANNEL_REPAIR_BATCH_INTERVAL: Duration = Duration::from_millis(100);
static PRODUCT_CHANNEL_REPAIR_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

pub struct ProductChannelProjectionWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl ProductChannelProjectionWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn start_product_channel_projection_reconciliation_if_ready(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<ProductChannelProjectionWorkerHandle>()
    {
        return Ok(());
    }

    let reconciler = ProductChannelProjectionReconciler::new(ctx.db_clone());
    if !reconciler.supports_background_reconciliation() {
        tracing::info!(
            "Product Search channel projection reconciliation not started: PostgreSQL backend is required"
        );
        return Ok(());
    }

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before Product channel reconciliation startup")
        .subscribe();

    let instance_id =
        PRODUCT_CHANNEL_REPAIR_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        instance_id,
        tenant_limit = DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT,
        "Starting Product Search channel projection reconciliation"
    );
    ctx.shared_insert(ProductChannelProjectionWorkerHandle {
        instance_id,
        _handle: tokio::spawn(product_channel_projection_reconciliation_loop(
            reconciler, stop_rx,
        )),
    });
    Ok(())
}

async fn product_channel_projection_reconciliation_loop(
    reconciler: ProductChannelProjectionReconciler,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!("Product Search channel projection reconciliation stopped");
            return;
        }

        let wait = match reconciler
            .sweep_due(DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT)
            .await
        {
            Ok(report) if report.due_tenants == 0 => {
                tracing::info!(
                    "Product Search channel projection reconciliation completed"
                );
                return;
            }
            Ok(report) => {
                tracing::info!(
                    due_tenants = report.due_tenants,
                    rebuilt_tenants = report.rebuilt_tenants,
                    "Product Search channel projection reconciliation batch completed"
                );
                PRODUCT_CHANNEL_REPAIR_BATCH_INTERVAL
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Product Search channel projection reconciliation failed"
                );
                PRODUCT_CHANNEL_REPAIR_RETRY_INTERVAL
            }
        };

        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(
                        "Product Search channel projection reconciliation received shutdown signal"
                    );
                    return;
                }
            }
        }
    }
}
