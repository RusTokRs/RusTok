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
    let db = runtime_ctx.db_clone();
    let paid_events = financial_runtime.paid_event_inbox(db.clone(), event_bus);
    let reversal_backfill = financial_runtime.provider_reversal_backfill(db.clone());
    let reversal_events = financial_runtime.reversal_event_inbox(db);

    tracing::info!(
        worker = "marketplace_financial_recovery",
        instance_id,
        "Starting runtime worker"
    );
    MarketplaceFinancialWorkerHandle {
        instance_id,
        _handle: tokio::spawn(worker_loop(
            instance_id,
            paid_events,
            reversal_backfill,
            reversal_events,
            stop_rx,
        )),
    }
}

async fn worker_loop(
    instance_id: u64,
    paid_events: rustok_commerce::MarketplacePaidEventInboxService,
    reversal_backfill: rustok_commerce::MarketplaceProviderReversalBackfillService,
    reversal_events: rustok_commerce::MarketplaceReversalEventInboxService,
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
                adapt_reversal_events(instance_id, &reversal_backfill).await;
                sweep_reversal_events(instance_id, &reversal_events).await;
                sweep_paid_events(instance_id, &paid_events).await;
            }
        }
    }
}

async fn adapt_reversal_events(
    instance_id: u64,
    backfill: &rustok_commerce::MarketplaceProviderReversalBackfillService,
) {
    match backfill.adapt_pending(MARKETPLACE_FINANCIAL_SWEEP_BATCH).await {
        Ok(report) if report.selected > 0 => {
            tracing::info!(
                worker = "marketplace_financial_recovery",
                instance_id,
                selected = report.selected,
                adapted = report.adapted,
                ignored = report.ignored,
                failed = report.failed,
                "Marketplace provider reversal adaptation completed"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::error!(
                worker = "marketplace_financial_recovery",
                instance_id,
                error = %error,
                "Marketplace provider reversal adaptation failed"
            );
        }
    }
}

async fn sweep_reversal_events(
    instance_id: u64,
    service: &rustok_commerce::MarketplaceReversalEventInboxService,
) {
    match service.sweep(MARKETPLACE_FINANCIAL_SWEEP_BATCH).await {
        Ok(report) if report.selected > 0 => {
            tracing::info!(
                worker = "marketplace_financial_recovery",
                instance_id,
                selected = report.selected,
                processed = report.processed,
                retryable_failures = report.retryable_failures,
                operator_review_failures = report.operator_review_failures,
                "Marketplace reversal recovery sweep completed"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::error!(
                worker = "marketplace_financial_recovery",
                instance_id,
                error = %error,
                "Marketplace reversal recovery sweep failed"
            );
        }
    }
}

async fn sweep_paid_events(
    instance_id: u64,
    service: &rustok_commerce::MarketplacePaidEventInboxService,
) {
    match service.sweep(MARKETPLACE_FINANCIAL_SWEEP_BATCH).await {
        Ok(report) if report.selected > 0 => {
            tracing::info!(
                worker = "marketplace_financial_recovery",
                instance_id,
                selected = report.selected,
                processed = report.processed,
                retryable_failures = report.retryable_failures,
                operator_review_failures = report.operator_review_failures,
                "Marketplace paid-event recovery sweep completed"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::error!(
                worker = "marketplace_financial_recovery",
                instance_id,
                error = %error,
                "Marketplace paid-event recovery sweep failed"
            );
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
