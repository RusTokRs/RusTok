use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_outbox::OutboxRetention;
use tokio::task::JoinHandle;

static OUTBOX_RETENTION_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Failure counter of the retention supervisor.
///
/// A prune failure is not fatal: the next interval retries, and a delivered row
/// that survives one run is pruned by a later one. Operators alert on the
/// counter instead of on a silent no-op pruner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutboxRetentionSupervisorMetricsSnapshot {
    pub failure_total: u64,
}

pub fn outbox_retention_supervisor_metrics_snapshot() -> OutboxRetentionSupervisorMetricsSnapshot {
    OutboxRetentionSupervisorMetricsSnapshot {
        failure_total: OUTBOX_RETENTION_FAILURE_TOTAL.load(Ordering::Relaxed),
    }
}

/// Runtime shape of the retention worker: how often to prune and the bounded
/// window/batch the owner applies.
#[derive(Clone)]
pub struct OutboxRetentionRuntimeConfig {
    pub interval: Duration,
    pub retention: OutboxRetention,
}

pub fn spawn_outbox_retention_worker(
    config: OutboxRetentionRuntimeConfig,
    stop_rx: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(supervise_outbox_retention(config, stop_rx))
}

async fn supervise_outbox_retention(
    config: OutboxRetentionRuntimeConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!("Outbox retention worker received shutdown signal, exiting");
            return;
        }

        let interval = config.interval;
        match config.retention.prune_once().await {
            Ok(report) => {
                if report.pruned > 0 || report.batch_exhausted {
                    tracing::info!(
                        pruned_events = report.pruned,
                        batch_exhausted = report.batch_exhausted,
                        "Outbox retention prune run completed"
                    );
                } else {
                    tracing::debug!("Outbox retention prune run found nothing to prune");
                }
            }
            Err(error) => {
                OUTBOX_RETENTION_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
                tracing::error!(error = %error, "Outbox retention prune failed");
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Outbox retention worker received shutdown signal, exiting");
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OutboxRetentionRuntimeConfig, outbox_retention_supervisor_metrics_snapshot,
        spawn_outbox_retention_worker,
    };
    use rustok_outbox::{OutboxRetention, OutboxRetentionConfig};
    use sea_orm::{ConnectionTrait, Database};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use std::time::Duration;

    async fn test_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("sys_events schema");
        db
    }

    #[tokio::test]
    async fn supervisor_stops_on_the_shutdown_signal() {
        let db = test_db().await;
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let handle = spawn_outbox_retention_worker(
            OutboxRetentionRuntimeConfig {
                interval: Duration::from_millis(5),
                retention: OutboxRetention::new(db).with_config(OutboxRetentionConfig {
                    retention: chrono::Duration::days(30),
                    batch_size: 10,
                }),
            },
            stop_rx,
        );

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!handle.is_finished());

        stop_tx.send(true).expect("shutdown signal");
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("supervisor exits")
            .expect("supervisor task joins");
    }

    #[tokio::test]
    async fn supervisor_counts_a_failed_prune_run() {
        let db = test_db().await;
        db.execute_unprepared("DROP TABLE sys_events")
            .await
            .expect("drop table to force a prune failure");
        let failures_before = outbox_retention_supervisor_metrics_snapshot().failure_total;
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let handle = spawn_outbox_retention_worker(
            OutboxRetentionRuntimeConfig {
                interval: Duration::from_millis(5),
                retention: OutboxRetention::new(db),
            },
            stop_rx,
        );

        for _ in 0..200 {
            if outbox_retention_supervisor_metrics_snapshot().failure_total > failures_before {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            outbox_retention_supervisor_metrics_snapshot().failure_total > failures_before,
            "a failed prune run must be observable"
        );
        handle.abort();
    }
}
