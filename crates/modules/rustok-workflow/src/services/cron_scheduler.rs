use std::str::FromStr;
use std::sync::Arc;

use cron::Schedule;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

use crate::entities::{WorkflowEntity, WorkflowStatus, workflow};
use crate::services::cron_durable::{
    MAX_ACTIVE_EXECUTIONS, MAX_PENDING_FIRINGS_PER_TICK, claim_firing, enqueue_due_slots,
    execute_claim, pending_firings, quarantine_stale_attempts,
};
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

    /// Validate the Workflow-owned cursor/firing schema before publishing the
    /// host scheduler handle. A missing migration is a fatal readiness error.
    pub async fn validate_persistence(&self) -> crate::error::WorkflowResult<()> {
        crate::services::cron_durable::verify_cron_schema(&self.db).await
    }

    /// Start the scheduler without a host shutdown signal.
    /// Aborting this handle also drops and aborts its owned executions; callers
    /// with a lifecycle signal should prefer `start_with_stop`.
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(error) = self.validate_persistence().await {
                error!(%error, "Workflow cron scheduler cannot run without durable owner storage");
                return;
            }
            info!("WorkflowCronScheduler started");
            self.run(None).await;
        })
    }

    /// Start with host-owned graceful shutdown. Stop admitting new triggers,
    /// then wait for already-started execution tasks before the scheduler exits.
    /// The host may still enforce a bounded forced-abort deadline.
    pub fn start_with_stop(self, stop_rx: watch::Receiver<bool>) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Do not touch storage when a host already requested shutdown.
            if *stop_rx.borrow() {
                return;
            }
            if let Err(error) = self.validate_persistence().await {
                error!(%error, "Workflow cron scheduler cannot run without durable owner storage");
                return;
            }
            // The async readiness query may have raced shutdown.
            if *stop_rx.borrow() {
                return;
            }
            info!("WorkflowCronScheduler started with host shutdown supervision");
            self.run(Some(stop_rx)).await;
        })
    }

    async fn run(&self, mut stop_rx: Option<watch::Receiver<bool>>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut executions = JoinSet::new();
        let mut workflow_cursor = None;

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
                // Resolve the wait before borrowing the same stop receiver for
                // tick admission. The stop branch is biased ahead of the tick.
                let tick_due = tokio::select! {
                    biased;
                    changed = stop.changed() => {
                        if changed.is_err() || *stop.borrow() {
                            break;
                        }
                        false
                    }
                    _ = interval.tick() => true,
                };
                if !tick_due {
                    continue;
                }
                if *stop.borrow() {
                    break;
                }
                if let Err(error) = self.tick(&mut executions, Some(stop), &mut workflow_cursor).await {
                    error!(%error, "CronScheduler tick error");
                }
            } else {
                interval.tick().await;
                if let Err(error) = self.tick(&mut executions, None, &mut workflow_cursor).await {
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
        workflow_cursor: &mut Option<uuid::Uuid>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        const WORKFLOW_PAGE_SIZE: u64 = 100;
        let now = chrono::Utc::now();

        // Bounded keyset pagination gives every active tenant's workflow a
        // turn without materializing the entire deployment on each tick.
        let mut select = WorkflowEntity::find()
            .filter(workflow::Column::Status.eq(WorkflowStatus::Active.to_string()))
            .order_by_asc(workflow::Column::Id)
            .limit(WORKFLOW_PAGE_SIZE);
        if let Some(after) = workflow_cursor {
            select = select.filter(workflow::Column::Id.gt(*after));
        }
        let workflows = select.all(&self.db).await?;
        if let Some(last) = workflows.last() {
            *workflow_cursor = Some(last.id);
        } else {
            *workflow_cursor = None;
        }

        for wf in workflows {
            if stop_rx.is_some_and(|rx| *rx.borrow()) { break; }
            let trigger = &wf.trigger_config;
            if trigger.get("type").and_then(|v| v.as_str()) != Some("cron") {
                continue;
            }
            let expr = match trigger.get("expression").and_then(|v| v.as_str()) {
                Some(expression) => expression,
                None => {
                    warn!(workflow_id = %wf.id, "Cron trigger missing expression");
                    continue;
                }
            };
            let schedule = match Schedule::from_str(expr) {
                Ok(schedule) => schedule,
                Err(error) => {
                    warn!(workflow_id = %wf.id, %error, "Invalid cron expression");
                    continue;
                }
            };
            if let Err(error) = enqueue_due_slots(&self.db, &wf, expr, &schedule, now).await {
                // Failed transaction leaves cursor and firings unadvanced.
                // Retry this schedule on the next bounded scan.
                error!(workflow_id = %wf.id, %error, "Durable cron slot enqueue failed");
            }
        }

        // Quarantine expired attempts without replay. Time alone cannot prove
        // that a remote HTTP/notify effect has not already been accepted.
        if !stop_rx.is_some_and(|rx| *rx.borrow()) {
            match quarantine_stale_attempts(&self.db, now).await {
                Ok(count) if count > 0 => warn!(
                    count, "Workflow cron attempts require operator reconciliation"
                ),
                Ok(_) => {}
                Err(error) => error!(%error, "Workflow cron stale-attempt scan failed"),
            }
        }

        // Claim is a DB CAS. A second replica seeing the same pending row
        // cannot spawn a duplicate execution. Reconciliation-required attempts
        // are never selected by this query.
        let capacity = MAX_ACTIVE_EXECUTIONS.saturating_sub(executions.len());
        if capacity == 0 || stop_rx.is_some_and(|rx| *rx.borrow()) {
            return Ok(());
        }
        let pending = pending_firings(&self.db, (capacity as u64).min(MAX_PENDING_FIRINGS_PER_TICK)).await?;
        for item in pending {
            if stop_rx.is_some_and(|rx| *rx.borrow()) { break; }
            let Some(token) = claim_firing(&self.db, &item, chrono::Utc::now()).await? else {
                continue;
            };
            // The durable claim is the admission point: this owner must
            // supervise the newly accepted operation to completion.
            let engine = Arc::clone(&self.engine);
            let service = Arc::clone(&self.service);
            let db = self.db.clone();
            executions.spawn(async move {
                if let Err(error) = execute_claim(&db, &engine, &service, item, token).await {
                    error!(%error, "Workflow cron execution requires recovery inspection");
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
