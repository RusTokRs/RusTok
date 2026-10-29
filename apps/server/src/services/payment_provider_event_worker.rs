use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use rustok_tenant::entities::tenant;

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

    /// Await a durable worker's current operation after the shutdown signal.
    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }
}

pub fn spawn_payment_provider_event_worker(
    runtime_ctx: ServerRuntimeContext,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> PaymentProviderEventWorkerHandle {
    let instance_id = PAYMENT_PROVIDER_EVENT_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let db = runtime_ctx.db_clone();
    let observers = runtime_ctx
        .shared_get::<rustok_payment::PaymentProviderEventObservers>()
        .unwrap_or_default();
    let recovery = rustok_payment::PaymentProviderEventRecoveryService::new(
        db.clone(),
        Arc::new(rustok_payment::PaymentObservedDomainEventApplier::new(
            db.clone(),
            observers,
        )),
    );
    let tenants = db;

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
    tenants: DatabaseConnection,
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
                sweep_tenants(instance_id, &tenants, &recovery, &stop_rx).await;
            }
        }
    }
}

/// External payment effects must be reconciled even after a tenant is disabled.
/// Tenant deactivation blocks user traffic, but it must not strand an already
/// verified provider event or leave captured/authorized state unresolved.
async fn sweep_tenants(
    instance_id: u64,
    tenants: &DatabaseConnection,
    recovery: &rustok_payment::PaymentProviderEventRecoveryService,
    stop_rx: &tokio::sync::watch::Receiver<bool>,
) {
    let mut after_tenant_id = None;
    loop {
        if *stop_rx.borrow() {
            // Finish the current durable recovery call, then stop before
            // selecting more tenants or starting new provider work.
            return;
        }
        let items = match next_recovery_tenant_batch(tenants, after_tenant_id).await {
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

        // Capture the cursor before consuming the batch; deletion of an earlier
        // tenant cannot shift future pages as it did under offset pagination.
        let batch_size = items.len();
        let last_tenant_id = items.last().expect("nonempty recovery batch").id;
        for tenant in items {
            if *stop_rx.borrow() {
                return;
            }
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

        after_tenant_id = Some(last_tenant_id);
        if batch_size < PAYMENT_PROVIDER_EVENT_TENANT_PAGE as usize {
            return;
        }
    }
}

fn recovery_tenant_query(after: Option<uuid::Uuid>) -> sea_orm::Select<tenant::Entity> {
    let mut query = tenant::Entity::find().order_by_asc(tenant::Column::Id);
    if let Some(id) = after {
        query = query.filter(tenant::Column::Id.gt(id));
    }
    query.limit(PAYMENT_PROVIDER_EVENT_TENANT_PAGE)
}

async fn next_recovery_tenant_batch(
    db: &DatabaseConnection,
    after: Option<uuid::Uuid>,
) -> Result<Vec<tenant::Model>, sea_orm::DbErr> {
    recovery_tenant_query(after).all(db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_tenant_scan_has_stable_keyset_cursor_and_bounded_batch() {
        use sea_orm::{DbBackend, QueryTrait};
        let cursor = uuid::Uuid::from_u128(42);
        let sql = super::recovery_tenant_query(Some(cursor))
            .build(DbBackend::Sqlite)
            .to_string();
        assert!(sql.contains("ORDER BY"), "{sql}");
        assert!(sql.contains("id"), "{sql}");
        assert!(sql.contains(" > "), "{sql}");
        assert!(sql.contains("LIMIT"), "{sql}");
    }

    #[tokio::test]
    async fn recovery_tenant_scan_does_not_skip_after_previous_tenant_is_deleted() {
        use sea_orm::{ActiveModelTrait, Database, EntityTrait, Set};
        use sea_orm_migration::{MigrationTrait, SchemaManager};

        let db = Database::connect("sqlite::memory:").await.expect("sqlite");
        rustok_migrations::SqliteTestMigrator::up(&db, None)
            .await
            .expect("test tenant schema");

        for i in 1..=3_u128 {
            let mut tenant = tenant::ActiveModel::new(&format!("Tenant {i}"), &format!("worker-tenant-{i}"));
            tenant.id = Set(uuid::Uuid::from_u128(i));
            tenant.insert(&db).await.expect("tenant inserted");
        }

        let first = super::next_recovery_tenant_batch(&db, None)
            .await
            .expect("load recovery page");
        assert_eq!(first.len(), 3);
        let last_seen = first[0].id;
        tenant::Entity::delete_by_id(last_seen)
            .exec(&db)
            .await
            .expect("delete preceding tenant");

        let remaining = super::next_recovery_tenant_batch(&db, Some(last_seen))
            .await
            .expect("resume after deleted cursor");
        assert_eq!(
            remaining.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![uuid::Uuid::from_u128(2), uuid::Uuid::from_u128(3)]
        );
    }

    #[test]
    fn worker_contract_is_bounded() {
        assert_eq!(PAYMENT_PROVIDER_EVENT_SWEEP_BATCH, 50);
        assert_eq!(PAYMENT_PROVIDER_EVENT_TENANT_PAGE, 100);
        assert!(PAYMENT_PROVIDER_EVENT_SWEEP_INTERVAL >= Duration::from_secs(1));
    }
}
