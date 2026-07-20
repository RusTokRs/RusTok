use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};

use crate::services::server_runtime_context::ServerRuntimeContext;

static PAYMENT_PROVIDER_EVENT_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);
const PAYMENT_PROVIDER_EVENT_SWEEP_INTERVAL: Duration = Duration::from_secs(10);
const PAYMENT_PROVIDER_EVENT_SWEEP_BATCH: u64 = 50;
const PAYMENT_PROVIDER_EVENT_TENANT_PAGE: u64 = 100;

pub struct PaymentProviderEventWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl PaymentProviderEventWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn spawn_payment_provider_event_worker(
    runtime_ctx: ServerRuntimeContext,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> PaymentProviderEventWorkerHandle {
    let instance_id = PAYMENT_PROVIDER_EVENT_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let db = runtime_ctx.db_clone();
    let recovery = rustok_payment::PaymentProviderEventRecoveryService::new(
        db.clone(),
        Arc::new(rustok_payment::PaymentDomainEventApplier::new(db.clone())),
    );
    let tenants = rustok_tenant::TenantService::new(db);

    tracing::info!(
        worker = "payment_provider_event_recovery",
        instance_id,
        "Starting runtime worker"
    );
    PaymentProviderEventWorkerHandle {
        instance_id,
        _handle: tokio::spawn(worker_loop(instance_id, tenants, recovery, stop_rx)),
    }
}

async fn worker_loop(
    instance_id: u64,
    tenants: rustok_tenant::TenantService,
    recovery: rustok_payment::PaymentProviderEventRecoveryService,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut ticker = interval(PAYMENT_PROVIDER_EVENT_SWEEP_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        if *stop_rx.borrow() {
            tracing::info!(
                worker = "payment_provider_event_recovery",
                instance_id,
                "Runtime worker received shutdown signal"
            );
            return;
        }

        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(
                        worker = "payment_provider_event_recovery",
                        instance_id,
                        "Runtime worker stopped"
                    );
                    return;
                }
            }
            _ = ticker.tick() => {
                sweep_tenants(instance_id, &tenants, &recovery).await;
            }
        }
    }
}

/// External payment effects must be reconciled even after a tenant is disabled.
/// Tenant deactivation blocks user traffic, but it must not strand an already
/// verified provider event or leave captured/authorized state unresolved.
async fn sweep_tenants(
    instance_id: u64,
    tenants: &rustok_tenant::TenantService,
    recovery: &rustok_payment::PaymentProviderEventRecoveryService,
) {
    let mut page = 1;
    loop {
        let (items, total) = match tenants
            .list_tenants(page, PAYMENT_PROVIDER_EVENT_TENANT_PAGE)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(
                    worker = "payment_provider_event_recovery",
                    instance_id,
                    error = %error,
                    "Failed to list tenants for payment provider event recovery"
                );
                return;
            }
        };
        if items.is_empty() {
            return;
        }

        for tenant in items {
            let worker_id = format!(
                "payment-provider-event-worker:{instance_id}:tenant:{}",
                tenant.id
            );
            match recovery
                .run(
                    tenant.id,
                    worker_id.as_str(),
                    Some(PAYMENT_PROVIDER_EVENT_SWEEP_BATCH),
                )
                .await
            {
                Ok(report) if report.scanned > 0 => {
                    tracing::info!(
                        worker = "payment_provider_event_recovery",
                        instance_id,
                        tenant_id = %tenant.id,
                        tenant_active = tenant.is_active,
                        scanned = report.scanned,
                        processed = report.processed,
                        retryable = report.retryable,
                        dead_letter = report.dead_letter,
                        in_progress = report.in_progress,
                        errors = report.errors,
                        "Payment provider event recovery sweep completed"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(
                        worker = "payment_provider_event_recovery",
                        instance_id,
                        tenant_id = %tenant.id,
                        tenant_active = tenant.is_active,
                        error = %error,
                        "Payment provider event recovery sweep failed"
                    );
                }
            }
        }

        if page.saturating_mul(PAYMENT_PROVIDER_EVENT_TENANT_PAGE) >= total {
            return;
        }
        page = page.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_contract_is_bounded() {
        assert_eq!(PAYMENT_PROVIDER_EVENT_SWEEP_BATCH, 50);
        assert_eq!(PAYMENT_PROVIDER_EVENT_TENANT_PAGE, 100);
        assert!(PAYMENT_PROVIDER_EVENT_SWEEP_INTERVAL >= Duration::from_secs(1));
    }
}
