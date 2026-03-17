use std::str::FromStr;
use std::sync::Arc;

use cron::Schedule;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::entities::{workflow, WorkflowEntity, WorkflowStatus};
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
        Self { db, engine, service }
    }

    /// Start the cron scheduler as a background task.
    /// Returns a handle that can be aborted to stop the scheduler.
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            info!("WorkflowCronScheduler started");
            self.run().await;
        })
    }

    async fn run(&self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if let Err(e) = self.tick().await {
                error!(error = %e, "CronScheduler tick error");
            }
        }
    }

    async fn tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = chrono::Utc::now();

        let workflows = WorkflowEntity::find()
            .filter(workflow::Column::Status.eq(WorkflowStatus::Active.to_string()))
            .all(&self.db)
            .await?;

        for wf in workflows {
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

            let should_fire = next_after_prev
                .map(|t| t <= now)
                .unwrap_or(false);

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

            let engine = self.engine.clone();
            let ctx = json!({ "trigger": { "type": "cron", "expression": expr } });

            tokio::spawn(async move {
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
