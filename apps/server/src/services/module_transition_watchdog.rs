use sea_orm::DatabaseConnection;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info};

use rustok_modules::{SecurityEpochRegistry, evaluate_transition_watchdog};

static MODULE_TRANSITION_WATCHDOG_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

pub struct ModuleTransitionWatchdogHandle {
    pub instance_id: u64,
    pub join_handle: JoinHandle<()>,
}

pub fn spawn_module_transition_watchdog_handle(
    db: DatabaseConnection,
    poll_interval_ms: u64,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) -> ModuleTransitionWatchdogHandle {
    let instance_id = MODULE_TRANSITION_WATCHDOG_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let interval = Duration::from_millis(poll_interval_ms.max(1_000));
    let security_registry = SecurityEpochRegistry::new();

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

                    match evaluate_transition_watchdog(&db, &security_registry).await {
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
