use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::Utc;
use cron::Schedule;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

use crate::context::{ExecutionContext, ExecutionPhase};
use crate::model::{Script, ScriptId, ScriptTrigger};
use crate::runner::ScriptExecutor;
use crate::storage::{ScriptQuery, ScriptRegistry};

use super::job::ScheduledJob;

pub struct Scheduler<S: ScriptRegistry + 'static> {
    executor: ScriptExecutor<S>,
    registry: Arc<S>,
    jobs: Arc<RwLock<HashMap<ScriptId, ScheduledJob>>>,
    running: Arc<RwLock<bool>>,
}

impl<S: ScriptRegistry + 'static> Scheduler<S> {
    pub fn new(executor: ScriptExecutor<S>, registry: Arc<S>) -> Self {
        Self {
            executor,
            registry,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn load_jobs(&self) -> Result<usize, crate::error::ScriptError> {
        let scripts = self.registry.find(ScriptQuery::Scheduled).await?;
        let mut jobs = self.jobs.write().await;
        jobs.clear();

        for script in scripts {
            if let ScriptTrigger::Cron { expression } = &script.trigger {
                match self.create_job(&script, expression) {
                    Ok(job) => {
                        info!("Loaded cron job: {} ({})", script.name, expression);
                        jobs.insert(script.id, job);
                    }
                    Err(err) => {
                        warn!("Invalid cron expression for {}: {}", script.name, err);
                    }
                }
            }
        }

        Ok(jobs.len())
    }

    pub async fn start(&self) {
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Scheduler already running");
                return;
            }
            *running = true;
        }

        info!("Scheduler started");
        let mut ticker = interval(Duration::from_secs(1));

        loop {
            ticker.tick().await;

            if !*self.running.read().await {
                info!("Scheduler stopped");
                break;
            }

            self.tick().await;
        }
    }

    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Scheduler stop requested");
    }

    pub async fn status(&self) -> Vec<ScheduledJob> {
        let jobs = self.jobs.read().await;
        jobs.values().cloned().collect()
    }

    async fn tick(&self) {
        let now = Utc::now();
        let mut jobs_to_run = Vec::new();

        {
            let jobs = self.jobs.read().await;
            for (id, job) in jobs.iter() {
                if !job.running && job.next_run <= now {
                    jobs_to_run.push(*id);
                }
            }
        }

        for script_id in jobs_to_run {
            self.execute_job(script_id).await;
        }
    }

    async fn execute_job(&self, script_id: ScriptId) {
        {
            let mut jobs = self.jobs.write().await;
            if let Some(job) = jobs.get_mut(&script_id) {
                job.running = true;
            }
        }

        let script = match self.registry.get(script_id).await {
            Ok(script) => script,
            Err(err) => {
                error!("Failed to load scheduled script {}: {}", script_id, err);
                self.mark_finished(script_id).await;
                return;
            }
        };

        info!("Executing scheduled script: {}", script.name);

        let ctx = ExecutionContext::new(ExecutionPhase::Scheduled)
            .with_tenant(script.tenant_id.to_string());
        let result = self.executor.execute(&script, &ctx, None).await;

        self.update_schedule(&script).await;

        match result.outcome {
            crate::runner::ExecutionOutcome::Failed { error } => {
                error!("Scheduled script {} failed: {}", script.name, error);
            }
            crate::runner::ExecutionOutcome::Aborted { reason } => {
                warn!("Scheduled script {} aborted: {}", script.name, reason);
            }
            crate::runner::ExecutionOutcome::Success { .. } => {
                info!(
                    "Scheduled script {} completed in {}ms",
                    script.name,
                    result.duration_ms()
                );
            }
        }
    }

    async fn mark_finished(&self, script_id: ScriptId) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&script_id) {
            job.running = false;
        }
    }

    async fn update_schedule(&self, script: &Script) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(&script.id) {
            job.running = false;
            job.last_run = Some(Utc::now());

            if let Ok(schedule) = Schedule::from_str(&job.cron_expression) {
                if let Some(next) = schedule.upcoming(Utc).next() {
                    job.next_run = next;
                }
            }
        }
    }

    fn create_job(&self, script: &Script, cron_expr: &str) -> Result<ScheduledJob, String> {
        let schedule =
            Schedule::from_str(cron_expr).map_err(|err| format!("Invalid cron: {err}"))?;

        let next_run = schedule
            .upcoming(Utc)
            .next()
            .ok_or_else(|| "No upcoming schedule".to_string())?;

        Ok(ScheduledJob {
            script_id: script.id,
            script_name: script.name.clone(),
            cron_expression: cron_expr.to_string(),
            next_run,
            last_run: None,
            running: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    use crate::create_default_alloy_draft_runtime;
    use crate::error::ScriptResult;
    use crate::execution_log::ExecutionLogSink;
    use crate::model::{AlloyWorkspace, ScriptStatus};
    use crate::runner::ExecutionResult;
    use crate::storage::InMemoryStorage;

    #[derive(Default)]
    struct CapturingExecutionLog {
        entries: Mutex<Vec<(ExecutionResult, ExecutionContext)>>,
    }

    #[async_trait]
    impl ExecutionLogSink for CapturingExecutionLog {
        async fn record_result(
            &self,
            result: &ExecutionResult,
            ctx: &ExecutionContext,
        ) -> ScriptResult<()> {
            self.entries
                .lock()
                .expect("execution log lock should not be poisoned")
                .push((result.clone(), ctx.clone()));
            Ok(())
        }
    }

    #[tokio::test]
    async fn scheduler_tick_persists_execution_log_with_script_tenant() {
        let storage = Arc::new(InMemoryStorage::new());
        let execution_log = Arc::new(CapturingExecutionLog::default());
        let executor =
            ScriptExecutor::new(create_default_alloy_draft_runtime(), Arc::clone(&storage))
                .with_execution_log(execution_log.clone());
        let scheduler = Scheduler::new(executor, Arc::clone(&storage));

        let mut script = Script::new(
            "scheduled_audit_smoke",
            AlloyWorkspace::single_source("40 + 2"),
            ScriptTrigger::Cron {
                expression: "0 0 0 1 1 * 2099".to_string(),
            },
        );
        script.status = ScriptStatus::Active;
        let script_id = script.id;
        let tenant_id = script.tenant_id;
        storage.save(script).await.unwrap();

        scheduler.jobs.write().await.insert(
            script_id,
            ScheduledJob {
                script_id,
                script_name: "scheduled_audit_smoke".to_string(),
                cron_expression: "0 0 0 1 1 * 2099".to_string(),
                next_run: Utc::now() - chrono::Duration::seconds(1),
                last_run: None,
                running: false,
            },
        );

        scheduler.tick().await;

        let entries = execution_log
            .entries
            .lock()
            .expect("execution log lock should not be poisoned");
        assert_eq!(entries.len(), 1);
        let (result, ctx) = &entries[0];
        assert_eq!(result.script_id, script_id);
        assert_eq!(result.phase, ExecutionPhase::Scheduled);
        assert_eq!(ctx.phase, ExecutionPhase::Scheduled);
        let tenant_id_str = tenant_id.to_string();
        assert_eq!(ctx.tenant_id.as_deref(), Some(tenant_id_str.as_str()));
        assert!(result.is_success());
    }
}
