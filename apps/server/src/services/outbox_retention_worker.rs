use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_outbox::OutboxRetention;
use tokio::task::JoinHandle;

use crate::services::stop_handle::may_admit_next_worker_operation;

static OUTBOX_RETENTION_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);
const OUTBOX_RETENTION_FAILURE_RETRY_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
enum PruneCycle {
    Complete,
    BatchExhausted,
    Failed,
}

fn next_prune_delay(interval: Duration, cycle: PruneCycle) -> Duration {
    match cycle {
        PruneCycle::Complete => interval,
        PruneCycle::BatchExhausted => Duration::from_millis(50).min(interval),
        PruneCycle::Failed => OUTBOX_RETENTION_FAILURE_RETRY_DELAY.min(interval),
    }
}

/// Failure counter of the retention supervisor.
///
/// A prune failure is not fatal: an unsuccessful sweep retries after at most
/// one minute, while successful sweeps follow the configured interval and
/// unfinished batches continue promptly. Operators alert on the counter
/// instead of on a silent no-op pruner.
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
    tracing::debug!(
        interval_secs = config.interval.as_secs(),
        "Spawning outbox retention worker"
    );
    tokio::spawn(supervise_outbox_retention(config, stop_rx))
}

async fn supervise_outbox_retention(
    config: OutboxRetentionRuntimeConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if !may_admit_next_worker_operation(&stop_rx) {
            tracing::info!("Outbox retention worker received shutdown signal, exiting");
            return;
        }

        let cycle = match config.retention.prune_once().await {
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
                if report.batch_exhausted {
                    PruneCycle::BatchExhausted
                } else {
                    PruneCycle::Complete
                }
            }
            Err(_error) => {
                OUTBOX_RETENTION_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
                tracing::error!(
                    error_code = "outbox_retention_prune_failed",
                    "Outbox retention prune failed"
                );
                PruneCycle::Failed
            }
        };

        let delay = next_prune_delay(config.interval, cycle);

        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
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
                    ..Default::default()
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

    #[test]
    fn failure_uses_bounded_retry_without_changing_success_or_batch_cadence() {
        let daily = Duration::from_secs(86_400);
        assert_eq!(
            super::next_prune_delay(daily, super::PruneCycle::Complete),
            daily
        );
        assert_eq!(
            super::next_prune_delay(daily, super::PruneCycle::Failed),
            Duration::from_secs(60),
        );
        assert_eq!(
            super::next_prune_delay(daily, super::PruneCycle::BatchExhausted),
            Duration::from_millis(50),
        );
        let short_interval = Duration::from_secs(10);
        assert_eq!(
            super::next_prune_delay(short_interval, super::PruneCycle::Failed),
            short_interval,
        );
    }

    #[test]
    fn metrics_snapshot_defaults_and_equality() {
        let snapshot1 = super::OutboxRetentionSupervisorMetricsSnapshot::default();
        let snapshot2 = super::OutboxRetentionSupervisorMetricsSnapshot { failure_total: 0 };
        assert_eq!(snapshot1, snapshot2);
        assert_eq!(snapshot1.failure_total, 0);
    }
}
