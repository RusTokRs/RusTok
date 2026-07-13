use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};

use crate::services::server_runtime_context::ServerRuntimeContext;

static PAID_ORDER_LABEL_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
const PAID_ORDER_LABEL_SWEEP_INTERVAL: Duration = Duration::from_secs(5);
const PAID_ORDER_LABEL_SWEEP_BATCH: u64 = 100;

pub struct PaidOrderCreateLabelWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl PaidOrderCreateLabelWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn spawn_paid_order_create_label_worker(
    runtime_ctx: ServerRuntimeContext,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> PaidOrderCreateLabelWorkerHandle {
    let instance_id = PAID_ORDER_LABEL_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let fulfillment_provider_registry = runtime_ctx
        .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
        .expect("FulfillmentProviderRegistry must be initialized before paid-order label worker");
    let service = rustok_commerce::PaidOrderCreateLabelSweepService::new(
        runtime_ctx.db_clone(),
        fulfillment_provider_registry,
    );

    tracing::info!(
        worker = "paid_order_create_label",
        instance_id,
        "Starting runtime worker"
    );
    PaidOrderCreateLabelWorkerHandle {
        instance_id,
        _handle: tokio::spawn(worker_loop(instance_id, service, stop_rx)),
    }
}

async fn worker_loop(
    instance_id: u64,
    service: rustok_commerce::PaidOrderCreateLabelSweepService,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut ticker = interval(PAID_ORDER_LABEL_SWEEP_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        if *stop_rx.borrow() {
            tracing::info!(
                worker = "paid_order_create_label",
                instance_id,
                "Runtime worker received shutdown signal"
            );
            return;
        }

        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(
                        worker = "paid_order_create_label",
                        instance_id,
                        "Runtime worker stopped"
                    );
                    return;
                }
            }
            _ = ticker.tick() => {
                match service.process_pending_once(PAID_ORDER_LABEL_SWEEP_BATCH).await {
                    Ok(report) => {
                        if report.examined > 0 {
                            tracing::info!(
                                worker = "paid_order_create_label",
                                instance_id,
                                examined = report.examined,
                                dispatched = report.dispatched,
                                skipped_unpaid = report.skipped_unpaid,
                                failed = report.failed,
                                "Paid-order label recovery sweep completed"
                            );
                        }
                    }
                    Err(error) => {
                        tracing::error!(
                            worker = "paid_order_create_label",
                            instance_id,
                            error = %error,
                            "Paid-order label recovery sweep failed"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_contract_is_bounded() {
        assert_eq!(PAID_ORDER_LABEL_SWEEP_BATCH, 100);
        assert!(PAID_ORDER_LABEL_SWEEP_INTERVAL >= Duration::from_secs(1));
    }
}
