use std::str::FromStr;
use std::sync::Arc;

use cron::Schedule;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

use crate::entities::{WorkflowEntity, WorkflowStatus, workflow};
use crate::services::{WorkflowEngine, WorkflowService};

/// Polls active workflows with cron triggers and fires them on schedule.
///
/// Each workflow with `{"type": "cron", "expression": "0 * * * * *"}` trigger
/// is checked every second against its cron expression.
pub struct WorkflowCronScheduler {
    db: DatabaseConnection,
    engine: Arc<WorkflowEngine>,
    service: Arc<WorkflowService>,
}

impl WorkflowCronScheduler {
    pub fn new(db: DatabaseConnection) -> Self {
        let engine = Arc::new(WorkflowEngine::new(db.clone()));
        let service = Arc::new(WorkflowService::new(db.clone()));
        Self {
            db,
            engine,
            service,
        }
    }

    /// Start the scheduler without a host shutdown signal.
    /// Aborting this handle also drops and aborts its owned executions; callers
    /// with a lifecycle signal should prefer `start_with_stop`.
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            info!("WorkflowCronScheduler started");
            self.run(None).await;
        })
    }

    /// Start with host-owned graceful shutdown. Stop admitting new triggers,
    /// then wait for already-started execution tasks before the scheduler exits.
    /// The host may still enforce a bounded forced-abort deadline.
    pub fn start_with_stop(self, stop_rx: watch::Receiver<bool>) -> JoinHandle<()> {
        tokio::spawn(async move {
            info!("WorkflowCronScheduler started with host shutdown supervision");
            self.run(Some(stop_rx)).await;
        })
    }

    async fn run(&self, mut stop_rx: Option<watch::Receiver<bool>>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut executions = JoinSet::new();

        loop {
            // Reap every completed execution so panics are observable and the
            // JoinSet does not grow without bound across normal cron ticks.
            while let Some(result) = executions.try_join_next() {
                observe_cron_execution_result(result);
            }
            if stop_rx.as_ref().is_some_and(|rx| *rx.borrow()) {
                break;
            }

            if let Some(stop) = stop_rx.as_mut() {
                tokio::select! {
                    biased;
                    changed = stop.changed() => {
                        if changed.is_err() || *stop.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        if *stop.borrow() {
                            break;
                        }
                        if let Err(error) = self.tick(&mut executions, Some(stop)).await {
                            error!(%error, "CronScheduler tick error");
                        }
                    }
                }
            } else {
                interval.tick().await;
                if let Err(error) = self.tick(&mut executions, None).await {
                    error!(%error, "CronScheduler tick error");
                }
            }
        }

        // A graceful stop is a fence on new work, not cancellation of accepted
        // HTTP/notification/step effects. Join the owned executions so their
        // database terminal states can be persisted before host shutdown.
        drain_cron_executions(&mut executions).await;
        info!("WorkflowCronScheduler stopped");
    }

    async fn tick(
        &self,
        executions: &mut JoinSet<()>,
        stop_rx: Option<&watch::Receiver<bool>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = chrono::Utc::now();

        let workflows = WorkflowEntity::find()
            .filter(workflow::Column::Status.eq(WorkflowStatus::Active.to_string()))
            .all(&self.db)
            .await?;

        for wf in workflows {
            // Tenant discovery may have awaited the database across shutdown.
            // Do not begin a new execution after that happens.
            if stop_rx.is_some_and(|rx| *rx.borrow()) {
                break;
            }
            let trigger = &wf.trigger_config;
            if trigger.get("type").and_then(|v| v.as_str()) != Some("cron") {
                continue;
            }

            let expr = match trigger.get("expression").and_then(|v| v.as_str()) {
                Some(e) => e,
                None => {
                    warn!(workflow_id = %wf.id, "Cron trigger missing 'expression'");
                    continue;
                }
            };

            let schedule = match Schedule::from_str(expr) {
                Ok(s) => s,
                Err(e) => {
                    warn!(workflow_id = %wf.id, expression = expr, error = %e, "Invalid cron expression");
                    continue;
                }
            };

            // Check if cron fires at this second
            let prev = now - chrono::Duration::seconds(1);
            let next_after_prev = schedule.after(&prev).next();

            let should_fire = next_after_prev.map(|t| t <= now).unwrap_or(false);

            if !should_fire {
                continue;
            }

            let workflow_id = wf.id;
            let tenant_id = wf.tenant_id;

            info!(workflow_id = %workflow_id, expression = expr, "Cron trigger fired");

            let steps = match self.service.load_steps(workflow_id).await {
                Ok(s) => s,
                Err(e) => {
                    error!(workflow_id = %workflow_id, error = %e, "Failed to load steps for cron");
                    continue;
                }
            };

            // Loading the steps is preparation: a stop during that await
            // must not admit a new workflow execution.
            if stop_rx.is_some_and(|rx| *rx.borrow()) {
                break;
            }
            let engine = self.engine.clone();
            let ctx = json!({ "trigger": { "type": "cron", "expression": expr } });

            executions.spawn(async move {
                if let Err(e) = engine
                    .execute(workflow_id, tenant_id, None, steps, ctx)
                    .await
                {
                    error!(workflow_id = %workflow_id, error = %e, "Cron workflow execution failed");
                }
            });
        }

        Ok(())
    }
}

/// Observe failures even when an execution panics instead of returning an
/// ordinary WorkflowResult. A completed task may be reaped on the next tick.
fn observe_cron_execution_result(result: Result<(), tokio::task::JoinError>) {
    if let Err(error) = result {
        if !error.is_cancelled() {
            error!(?error, "Cron workflow execution task failed");
        }
    }
}

async fn drain_cron_executions(executions: &mut JoinSet<()>) {
    while let Some(result) = executions.join_next().await {
        observe_cron_execution_result(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn graceful_cron_drain_waits_for_accepted_work() {
        let mut executions = JoinSet::new();
        let (started_tx, started_rx) = oneshot::channel::<()>();
        let (finish_tx, finish_rx) = oneshot::channel::<()>();
        executions.spawn(async move {
            started_tx.send(()).expect("execution entered");
            finish_rx.await.expect("execution allowed to finish");
        });
        started_rx.await.expect("execution started");

        let drain = tokio::spawn(async move {
            drain_cron_executions(&mut executions).await;
        });
        tokio::task::yield_now().await;
        assert!(!drain.is_finished(), "a started execution must be joined");
        finish_tx.send(()).expect("release execution");
        tokio::time::timeout(tokio::time::Duration::from_secs(2), drain)
            .await
            .expect("drain completes")
            .expect("drain task joins");
    }

    #[tokio::test]
    async fn pre_signaled_cron_shutdown_does_not_admit_a_tick() {
        let (stop, stop_rx) = watch::channel(true);
        let scheduler = WorkflowCronScheduler::new(DatabaseConnection::default());
        let handle = scheduler.start_with_stop(stop_rx);
        tokio::time::timeout(tokio::time::Duration::from_secs(2), handle)
            .await
            .expect("scheduler observes pre-signaled stop")
            .expect("scheduler exits without database access");
        drop(stop);
    }
}
