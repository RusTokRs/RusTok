use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};

use crate::services::server_runtime_context::ServerRuntimeContext;

static MARKETPLACE_FINANCIAL_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
const MARKETPLACE_FINANCIAL_SWEEP_INTERVAL: Duration = Duration::from_secs(10);
const MARKETPLACE_FINANCIAL_SWEEP_BATCH: u64 = 100;

pub struct MarketplaceFinancialWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl MarketplaceFinancialWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn spawn_marketplace_financial_worker(
    runtime_ctx: ServerRuntimeContext,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> MarketplaceFinancialWorkerHandle {
    let instance_id = MARKETPLACE_FINANCIAL_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let financial_runtime = runtime_ctx
        .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
        .expect("MarketplaceFinancialRuntime must be initialized before financial recovery worker");
    let event_bus = runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .expect("TransactionalEventBus must be initialized before marketplace financial worker");
    let service = financial_runtime.paid_event_inbox(runtime_ctx.db_clone(), event_bus);

    tracing::info!(
        worker = "marketplace_financial_recovery",
        instance_id,
        "Starting runtime worker"
    );
    MarketplaceFinancialWorkerHandle {
        instance_id,
        _handle: tokio::spawn(worker_loop(instance_id, service, stop_rx)),
    }
}

async fn worker_loop(
    instance_id: u64,
    service: rustok_commerce::MarketplacePaidEventInboxService,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut ticker = interval(MARKETPLACE_FINANCIAL_SWEEP_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        if *stop_rx.borrow() {
            tracing::info!(
                worker = "marketplace_financial_recovery",
                instance_id,
                "Runtime worker received shutdown signal"
            );
            return;
        }

        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(
                        worker = "marketplace_financial_recovery",
                        instance_id,
                        "Runtime worker stopped"
                    );
                    return;
                }
            }
            _ = ticker.tick() => {
                match service.sweep(MARKETPLACE_FINANCIAL_SWEEP_BATCH).await {
                    Ok(report) => {
                        if report.selected > 0 {
                            tracing::info!(
                                worker = "marketplace_financial_recovery",
                                instance_id,
                                selected = report.selected,
                                processed = report.processed,
                                retryable_failures = report.retryable_failures,
                                operator_review_failures = report.operator_review_failures,
                                "Marketplace financial recovery sweep completed"
                            );
                        }
                    }
                    Err(error) => {
                        tracing::error!(
                            worker = "marketplace_financial_recovery",
                            instance_id,
                            error = %error,
                            "Marketplace financial recovery sweep failed"
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
        assert_eq!(MARKETPLACE_FINANCIAL_SWEEP_BATCH, 100);
        assert!(MARKETPLACE_FINANCIAL_SWEEP_INTERVAL >= Duration::from_secs(1));
    }
}
