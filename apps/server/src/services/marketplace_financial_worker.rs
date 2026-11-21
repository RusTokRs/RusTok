use std::future::Future;
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

    /// Await a durable worker's current operation after the shutdown signal.
    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }

    pub(crate) fn into_task(self) -> JoinHandle<()> {
        self._handle
    }
}

pub fn spawn_marketplace_financial_worker(
    runtime_ctx: ServerRuntimeContext,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> Option<MarketplaceFinancialWorkerHandle> {
    let instance_id = MARKETPLACE_FINANCIAL_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let Some(financial_runtime) =
        runtime_ctx.shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
    else {
        tracing::warn!(
            "MarketplaceFinancialRuntime not available; skipping financial recovery worker"
        );
        return None;
    };
    let event_bus = runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .unwrap_or_else(|| {
            crate::services::event_bus::transactional_event_bus_from_context(&runtime_ctx)
        });
    let db = runtime_ctx.db_clone();
    let paid_events = financial_runtime.paid_event_inbox(db.clone(), event_bus);
    let reversal_backfill = financial_runtime.provider_reversal_backfill(db.clone());
    let reversal_events = financial_runtime.reversal_event_inbox(db);

    tracing::info!(
        worker = "marketplace_financial_recovery",
        instance_id,
        "Starting runtime worker"
    );
    Some(MarketplaceFinancialWorkerHandle {
        instance_id,
        _handle: tokio::spawn(worker_loop(
            instance_id,
            paid_events,
            reversal_backfill,
            reversal_events,
            stop_rx,
        )),
    })
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
                // A durable sweep is not cancelled mid-operation, but a stop
                // signal must prevent admission of subsequent recovery phases.
                if !run_recovery_phases(
                    &stop_rx,
                    || adapt_reversal_events(instance_id, &reversal_backfill),
                    || sweep_reversal_events(instance_id, &reversal_events),
                    || sweep_paid_events(instance_id, &paid_events),
                )
                .await
                {
                    tracing::info!(
                        worker = "marketplace_financial_recovery",
                        instance_id,
                        "Worker stopped between durable recovery phases"
                    );
                    return;
                }
            }
        }
    }
}

/// Preserve the current durable operation after shutdown, but never start
/// another external recovery phase once the stop signal has been observed.
async fn run_recovery_phases<A, AF, R, RF, P, PF>(
    stop_rx: &tokio::sync::watch::Receiver<bool>,
    adapt: A,
    reversal: R,
    paid: P,
) -> bool
where
    A: FnOnce() -> AF,
    AF: Future<Output = ()>,
    R: FnOnce() -> RF,
    RF: Future<Output = ()>,
    P: FnOnce() -> PF,
    PF: Future<Output = ()>,
{
    if *stop_rx.borrow() {
        return false;
    }
    adapt().await;
    if *stop_rx.borrow() {
        return false;
    }
    reversal().await;
    if *stop_rx.borrow() {
        return false;
    }
    paid().await;
    !*stop_rx.borrow()
}

async fn adapt_reversal_events(
    instance_id: u64,
    backfill: &rustok_commerce::MarketplaceProviderReversalBackfillService,
) {
    match backfill
        .adapt_pending(MARKETPLACE_FINANCIAL_SWEEP_BATCH)
        .await
    {
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
        Err(_error) => {
            tracing::error!(
                worker = "marketplace_financial_recovery",
                instance_id,
                error_code = "marketplace_provider_reversal_adaptation_failed",
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
        Err(_error) => {
            tracing::error!(
                worker = "marketplace_financial_recovery",
                instance_id,
                error_code = "marketplace_reversal_recovery_sweep_failed",
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
        Err(_error) => {
            tracing::error!(
                worker = "marketplace_financial_recovery",
                instance_id,
                error_code = "marketplace_paid_event_recovery_sweep_failed",
                "Marketplace paid-event recovery sweep failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn simulated_recovery_cycle(stop_during: Option<u8>) -> (bool, Vec<u8>) {
        use std::sync::{Arc, Mutex};

        let (stop, stop_rx) = crate::services::stop_handle::StopHandle::new();
        let phases = Arc::new(Mutex::new(Vec::new()));
        if stop_during == Some(0) {
            stop.stop().await;
        }
        let make_phase = |phase: u8| {
            let stop = stop.clone();
            let phases = Arc::clone(&phases);
            move || async move {
                phases.lock().expect("phase recorder lock").push(phase);
                if stop_during == Some(phase) {
                    stop.stop().await;
                }
            }
        };
        let completed = run_recovery_phases(
            &stop_rx,
            make_phase(1),
            make_phase(2),
            make_phase(3),
        )
        .await;
        let recorded = phases.lock().expect("phase recorder lock").clone();
        (completed, recorded)
    }

    #[tokio::test]
    async fn recovery_finishes_current_operation_but_never_admits_next_after_stop() {
        for (stop_during, expected_phases, expected_completed) in [
            (Some(0), vec![], false),
            (Some(1), vec![1], false),
            (Some(2), vec![1, 2], false),
            (Some(3), vec![1, 2, 3], false),
            (None, vec![1, 2, 3], true),
        ] {
            let (completed, phases) = simulated_recovery_cycle(stop_during).await;
            assert_eq!(phases, expected_phases, "stop during phase {stop_during:?}");
            assert_eq!(
                completed, expected_completed,
                "cycle completion after stop during phase {stop_during:?}"
            );
        }
    }

    #[test]
    fn worker_contract_is_bounded() {
        assert_eq!(MARKETPLACE_FINANCIAL_SWEEP_BATCH, 100);
        assert!(MARKETPLACE_FINANCIAL_SWEEP_INTERVAL >= Duration::from_secs(1));
    }
}
