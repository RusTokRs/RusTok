use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::Utc;
use cron::Schedule;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
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
