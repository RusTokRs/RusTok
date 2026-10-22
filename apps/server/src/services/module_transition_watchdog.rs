use sea_orm::DatabaseConnection;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info};

use rustok_modules::evaluate_transition_watchdog;

static MODULE_TRANSITION_WATCHDOG_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

pub struct ModuleTransitionWatchdogHandle {
    pub instance_id: u64,
    pub join_handle: JoinHandle<()>,
}

impl ModuleTransitionWatchdogHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self.join_handle.is_finished()
    }

    pub fn abort(&self) {
        self.join_handle.abort();
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self.join_handle.await
    }
}

pub fn spawn_module_transition_watchdog_handle(
    db: DatabaseConnection,
    poll_interval_ms: u64,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) -> ModuleTransitionWatchdogHandle {
    let instance_id = MODULE_TRANSITION_WATCHDOG_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let min_interval_ms = if cfg!(test) { 5 } else { 1_000 };
    let interval = Duration::from_millis(poll_interval_ms.max(min_interval_ms));
    // The security epoch is durable (loaded from the database by each
    // evaluation), so no process-local registry is kept here.

    info!(
        worker = "module_transition_watchdog",
        instance_id, poll_interval_ms, "Starting module transition watchdog background worker"
    );

    let join_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if *stop_rx.borrow() {
                        info!(
                            worker = "module_transition_watchdog",
                            instance_id,
                            "Module transition watchdog received shutdown signal"
                        );
                        break;
                    }

                    tracing::debug!(
                        worker = "module_transition_watchdog",
                        instance_id,
                        "Evaluating transition watchdog checkpoints"
                    );

                    match evaluate_transition_watchdog(&db).await {
                        Ok(updated) if !updated.is_empty() => {
                            for checkpoint in updated {
                                info!(
                                    worker = "module_transition_watchdog",
                                    instance_id,
                                    operation_id = %checkpoint.operation_id,
                                    module_slug = %checkpoint.module_slug,
                                    state = %checkpoint.state.name(),
                                    "Transition watchdog advanced checkpoint state"
                                );
                            }
                        }
                        Ok(_) => {}
                        Err(err) => {
                            error!(
                                worker = "module_transition_watchdog",
                                instance_id,
                                error = %err,
                                "Transition watchdog evaluation failed"
                            );
                        }
                    }
                }
                changed = stop_rx.changed() => {
                    if changed.is_err() || *stop_rx.borrow() {
                        info!(
                            worker = "module_transition_watchdog",
                            instance_id,
                            "Module transition watchdog stopping"
                        );
                        break;
                    }
                }
            }
        }
    });

    ModuleTransitionWatchdogHandle {
        instance_id,
        join_handle,
    }
}

#[cfg(test)]
mod tests {
    use super::spawn_module_transition_watchdog_handle;
    use sea_orm::Database;
    use std::time::Duration;

    #[tokio::test]
    async fn watchdog_handle_instance_id_monotonic() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let (_stop_tx1, stop_rx1) = tokio::sync::watch::channel(false);
        let (_stop_tx2, stop_rx2) = tokio::sync::watch::channel(false);

        let handle1 = spawn_module_transition_watchdog_handle(db.clone(), 5, stop_rx1);
        let handle2 = spawn_module_transition_watchdog_handle(db, 5, stop_rx2);

        assert!(handle2.instance_id() > handle1.instance_id());

        handle1.abort();
        handle2.abort();
    }

    #[tokio::test]
    async fn watchdog_handle_abort_terminates_task() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);

        let handle = spawn_module_transition_watchdog_handle(db, 5, stop_rx);
        assert!(!handle.is_finished());

        handle.abort();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(handle.is_finished());
    }

    #[tokio::test]
    async fn watchdog_stops_on_shutdown_signal() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

        let handle = spawn_module_transition_watchdog_handle(db, 5, stop_rx);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!handle.is_finished());

        stop_tx.send(true).expect("shutdown signal sent");
        tokio::time::timeout(Duration::from_secs(5), handle.join())
            .await
            .expect("watchdog exits cleanly within timeout")
            .expect("watchdog task completed");
    }
}
